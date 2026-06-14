use std::fs;
use std::path::Path;

use assert_cmd::Command;
use predicates::prelude::{PredicateBooleanExt, predicate};

/// An `mmz` invocation rooted at `dir`.
fn mmz(dir: &Path) -> Command {
    let mut cmd = Command::cargo_bin("mmz").expect("binary should build");
    cmd.current_dir(dir);
    cmd
}

/// Writes a manifest whose `sh` rule depends on every `*.txt` file, plus one
/// such input file.
fn write_project(dir: &Path) {
    fs::write(
        dir.join("mmz.yaml"),
        "scopes:\n  src: [\"*.txt\"]\ncommands:\n  - name: sh\n    inputs: [src]\n",
    )
    .expect("write manifest");
    fs::write(dir.join("a.txt"), b"one").expect("write input");
}

fn log_len(dir: &Path) -> usize {
    fs::read(dir.join("runs.log")).map_or(0, |bytes| bytes.len())
}

#[test]
fn skips_when_inputs_unchanged_and_reruns_on_change() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_project(dir.path());

    mmz(dir.path())
        .args(["sh", "-c", "printf x >> runs.log"])
        .assert()
        .success();
    assert_eq!(log_len(dir.path()), 1, "first run executes");

    mmz(dir.path())
        .args(["sh", "-c", "printf x >> runs.log"])
        .assert()
        .success();
    assert_eq!(log_len(dir.path()), 1, "second run is a cache hit");

    fs::write(dir.path().join("a.txt"), b"two").expect("rewrite input");
    mmz(dir.path())
        .args(["sh", "-c", "printf x >> runs.log"])
        .assert()
        .success();
    assert_eq!(log_len(dir.path()), 2, "changed input re-runs");
}

#[test]
fn propagates_exit_code_without_caching_failure() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_project(dir.path());

    mmz(dir.path())
        .args(["sh", "-c", "exit 3"])
        .assert()
        .code(3);
    mmz(dir.path())
        .args(["sh", "-c", "exit 3"])
        .assert()
        .code(3);
}

#[test]
fn unmatched_command_is_a_strict_refusal() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_project(dir.path());
    // The manifest only knows `sh`; an unmatched command is refused (exit 3).
    mmz(dir.path()).args(["env", "false"]).assert().code(3);
}

#[test]
fn unmatched_command_passes_through_when_relaxed() {
    let dir = tempfile::tempdir().expect("tempdir");
    fs::write(
        dir.path().join("mmz.yaml"),
        "commands:\n  - name: sh\nstrict: []\n",
    )
    .expect("write manifest");
    mmz(dir.path()).args(["env", "false"]).assert().code(1);
}

#[test]
fn missing_manifest_is_a_config_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    mmz(dir.path())
        .args(["sh", "-c", "exit 7"])
        .assert()
        .code(4)
        .stderr(predicate::str::contains("no mmz.yaml"));
}

#[test]
fn init_writes_a_manifest_then_refuses_to_clobber() {
    let dir = tempfile::tempdir().expect("tempdir");
    mmz(dir.path()).arg("--init").assert().success();
    let manifest = fs::read_to_string(dir.path().join("mmz.yaml")).expect("manifest written");
    assert!(manifest.contains("$schema"), "carries the schema line");
    mmz(dir.path()).arg("--init").assert().code(2);
}

#[test]
fn schema_prints_the_manifest_schema() {
    Command::cargo_bin("mmz")
        .expect("binary should build")
        .arg("--schema")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"$schema\"").and(predicate::str::contains("no_match")));
}

#[test]
fn status_reports_rule_freshness() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_project(dir.path());
    mmz(dir.path())
        .arg("--status")
        .assert()
        .success()
        .stdout(predicate::str::contains("sh").and(predicate::str::contains("never")));
}

#[test]
fn unknown_option_is_a_usage_error() {
    Command::cargo_bin("mmz")
        .expect("binary should build")
        .arg("--bogus")
        .assert()
        .code(2)
        .stderr(predicate::str::contains("unknown option"));
}

#[test]
fn reports_version() {
    Command::cargo_bin("mmz")
        .expect("binary should build")
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")));
}
