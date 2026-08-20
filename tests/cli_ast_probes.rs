//! AST-selected probes, end to end.
//!
//! The unit tests in `src/ast_tests.rs` pin the rendering and
//! `src/probe_ast_tests.rs` the wiring. What this file proves is the property a
//! user actually buys: a rule can depend on the *shape* of some code, re-run
//! when that shape moves, and stay fresh when the file changes around it —
//! which is the input a scope naming the file cannot express at all.
//!
//! Every test here needs a grammar, and `lang-rust` is the one a default build
//! carries. Under `--no-default-features` the file compiles to nothing rather
//! than failing, since what it asserts is about a language the binary may not
//! have been built with.

#![cfg(feature = "lang-rust")]

use std::fs;
use std::path::Path;
use std::time::Duration;

use predicates::prelude::predicate;

mod support;
use support::{mmz, write_manifest};

/// Longest a probe may take before the test calls it hung rather than slow.
const PATIENCE: Duration = Duration::from_secs(30);

/// A rule pinned to the public functions of `lib.rs`. Nothing else about the
/// file is an input — not its comments, not its private items.
const PINNED_API: &str = concat!(
    "probes:\n  public-api:\n    file: lib.rs\n",
    "    ast: 'pub fn $NAME($$$ARGS) -> $RET { $$$BODY }'\n",
    "commands:\n  - name: sh\n    inputs: [public-api]\n",
);

fn write_project(dir: &Path, manifest: &str, source: &str) {
    write_manifest(dir, manifest);
    fs::write(dir.join("lib.rs"), source).expect("write source");
}

/// Wraps a command that logs one byte per real execution.
fn build(dir: &Path) -> assert_cmd::assert::Assert {
    mmz(dir)
        .timeout(PATIENCE)
        .args(["sh", "-c", "printf x >> runs.log"])
        .assert()
}

fn runs(dir: &Path) -> usize {
    fs::read(dir.join("runs.log")).map_or(0, |bytes| bytes.len())
}

/// Whether any cache record was written — the assertion that a hard error
/// stopped before the hasher rather than after it.
fn recorded(dir: &Path) -> bool {
    fs::read_dir(dir.join(".mmz/cache")).is_ok_and(|entries| {
        entries
            .filter_map(Result::ok)
            .any(|entry| entry.file_name().to_string_lossy().ends_with(".yaml"))
    })
}

/// The headline property, in one test: the signatures are the input, and
/// everything else in the file is free to move.
#[test]
fn the_matched_shape_is_the_input_and_the_rest_of_the_file_is_not() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_project(
        dir.path(),
        PINNED_API,
        "// a first thought\npub fn one(a: u8) -> u8 { a }\nfn hidden() -> u8 { 1 }\n",
    );
    build(dir.path()).success();
    assert_eq!(runs(dir.path()), 1, "the first run is a miss");

    build(dir.path()).success();
    assert_eq!(runs(dir.path()), 1, "an unchanged tree is a hit");

    // Reword the comment, rewrite the private body, reflow the matched one
    // across lines. None of it is this probe's input: the first two are not
    // matched at all, and the third changes no token.
    fs::write(
        dir.path().join("lib.rs"),
        "// a second thought, at length\npub fn one(\n    a: u8\n) -> u8 {\n    a\n}\nfn hidden() -> u8 { 99 }\n",
    )
    .expect("rewrite source");
    build(dir.path()).success();
    assert_eq!(
        runs(dir.path()),
        1,
        "a comment, a private body and a reflowed signature move no token"
    );

    // Rename the public function. That is the input.
    fs::write(
        dir.path().join("lib.rs"),
        "// a second thought, at length\npub fn renamed(\n    a: u8\n) -> u8 {\n    a\n}\nfn hidden() -> u8 { 99 }\n",
    )
    .expect("rewrite source");
    build(dir.path()).success();
    assert_eq!(
        runs(dir.path()),
        2,
        "the signature moved, so the rule re-ran"
    );
}

