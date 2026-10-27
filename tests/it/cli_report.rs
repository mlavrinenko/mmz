//! `MMZ_REPORT`, end to end: a wrapped run writes `skipped` or `ran` to the
//! named path once it decides, so a caller whose `on_hit` prints nothing can
//! still tell a skip from a run.
//!
//! In the CLI suite for the reason `cli_disable.rs` is: setting an environment
//! variable in-process is `unsafe` in edition 2024, which this crate denies.

use std::fs;
use std::path::Path;

use predicates::prelude::predicate;

use crate::support::{mmz, write_manifest};

/// One rule over one file, with a silent `on_hit`, as a quiet gate runner sets.
fn write_project(dir: &Path) {
    write_manifest(
        dir,
        "on_hit: \"\"\nscopes:\n  src: [\"*.txt\"]\ncommands:\n  - name: sh\n    inputs: [src]\n",
    );
    fs::write(dir.join("a.txt"), b"one").expect("write input");
}

/// The wrapped command: appends a line to `ran.log`.
const APPEND: [&str; 3] = ["sh", "-c", "echo ran >> ran.log"];

fn report(dir: &Path) -> String {
    fs::read_to_string(dir.join("report")).expect("read report")
}

#[test]
fn a_run_reports_ran_and_a_skip_reports_skipped() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_project(dir.path());

    mmz(dir.path())
        .env("MMZ_REPORT", "report")
        .args(APPEND)
        .assert()
        .success()
        .stdout("")
        .stderr("");
    assert_eq!(report(dir.path()), "ran\n");

    mmz(dir.path())
        .env("MMZ_REPORT", "report")
        .args(APPEND)
        .assert()
        .success()
        .stdout("")
        .stderr("");
    assert_eq!(report(dir.path()), "skipped\n");
}

#[test]
fn a_failing_run_still_reports_ran() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_project(dir.path());

    mmz(dir.path())
        .env("MMZ_REPORT", "report")
        .args(["sh", "-c", "exit 3"])
        .assert()
        .code(3);
    assert_eq!(report(dir.path()), "ran\n");
}

#[test]
fn an_unmemoized_run_reports_ran() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_manifest(
        dir.path(),
        "strict: []\nscopes:\n  src: [\"*.txt\"]\ncommands:\n  - name: make\n    inputs: [src]\n",
    );

    mmz(dir.path())
        .env("MMZ_REPORT", "report")
        .args(["sh", "-c", "true"])
        .assert()
        .success();
    assert_eq!(report(dir.path()), "ran\n");
}

#[test]
fn a_forced_run_reports_ran() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_project(dir.path());

    mmz(dir.path()).args(APPEND).assert().success();
    mmz(dir.path())
        .env("MMZ_DISABLE", "1")
        .env("MMZ_REPORT", "report")
        .args(APPEND)
        .assert()
        .success();
    assert_eq!(report(dir.path()), "ran\n");
}

#[test]
fn an_empty_value_writes_nothing() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_project(dir.path());

    mmz(dir.path())
        .env("MMZ_REPORT", "")
        .args(APPEND)
        .assert()
        .success();
    assert!(
        !dir.path().join("report").exists(),
        "an empty MMZ_REPORT is off"
    );
}

#[test]
fn an_unwritable_report_stops_before_the_command_runs() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_project(dir.path());

    mmz(dir.path())
        .env("MMZ_REPORT", "missing/report")
        .args(APPEND)
        .assert()
        .code(8)
        .stderr(predicate::str::contains("missing/report"));
    assert!(
        !dir.path().join("ran.log").exists(),
        "the command never ran"
    );
    assert!(
        !dir.path().join(".mmz/cache").exists()
            || fs::read_dir(dir.path().join(".mmz/cache"))
                .expect("cache dir")
                .filter_map(Result::ok)
                .all(|entry| entry.path().extension().is_none_or(|ext| ext != "yaml")),
        "and nothing was recorded"
    );
}

#[test]
fn is_fresh_ignores_the_variable() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_project(dir.path());

    mmz(dir.path()).args(APPEND).assert().success();
    mmz(dir.path())
        .env("MMZ_REPORT", "report")
        .args(["--is-fresh", "sh"])
        .assert()
        .success();
    assert!(!dir.path().join("report").exists());
}
