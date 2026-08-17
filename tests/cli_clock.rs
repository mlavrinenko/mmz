//! `MMZ_NOW`, end to end: the pinned clock is what a record stamps and what
//! `--status` ages against, and a malformed pin stops the invocation instead of
//! quietly falling back to the system clock.
//!
//! These live in the CLI suite rather than beside `src/clock.rs` because setting
//! an environment variable in-process is `unsafe` in edition 2024 and this crate
//! denies `unsafe_code`. A child process is the only honest way to drive the
//! variable, which is also how every real caller sets it.

use std::fs;
use std::path::Path;

use predicates::prelude::predicate;

mod support;
use support::{mmz, write_manifest};

/// An arbitrary but fixed pin: 2023-11-14T22:13:20Z.
const PIN: u64 = 1_700_000_000;

/// The simplest memoizable project: one rule over one file.
fn write_project(dir: &Path) {
    write_manifest(
        dir,
        "scopes:\n  src: [\"*.txt\"]\ncommands:\n  - name: sh\n    inputs: [src]\n",
    );
    fs::write(dir.join("a.txt"), b"one").expect("write input");
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
fn a_pinned_clock_stamps_the_record_and_ages_the_table() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_project(dir.path());

    mmz(dir.path())
        .env("MMZ_NOW", PIN.to_string())
        .args(["sh", "-c", "exit 0"])
        .assert()
        .success();
    assert!(
        record(dir.path()).contains(&format!("ran_at: {PIN}")),
        "the record stamps the pin, so a captured record is byte-reproducible"
    );

    // The fixture the system clock could never produce: a record whose age is
    // stated rather than accidental.
    mmz(dir.path())
        .env("MMZ_NOW", (PIN + 3 * 3600).to_string())
        .arg("--status")
        .assert()
        .success()
        .stdout(predicate::str::contains("3h ago"));

    mmz(dir.path())
        .env("MMZ_NOW", PIN.to_string())
        .arg("--status")
        .assert()
        .success()
        .stdout(predicate::str::contains("0s ago"));
}

#[test]
fn a_malformed_pin_is_refused_before_the_command_runs() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_project(dir.path());

    mmz(dir.path())
        .env("MMZ_NOW", "yesterday")
        .args(["sh", "-c", "printf ran > ran.marker"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("MMZ_NOW"));
    assert!(
        !dir.path().join("ran.marker").exists(),
        "a misconfigured clock stops the invocation; it never runs the command and stamps it wrong"
    );
    assert!(
        !dir.path().join(".mmz/cache").exists(),
        "and nothing is recorded"
    );
}

#[test]
fn both_status_renderings_refuse_a_malformed_pin() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_project(dir.path());

    for arg in ["--status", "--status=json"] {
        mmz(dir.path())
            .env("MMZ_NOW", "")
            .arg(arg)
            .assert()
            .code(2)
            .stderr(predicate::str::contains("MMZ_NOW"));
    }
}

#[test]
fn an_unset_pin_leaves_mmz_on_the_system_clock() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_project(dir.path());

    mmz(dir.path())
        .env_remove("MMZ_NOW")
        .args(["sh", "-c", "exit 0"])
        .assert()
        .success();
    let stamped: u64 = record(dir.path())
        .lines()
        .find_map(|line| line.strip_prefix("ran_at: ")?.trim().parse().ok())
        .expect("ran_at is a number");
    assert!(
        stamped > 1_600_000_000,
        "unset, mmz stamps a real time rather than a pin left over from a test"
    );

    mmz(dir.path())
        .env_remove("MMZ_NOW")
        .arg("--status")
        .assert()
        .success()
        .stdout(predicate::str::contains("ago"));
}
