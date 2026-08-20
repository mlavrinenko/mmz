//! Unit tests for a probe's `ast:`, `capture:` and `lang:` keys, split from
//! `probe_tests.rs` so neither file reaches the line cap.
//!
//! [`crate::ast`]'s own tests cover the rendering. What is tested here is the
//! wiring: that a real probe over a real file on disk narrows to the slice its
//! pattern names — and, under `capture:`, to the parts of that slice the list
//! names — that the shape rules refuse every key combination mmz cannot read,
//! and that a pattern matching nothing stops before the hasher.

use super::{Resolver, validate};
use crate::manifest::Manifest;

/// Parses a manifest body, skipping validation so a test can build a shape
/// `validate` would reject on purpose.
fn parse(body: &str) -> Manifest {
    serde_yaml_ng::from_str(body).expect("manifest parses")
}

/// The rule named `sh` — every fixture below declares exactly one.
fn rule(manifest: &Manifest) -> &crate::manifest::Command {
    manifest
        .commands
        .iter()
        .find(|rule| rule.name == "sh")
        .expect("rule declared")
}

/// A project root holding `lib.rs` with the given source, plus a manifest whose
/// one probe matches `pattern` over it.
fn rust_project(source: &str, pattern: &str) -> (tempfile::TempDir, Manifest) {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("lib.rs"), source).expect("write source");
    let manifest = parse(&format!(
        "probes:\n  api:\n    file: lib.rs\n    ast: '{pattern}'\ncommands:\n  - name: sh\n    inputs: [api]\n"
    ));
    (dir, manifest)
}

/// The digest of the fixture's `api` probe, resolved from `dir`.
fn digest_of(dir: &tempfile::TempDir, manifest: &Manifest) -> String {
    Resolver::new(manifest, dir.path())
        .for_rule(rule(manifest))
        .expect("probe resolves")
        .get("api")
        .cloned()
        .expect("probe `api` resolved")
}

/// The error a fixture's probe fails with.
fn failure_of(dir: &tempfile::TempDir, manifest: &Manifest) -> crate::error::Error {
    Resolver::new(manifest, dir.path())
        .for_rule(rule(manifest))
        .expect_err("probe fails")
}

/// The reason `validate` refuses `body`'s probes.
fn shape_refusal(body: &str) -> String {
    let manifest = parse(body);
    let failed = validate(&manifest.probes, &manifest.scopes, &manifest.probe_shell)
        .expect_err("the shape is refused");
    failed.to_string()
}

#[cfg(feature = "lang-rust")]
#[test]
fn an_ast_probe_reads_a_file_without_spawning_anything() {
    let (dir, manifest) = rust_project("pub fn one() {}\n", "pub fn $N() {}");
    assert_eq!(digest_of(&dir, &manifest).len(), 64, "a blake3 hex digest");
}

/// The headline: the probe depends on the signatures, so everything else in
/// the file is free to move. A scope naming `lib.rs` could not say this.
#[cfg(feature = "lang-rust")]
#[test]
fn only_the_matched_slice_moves_the_digest() {
    let pattern = "pub fn $N() {}";
    let (before, manifest) =
        rust_project("// first\npub fn one() {}\nfn hidden() { 1; }\n", pattern);
    let (after, _) = rust_project(
        "// second thoughts\npub fn one() {}\nfn hidden() { 2; }\n",
        pattern,
    );
    assert_eq!(
        digest_of(&before, &manifest),
        digest_of(&after, &manifest),
        "a reworded comment and a changed private body are not this probe's input"
    );
}

#[cfg(feature = "lang-rust")]
#[test]
fn a_change_inside_the_matched_slice_moves_the_digest() {
    let pattern = "pub fn $N() {}";
    let (before, manifest) = rust_project("pub fn one() {}\n", pattern);
    let (after, _) = rust_project("pub fn renamed() {}\n", pattern);
    assert_ne!(digest_of(&before, &manifest), digest_of(&after, &manifest));
}

/// The same refusal an empty `json:` selection gets, for the same reason.
#[cfg(feature = "lang-rust")]
#[test]
fn a_pattern_matching_nothing_is_a_hard_error() {
    let (dir, manifest) = rust_project("fn private() {}\n", "pub fn $N() {}");
    let message = failure_of(&dir, &manifest).to_string();
    assert!(message.contains("api"), "names the probe: {message}");
    assert!(
        message.contains("allow_empty"),
        "names the opt-in: {message}"
    );
}