/// The limit, asserted rather than left for a user to discover: a match is a
/// whole node, so a pattern spanning a function's body depends on that body.
/// Narrowing further is a question about captures, filed separately.
#[test]
fn a_matched_functions_body_is_part_of_the_match() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_project(dir.path(), PINNED_API, "pub fn one(a: u8) -> u8 { a }\n");
    build(dir.path()).success();
    assert_eq!(runs(dir.path()), 1);

    fs::write(
        dir.path().join("lib.rs"),
        "pub fn one(a: u8) -> u8 { a + 0 }\n",
    )
    .expect("rewrite source");
    build(dir.path()).success();
    assert_eq!(
        runs(dir.path()),
        2,
        "the pattern spans the body, so the body is an input"
    );
}

/// The same refusal an empty `json:` selection gets: a probe measuring nothing
/// would report one digest whatever the file said.
#[test]
fn a_pattern_that_matches_nothing_fails_before_the_hasher() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_project(dir.path(), PINNED_API, "fn private(a: u8) -> u8 { a }\n");
    build(dir.path())
        .code(6)
        .stderr(predicate::str::contains("public-api"))
        .stderr(predicate::str::contains("allow_empty"));
    assert!(!recorded(dir.path()), "no record was written");
    assert_eq!(runs(dir.path()), 0, "the command never ran");
}

/// A typo'd pattern would otherwise compile fine and match nothing, which
/// `allow_empty: true` would then wave straight through.
#[test]
fn a_pattern_the_grammar_cannot_parse_is_refused_even_with_allow_empty() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_project(
        dir.path(),
        concat!(
            "probes:\n  public-api:\n    file: lib.rs\n",
            "    ast: 'pub fn $NAME('\n    allow_empty: true\n",
            "commands:\n  - name: sh\n    inputs: [public-api]\n",
        ),
        "pub fn one() {}\n",
    );
    build(dir.path())
        .code(6)
        .stderr(predicate::str::contains("public-api"));
    assert!(!recorded(dir.path()));
}

/// The failure whose answer is a build rather than an edit, so the message has
/// to carry the flag.
#[test]
fn a_language_this_build_lacks_names_the_feature_to_rebuild_with() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_project(
        dir.path(),
        concat!(
            "probes:\n  api:\n    file: lib.rs\n",
            "    ast: 'pub fn $NAME()'\n    lang: kotlin\n",
            "commands:\n  - name: sh\n    inputs: [api]\n",
        ),
        "pub fn one() {}\n",
    );
    build(dir.path())
        .code(6)
        .stderr(predicate::str::contains("--features lang-kotlin"));
    assert!(!recorded(dir.path()));
}

/// One namespace, one selector, one meaning per key: each of these is refused
/// at load (exit 4) rather than resolved by a precedence rule.
#[test]
fn the_shape_rules_are_refused_at_load() {
    for (manifest, expected) in [
        (
            "probes:\n  api:\n    file: lib.rs\n    json: '.a'\n    ast: 'fn $N() {}'\n",
            "one selector",
        ),
        (
            "probes:\n  api:\n    file: lib.rs\n    json: '.a'\n    lang: rust\n",
            "without `ast:`",
        ),
        ("probes:\n  api:\n    file: lib.rs\n", "`json:` or `ast:`"),
    ] {
        let dir = tempfile::tempdir().expect("tempdir");
        write_project(
            dir.path(),
            &format!("{manifest}commands:\n  - name: sh\n    inputs: [api]\n"),
            "pub fn one() {}\n",
        );
        build(dir.path())
            .code(4)
            .stderr(predicate::str::contains(expected));
    }
}

/// `--dump-config` reports the merged manifest, so a key it cannot render is a
/// key a reader cannot audit.
#[test]
fn dump_config_reports_the_selector_and_its_language() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_project(
        dir.path(),
        concat!(
            "probes:\n  api:\n    run: printf 'pub fn one() {}'\n",
            "    ast: 'pub fn $NAME()'\n    lang: rust\n",
            "commands:\n  - name: sh\n    inputs: [api]\n",
        ),
        "pub fn one() {}\n",
    );
    mmz(dir.path())
        .timeout(PATIENCE)
        .arg("--dump-config")
        .assert()
        .success()
        .stdout(predicate::str::contains("ast: pub fn $NAME()"))
        .stdout(predicate::str::contains("lang: rust"));
}
