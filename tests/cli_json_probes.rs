//! File-sourced and JSON-selected probes, end to end.
//!
//! The unit tests in `src/probe_json_tests.rs` pin the digest arithmetic. What
//! this file proves is the property a user actually buys: a rule can depend on
//! one field of a JSON file, re-run when that field moves, and stay fresh when
//! anything else in the file does — including the file being rewritten with its
//! keys in a different order, which is the failure every shelled-out probe here
//! carries `jq -S` to avoid.

use std::fs;
use std::path::Path;
use std::time::Duration;

use predicates::prelude::predicate;

mod support;
use support::{mmz, write_manifest};

/// Longest a probe may take before the test calls it hung rather than slow.
const PATIENCE: Duration = Duration::from_secs(30);

/// A rule pinned to one node of a lockfile-shaped document. Nothing else about
/// `lock.json` is an input — which is the whole point, since the real
/// `flake.lock` this stands in for has over a hundred nodes.
const PINNED_NODE: &str = concat!(
    "probes:\n  qahq-input:\n    file: lock.json\n",
    "    json: '.nodes[\"qahq\"][\"locked\"][\"narHash\"]'\n",
    "commands:\n  - name: sh\n    inputs: [qahq-input]\n",
);

fn write_project(dir: &Path, manifest: &str, lock: &str) {
    write_manifest(dir, manifest);
    fs::write(dir.join("lock.json"), lock).expect("write lockfile");
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

#[test]
fn the_selected_field_is_the_input_and_the_rest_of_the_file_is_not() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_project(
        dir.path(),
        PINNED_NODE,
        r#"{"nodes": {"qahq": {"locked": {"narHash": "sha256-one"}},
                     "nixpkgs": {"locked": {"narHash": "sha256-old"}}}}"#,
    );

    build(dir.path()).success();
    assert_eq!(runs(dir.path()), 1, "first run executes");
    build(dir.path()).success();
    assert_eq!(runs(dir.path()), 1, "nothing moved, so the rule is fresh");

    // A sibling node moves — the case that busts a whole-file scope today.
    fs::write(
        dir.path().join("lock.json"),
        r#"{"nodes": {"qahq": {"locked": {"narHash": "sha256-one"}},
                     "nixpkgs": {"locked": {"narHash": "sha256-new"}}}}"#,
    )
    .expect("bump the sibling");
    build(dir.path()).success();
    assert_eq!(
        runs(dir.path()),
        1,
        "an input the selector does not name is not this rule's dependency"
    );

    fs::write(
        dir.path().join("lock.json"),
        r#"{"nodes": {"qahq": {"locked": {"narHash": "sha256-two"}},
                     "nixpkgs": {"locked": {"narHash": "sha256-new"}}}}"#,
    )
    .expect("bump the pinned node");
    build(dir.path()).success();
    assert_eq!(
        runs(dir.path()),
        2,
        "the selected value moved, so the rule re-runs"
    );
}

#[test]
fn rewriting_the_file_with_its_keys_reordered_does_not_bust_the_rule() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_manifest(
        dir.path(),
        concat!(
            "probes:\n  recipe:\n    file: dump.json\n    json: '.recipes[\"clippy\"]'\n",
            "commands:\n  - name: sh\n    inputs: [recipe]\n",
        ),
    );
    fs::write(
        dir.path().join("dump.json"),
        r#"{"recipes": {"clippy": {"body": "cargo clippy", "doc": "lint", "name": "clippy"}}}"#,
    )
    .expect("write dump");

    build(dir.path()).success();
    assert_eq!(runs(dir.path()), 1, "first run executes");

    // The same content, rendered with the keys in the order a different tool
    // version might pick. Byte-different; content-identical.
    fs::write(
        dir.path().join("dump.json"),
        r#"{"recipes": {"clippy": {"name": "clippy", "doc": "lint", "body": "cargo clippy"}}}"#,
    )
    .expect("re-render dump");
    build(dir.path()).success();
    assert_eq!(
        runs(dir.path()),
        1,
        "mmz hashes the parsed value with keys sorted, so a renderer's key order \
         is structurally not an input"
    );
}

#[test]
fn a_selector_that_matches_nothing_exits_6_and_records_nothing() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_project(dir.path(), PINNED_NODE, r#"{"nodes": {}}"#);

    build(dir.path())
        .code(6)
        .stderr(predicate::str::contains("probe `qahq-input`"))
        .stderr(predicate::str::contains("selected nothing"));
    assert_eq!(runs(dir.path()), 0, "the wrapped command never ran");
    assert!(!recorded(dir.path()), "and nothing was recorded");
}

#[test]
fn a_file_that_is_missing_or_malformed_exits_6_naming_the_probe_and_the_path() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_manifest(dir.path(), PINNED_NODE);

    build(dir.path())
        .code(6)
        .stderr(predicate::str::contains("probe `qahq-input`"))
        .stderr(predicate::str::contains("lock.json"));

    fs::write(dir.path().join("lock.json"), "{ this is not json").expect("write junk");
    build(dir.path())
        .code(6)
        .stderr(predicate::str::contains("probe `qahq-input`"))
        .stderr(predicate::str::contains("not one JSON value"));
    assert!(!recorded(dir.path()), "neither case reached the hasher");
}

#[test]
fn declaring_both_run_and_file_is_a_manifest_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_manifest(
        dir.path(),
        concat!(
            "probes:\n  confused:\n    run: cat lock.json\n    file: lock.json\n",
            "    json: '.'\n",
            "commands:\n  - name: sh\n    inputs: [confused]\n",
        ),
    );
    fs::write(dir.path().join("lock.json"), "{}").expect("write lockfile");

    build(dir.path())
        .code(4)
        .stderr(predicate::str::contains("probe `confused`"))
        .stderr(predicate::str::contains("exactly one source"));
    assert_eq!(runs(dir.path()), 0, "an invalid manifest runs nothing");
}

#[test]
fn a_run_line_can_carry_a_selector_instead_of_piping_through_jq() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_manifest(
        dir.path(),
        concat!(
            "probes:\n  recipe:\n    run: cat dump.json\n    json: '.recipes[\"clippy\"]'\n",
            "commands:\n  - name: sh\n    inputs: [recipe]\n",
        ),
    );
    fs::write(
        dir.path().join("dump.json"),
        r#"{"recipes": {"clippy": {"body": "cargo clippy"}, "test": {"body": "cargo test"}}}"#,
    )
    .expect("write dump");

    build(dir.path()).success();
    build(dir.path()).success();
    assert_eq!(runs(dir.path()), 1, "one spawn, and the rule is fresh");

    fs::write(
        dir.path().join("dump.json"),
        r#"{"recipes": {"clippy": {"body": "cargo clippy --all"}, "test": {"body": "cargo test"}}}"#,
    )
    .expect("edit the selected recipe");
    build(dir.path()).success();
    assert_eq!(
        runs(dir.path()),
        2,
        "the selection moved, without `jq` ever being on PATH"
    );
}