#[cfg(feature = "lang-rust")]
#[test]
fn allow_empty_opts_into_a_pattern_that_matched_nothing() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("lib.rs"), "fn private() {}\n").expect("write source");
    let manifest = parse(
        "probes:\n  api:\n    file: lib.rs\n    ast: 'pub fn $N() {}'\n    allow_empty: true\ncommands:\n  - name: sh\n    inputs: [api]\n",
    );
    assert_eq!(digest_of(&dir, &manifest).len(), 64);
}

/// A `run:` probe has no path to infer a language from, and mmz says so rather
/// than guessing one out of the command line.
#[test]
fn a_run_probe_without_lang_names_the_missing_key() {
    let dir = tempfile::tempdir().expect("tempdir");
    let manifest = parse(
        "probes:\n  api:\n    run: printf 'pub fn one() {}'\n    ast: 'pub fn $N() {}'\ncommands:\n  - name: sh\n    inputs: [api]\n",
    );
    let message = failure_of(&dir, &manifest).to_string();
    assert!(message.contains("lang:"), "names the key: {message}");
}

#[cfg(feature = "lang-rust")]
#[test]
fn a_run_probe_with_lang_matches_over_stdout() {
    let dir = tempfile::tempdir().expect("tempdir");
    let manifest = parse(
        "probes:\n  api:\n    run: printf 'pub fn one() {}'\n    ast: 'pub fn $N() {}'\n    lang: rust\ncommands:\n  - name: sh\n    inputs: [api]\n",
    );
    assert_eq!(digest_of(&dir, &manifest).len(), 64);
}

#[test]
fn a_probe_declaring_both_selectors_is_refused() {
    let message =
        shape_refusal("probes:\n  api:\n    file: lib.rs\n    json: '.a'\n    ast: 'fn $N() {}'\n");
    assert!(message.contains("one selector"), "{message}");
}

#[test]
fn lang_without_ast_is_refused() {
    let message =
        shape_refusal("probes:\n  api:\n    file: data.json\n    json: '.a'\n    lang: rust\n");
    assert!(message.contains("without `ast:`"), "{message}");
}

/// `file:` alone was already refused; the message now offers both selectors,
/// so a reader pointed at a source file is not sent to jq.
#[test]
fn a_file_with_no_selector_offers_both() {
    let message = shape_refusal("probes:\n  api:\n    file: lib.rs\n");
    assert!(message.contains("`json:` or `ast:`"), "{message}");
}

#[test]
fn an_unknown_language_is_refused_at_resolve_time() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("lib.rs"), "pub fn one() {}\n").expect("write source");
    let manifest = parse(
        "probes:\n  api:\n    file: lib.rs\n    ast: 'pub fn $N() {}'\n    lang: cobol\ncommands:\n  - name: sh\n    inputs: [api]\n",
    );
    let message = failure_of(&dir, &manifest).to_string();
    assert!(message.contains("cobol"), "names the language: {message}");
    assert!(message.contains("this build parses"), "{message}");
}

/// A project root holding `lib.rs`, plus a manifest whose one probe matches
/// `pattern` over it under the given `capture:` list.
fn captured_project(source: &str, pattern: &str, capture: &str) -> (tempfile::TempDir, Manifest) {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("lib.rs"), source).expect("write source");
    let manifest = parse(&format!(
        "probes:\n  api:\n    file: lib.rs\n    ast: '{pattern}'\n    capture: {capture}\ncommands:\n  - name: sh\n    inputs: [api]\n"
    ));
    (dir, manifest)
}

/// The task's motivating example: the pattern has to span the body to reach a
/// function that has one, and `capture:` is what keeps that body out of the
/// digest.
const SIGNATURE: &str = "pub fn $NAME($$$ARGS) -> $RET { $$$BODY }";

