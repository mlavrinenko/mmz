//! Declared `outputs:`, end to end: a rule that promises an artifact is fresh
//! only while that artifact is on disk, and a run that exits 0 without writing
//! it is refused rather than recorded.

use std::fs;
use std::path::Path;

use predicates::prelude::{PredicateBooleanExt, predicate};

mod support;
use support::{mmz, write_manifest};

/// A rule keyed on `*.txt` that declares the artifact it produces.
const PRODUCER: &str = concat!(
    "scopes:\n  src: [\"*.txt\"]\n",
    "commands:\n  - name: sh\n    inputs: [src]\n",
    "    outputs:\n      - out/artifact.bin\n",
);

/// A project whose artifact tree is git-ignored, so the run also proves the
/// ignore filter never reaches a declared output.
fn write_project(dir: &Path) {
    write_manifest(dir, PRODUCER);
    fs::write(dir.join(".gitignore"), "/out\n").expect("write .gitignore");
    fs::write(dir.join("a.txt"), b"one").expect("write input");
    fs::create_dir(dir.join("out")).expect("mkdir out");
}

fn run_len(dir: &Path) -> usize {
    fs::read(dir.join("runs.log")).map_or(0, |bytes| bytes.len())
}

/// Wraps a build that logs a run and writes the declared artifact.
fn build(dir: &Path) {
    mmz(dir)
        .args([
            "sh",
            "-c",
            "printf x >> runs.log; printf built > out/artifact.bin",
        ])
        .assert()
        .success();
}

#[test]
fn a_deleted_artifact_voids_the_record_and_the_rule_runs_again() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_project(dir.path());

    build(dir.path());
    assert_eq!(run_len(dir.path()), 1, "first run executes");
    build(dir.path());
    assert_eq!(
        run_len(dir.path()),
        1,
        "inputs and artifact intact: skipped"
    );

    // The `cargo clean` case: the artifact goes, no input is touched.
    fs::remove_file(dir.path().join("out/artifact.bin")).expect("delete artifact");
    build(dir.path());
    assert_eq!(
        run_len(dir.path()),
        2,
        "the void record re-runs the command"
    );
}

#[test]
fn a_gate_on_a_voided_record_fails_and_names_the_artifact() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_project(dir.path());
    build(dir.path());

    mmz(dir.path()).arg("--is-fresh").assert().success();

    fs::remove_file(dir.path().join("out/artifact.bin")).expect("delete artifact");
    mmz(dir.path()).arg("--is-fresh").assert().code(1).stderr(
        predicate::str::contains("`sh` is missing-output")
            .and(predicate::str::contains(
                "declared output `out/artifact.bin` is missing",
            ))
            .and(predicate::str::contains("inputs changed").not()),
    );
    assert_eq!(run_len(dir.path()), 1, "a failing gate still runs nothing");
}

#[test]
fn status_surfaces_the_missing_artifact_in_the_table_and_json() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_project(dir.path());
    build(dir.path());
    fs::remove_file(dir.path().join("out/artifact.bin")).expect("delete artifact");

    mmz(dir.path()).arg("--status").assert().success().stdout(
        predicate::str::contains("missing-output")
            .and(predicate::str::contains("MISSING OUTPUT"))
            .and(predicate::str::contains("out/artifact.bin")),
    );
    mmz(dir.path())
        .arg("--status=json")
        .assert()
        .success()
        .stdout(
            predicate::str::contains("\"state\": \"missing-output\"").and(
                predicate::str::contains("\"missing_output\": \"out/artifact.bin\""),
            ),
        );
}

#[test]
fn a_success_without_the_artifact_exits_five_and_records_nothing() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_project(dir.path());

    mmz(dir.path())
        .args(["sh", "-c", "printf x >> runs.log"])
        .assert()
        .code(5)
        .stderr(
            predicate::str::contains("out/artifact.bin")
                .and(predicate::str::contains("no cache record was written")),
        );
    assert_eq!(run_len(dir.path()), 1, "the command did run");

    // Nothing was recorded, so the rule is still `never` — not quietly fresh.
    mmz(dir.path())
        .arg("--status")
        .assert()
        .success()
        .stdout(predicate::str::contains("never"));
    mmz(dir.path()).arg("--is-fresh").assert().code(1);
}

#[test]
fn a_glob_in_outputs_is_refused_as_a_manifest_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_manifest(
        dir.path(),
        concat!(
            "scopes:\n  src: [\"*.txt\"]\n",
            "commands:\n  - name: sh\n    inputs: [src]\n",
            "    outputs: [\"out/*.bin\"]\n",
        ),
    );
    fs::write(dir.path().join("a.txt"), b"one").expect("write input");

    mmz(dir.path())
        .args(["sh", "-c", "exit 0"])
        .assert()
        .code(4)
        .stderr(predicate::str::contains("literal paths, not patterns"));
}

#[test]
fn a_rule_without_outputs_is_unchanged() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_manifest(
        dir.path(),
        "scopes:\n  src: [\"*.txt\"]\ncommands:\n  - name: sh\n    inputs: [src]\n",
    );
    fs::write(dir.path().join("a.txt"), b"one").expect("write input");

    for _ in 0..2 {
        mmz(dir.path())
            .args(["sh", "-c", "printf x >> runs.log"])
            .assert()
            .success();
    }
    assert_eq!(run_len(dir.path()), 1, "still a plain inputs-only skip");
    mmz(dir.path()).arg("--status").assert().success().stdout(
        predicate::str::contains("fresh").and(predicate::str::contains("MISSING OUTPUT").not()),
    );
}
