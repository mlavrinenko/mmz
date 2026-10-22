//! `MMZ_DISABLE`, end to end: `1` makes a wrapped run execute instead of skip
//! and record its outcome as usual, `0` and the empty string leave mmz as it
//! was, and any other value stops the invocation before the command runs.
//!
//! These live in the CLI suite for the same reason `cli_clock.rs` does: setting
//! an environment variable in-process is `unsafe` in edition 2024 and this crate
//! denies `unsafe_code`, so a child process is the only way to drive it.

use std::fs;
use std::path::Path;

use predicates::prelude::{PredicateBooleanExt, predicate};

use crate::support::{mmz, write_manifest};

/// An arbitrary but fixed pin: 2023-11-14T22:13:20Z.
const PIN: u64 = 1_700_000_000;

/// One rule over one file, with `strict` left at its default.
fn write_project(dir: &Path) {
    write_manifest(
        dir,
        "scopes:\n  src: [\"*.txt\"]\ncommands:\n  - name: sh\n    inputs: [src]\n",
    );
    fs::write(dir.join("a.txt"), b"one").expect("write input");
}

/// The wrapped command: appends a line to `ran.log`, so how many times it ran
/// is a count of lines rather than a guess.
const APPEND: [&str; 3] = ["sh", "-c", "echo ran >> ran.log"];

/// How many times the wrapped command has run in `dir`.
fn runs(dir: &Path) -> usize {
    fs::read_to_string(dir.join("ran.log")).map_or(0, |log| log.lines().count())
}

/// The single cache record's text, however its file is named.
fn record(dir: &Path) -> String {
    let entry = fs::read_dir(dir.join(".mmz/cache"))
        .expect("cache dir")
        .filter_map(Result::ok)
        .find(|entry| entry.path().extension().is_some_and(|ext| ext == "yaml"))
        .expect("one record");
    fs::read_to_string(entry.path()).expect("read record")
}

#[test]
fn a_fresh_rule_runs_again_under_disable() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_project(dir.path());

    mmz(dir.path()).args(APPEND).assert().success();
    mmz(dir.path()).args(APPEND).assert().success();
    assert_eq!(
        runs(dir.path()),
        1,
        "without the switch the second run skips"
    );

    mmz(dir.path())
        .env("MMZ_DISABLE", "1")
        .args(APPEND)
        .assert()
        .success();
    assert_eq!(
        runs(dir.path()),
        2,
        "under MMZ_DISABLE=1 an unchanged, fresh rule still runs"
    );
}

#[test]
fn a_forced_run_re_records_so_the_gate_stays_fresh() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_project(dir.path());

    mmz(dir.path())
        .env("MMZ_NOW", PIN.to_string())
        .args(APPEND)
        .assert()
        .success();
    mmz(dir.path())
        .env("MMZ_NOW", (PIN + 100).to_string())
        .env("MMZ_DISABLE", "1")
        .args(APPEND)
        .assert()
        .success();
    assert!(
        record(dir.path()).contains(&format!("ran_at: {}", PIN + 100)),
        "the forced run writes its record, stamped from the clock like any other"
    );

    mmz(dir.path()).args(APPEND).assert().success();
    assert_eq!(
        runs(dir.path()),
        2,
        "and that record is a pass: the next unforced run skips"
    );
    mmz(dir.path())
        .args(["--is-fresh", "sh"])
        .assert()
        .success();
}

#[test]
fn a_forced_failure_is_recorded_and_propagates_its_code() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_project(dir.path());

    mmz(dir.path()).args(APPEND).assert().success();
    mmz(dir.path())
        .env("MMZ_DISABLE", "1")
        .args(["sh", "-c", "exit 3"])
        .assert()
        .code(3);
    assert!(
        record(dir.path()).contains("status: failed"),
        "a forced run records its failure exactly as an unforced one would"
    );
    mmz(dir.path()).args(["--is-fresh", "sh"]).assert().code(1);
}

#[test]
fn zero_and_the_empty_string_leave_mmz_memoizing() {
    for off in ["0", ""] {
        let dir = tempfile::tempdir().expect("tempdir");
        write_project(dir.path());

        for _ in 0..2 {
            mmz(dir.path())
                .env("MMZ_DISABLE", off)
                .args(APPEND)
                .assert()
                .success();
        }
        assert_eq!(
            runs(dir.path()),
            1,
            "MMZ_DISABLE={off:?} is off, so the second run skips"
        );
    }
}

#[test]
fn any_other_value_is_refused_before_the_command_runs() {
    for bad in ["yes", "true", "2", "01", " 1", "1 "] {
        let dir = tempfile::tempdir().expect("tempdir");
        write_project(dir.path());

        mmz(dir.path())
            .env("MMZ_DISABLE", bad)
            .args(APPEND)
            .assert()
            .code(2)
            .stderr(predicate::str::contains("MMZ_DISABLE").and(predicate::str::contains(bad)));
        assert_eq!(
            runs(dir.path()),
            0,
            "MMZ_DISABLE={bad:?} is a misconfiguration, so the command never runs"
        );
        assert!(
            !dir.path().join(".mmz/cache").exists(),
            "and nothing is recorded"
        );
    }
}

#[test]
fn matching_and_strict_behave_as_without_the_switch() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_project(dir.path());

    mmz(dir.path())
        .env("MMZ_DISABLE", "1")
        .args(["sh", "-c", "echo ran >> ran.log"])
        .assert()
        .success();
    mmz(dir.path())
        .env("MMZ_DISABLE", "1")
        .args(["unmatched", "--flag"])
        .assert()
        .code(3);
    assert_eq!(
        runs(dir.path()),
        1,
        "a command no rule matches is still refused, so forcing is not a way round `strict`"
    );
}

#[test]
fn a_forced_run_still_answers_to_its_declared_outputs() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_manifest(
        dir.path(),
        "scopes:\n  src: [\"*.txt\"]\ncommands:\n  - name: sh\n    inputs: [src]\n    outputs: [out.bin]\n",
    );
    fs::write(dir.path().join("a.txt"), b"one").expect("write input");

    mmz(dir.path())
        .env("MMZ_DISABLE", "1")
        .args(["sh", "-c", "exit 0"])
        .assert()
        .code(5)
        .stderr(predicate::str::contains("out.bin"));
    assert!(
        !dir.path().join(".mmz/cache").exists(),
        "a run that skipped its output records nothing, forced or not"
    );

    mmz(dir.path())
        .env("MMZ_DISABLE", "1")
        .args(["sh", "-c", "touch out.bin"])
        .assert()
        .success();
    assert!(
        record(dir.path()).contains("out.bin"),
        "and one that produced it records the artifact, as an unforced run does"
    );
}

#[test]
fn is_fresh_and_status_ignore_the_variable() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_project(dir.path());
    mmz(dir.path()).args(APPEND).assert().success();

    for value in ["1", "0", "yes"] {
        mmz(dir.path())
            .env("MMZ_DISABLE", value)
            .args(["--is-fresh", "sh"])
            .assert()
            .success();
        mmz(dir.path())
            .env("MMZ_DISABLE", value)
            .arg("--status")
            .assert()
            .success()
            .stdout(predicate::str::contains("fresh"));
    }
    assert_eq!(runs(dir.path()), 1, "neither action runs the command");
}