#[cfg(feature = "lang-rust")]
#[test]
fn a_captured_probe_depends_on_the_signature_and_not_the_body() {
    let (before, manifest) = captured_project(
        "pub fn one(a: u8) -> u8 { a }\n",
        SIGNATURE,
        "[NAME, ARGS, RET]",
    );
    let (after, _) = captured_project(
        "pub fn one(a: u8) -> u8 { a + 0 }\n",
        SIGNATURE,
        "[NAME, ARGS, RET]",
    );
    assert_eq!(
        digest_of(&before, &manifest),
        digest_of(&after, &manifest),
        "the body was matched but not captured"
    );

    let (renamed, _) = captured_project(
        "pub fn renamed(a: u8) -> u8 { a }\n",
        SIGNATURE,
        "[NAME, ARGS, RET]",
    );
    assert_ne!(
        digest_of(&before, &manifest),
        digest_of(&renamed, &manifest),
        "the signature is still the input"
    );
}

/// Without `capture:` the same pattern depends on the body, which is the
/// default this feature narrows rather than replaces.
#[cfg(feature = "lang-rust")]
#[test]
fn the_default_is_still_the_whole_matched_node() {
    let (before, manifest) = rust_project("pub fn one(a: u8) -> u8 { a }\n", SIGNATURE);
    let (after, _) = rust_project("pub fn one(a: u8) -> u8 { a + 0 }\n", SIGNATURE);
    assert_ne!(
        digest_of(&before, &manifest),
        digest_of(&after, &manifest),
        "a match is a whole node unless `capture:` says otherwise"
    );
}

/// Naming something the pattern does not define fails at the probe, naming the
/// probe — an empty capture would otherwise narrow the digest silently.
#[cfg(feature = "lang-rust")]
#[test]
fn a_capture_the_pattern_does_not_define_names_the_probe() {
    let (dir, manifest) = captured_project("pub fn one() -> u8 { 1 }\n", SIGNATURE, "[NAME, TYPO]");
    let message = failure_of(&dir, &manifest).to_string();
    assert!(message.contains("api"), "names the probe: {message}");
    assert!(message.contains("TYPO"), "names the miss: {message}");
    assert!(
        message.contains("`NAME`"),
        "names what is defined: {message}"
    );
}

/// `allow_empty:` waives an empty match set, never an undefined capture: the
/// matches are all there, so there is no emptiness for it to be about.
#[cfg(feature = "lang-rust")]
#[test]
fn allow_empty_does_not_waive_an_undefined_capture() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("lib.rs"), "pub fn one() -> u8 { 1 }\n").expect("write source");
    let manifest = parse(&format!(
        "probes:\n  api:\n    file: lib.rs\n    ast: '{SIGNATURE}'\n    capture: [TYPO]\n    allow_empty: true\ncommands:\n  - name: sh\n    inputs: [api]\n"
    ));
    assert!(
        failure_of(&dir, &manifest).to_string().contains("TYPO"),
        "an undefined capture is not an emptiness `allow_empty` can opt into"
    );
}

/// Everything decidable from the manifest alone is decided at load, so a bad
/// list never waits for the probe to be reached.
#[test]
fn the_capture_list_rules_are_refused_at_load() {
    for (body, expected) in [
        (
            "probes:\n  api:\n    file: lib.rs\n    json: '.a'\n    capture: [NAME]\n",
            "without `ast:`",
        ),
        (
            "probes:\n  api:\n    file: lib.rs\n    ast: 'pub fn $NAME()'\n    capture: []\n",
            "empty `capture:` list",
        ),
        (
            "probes:\n  api:\n    file: lib.rs\n    ast: 'pub fn $NAME()'\n    capture: ['$NAME']\n",
            "without the `$`",
        ),
        (
            "probes:\n  api:\n    file: lib.rs\n    ast: 'pub fn $NAME()'\n    capture: [name]\n",
            "not a metavariable name",
        ),
        (
            "probes:\n  api:\n    file: lib.rs\n    ast: 'pub fn $NAME()'\n    capture: [NAME, NAME]\n",
            "twice",
        ),
    ] {
        let message = shape_refusal(body);
        assert!(message.contains(expected), "{expected}: {message}");
    }
}

/// `$_X` is a *dropped* variable in ast-grep rather than a captured one, so it
/// could only ever hash nothing — refused by the name rule at load rather than
/// left to come back as "the pattern does not define it", which would send a
/// reader to edit the pattern instead of the list.
#[test]
fn a_dropped_variable_is_refused_by_name() {
    let message = shape_refusal(
        "probes:\n  api:\n    file: lib.rs\n    ast: 'pub fn $NAME($_ARG)'\n    capture: [_ARG]\n",
    );
    assert!(message.contains("not a metavariable name"), "{message}");
}
