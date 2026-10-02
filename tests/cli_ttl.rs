//! A rule's `ttl`, end to end: a successful record whose age reaches the ttl is
//! not fresh, in every surface that asks — the wrapped run re-executes,
//! `--is-fresh` refuses, and `--status` reports `expired` rather than `stale`.
//!
//! Time is driven through `MMZ_NOW` (see `src/clock.rs`) rather than slept
//! through: the variable is the process clock mmz ages a record against, so a
//! fixture can state a record's age instead of waiting for it.

use std::fs;
use std::path::Path;

use predicates::prelude::{PredicateBooleanExt, predicate};

mod support;
use support::{mmz, write_manifest};

/// An arbitrary but fixed pin: 2023-11-14T22:13:20Z.
const PIN: u64 = 1_700_000_000;
const MINUTE: u64 = 60;
const HOUR: u64 = 60 * MINUTE;
const DAY: u64 = 24 * HOUR;

/// One rule over one file, expiring an hour after it last passed.
fn write_project(dir: &Path) {
    write_manifest(
        dir,
        "scopes:\n  src: [\"*.txt\"]\ncommands:\n  - name: sh\n    inputs: [src]\n    ttl: 1h\n",
    );
    fs::write(dir.join("a.txt"), b"one").expect("write input");
}

fn log_len(dir: &Path) -> usize {
    fs::read(dir.join("runs.log")).map_or(0, |bytes| bytes.len())
}

/// Runs the logging command under mmz at `now`.
fn run_at(dir: &Path, now: u64) {
    mmz(dir)
        .env("MMZ_NOW", now.to_string())
        .args(["sh", "-c", "printf x >> runs.log"])
        .assert()
        .success();
}

#[test]
fn a_pass_reruns_once_its_ttl_has_elapsed() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_project(dir.path());

    run_at(dir.path(), PIN);
    assert_eq!(log_len(dir.path()), 1, "first run executes");

    run_at(dir.path(), PIN + 59 * MINUTE);
    assert_eq!(log_len(dir.path()), 1, "inside the ttl it is a cache hit");

    run_at(dir.path(), PIN + HOUR);
    assert_eq!(log_len(dir.path()), 2, "at the ttl the pass has expired");

    run_at(dir.path(), PIN + HOUR + 30 * MINUTE);
    assert_eq!(
        log_len(dir.path()),
        2,
        "the re-run recorded a new pass, aged from its own run"
    );
}

#[test]
fn is_fresh_refuses_an_expired_pass_and_says_why() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_project(dir.path());
    run_at(dir.path(), PIN);

    mmz(dir.path())
        .env("MMZ_NOW", (PIN + 59 * MINUTE).to_string())
        .args(["--is-fresh", "--", "sh"])
        .assert()
        .success();

    mmz(dir.path())
        .env("MMZ_NOW", (PIN + 2 * HOUR).to_string())
        .args(["--is-fresh", "--", "sh"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("`sh` is expired"))
        .stderr(predicate::str::contains("ttl `1h`"))
        .stderr(predicate::str::contains("re-run each listed command"));
}

#[test]
fn status_reports_expired_apart_from_stale() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_project(dir.path());
    run_at(dir.path(), PIN);
    let later = (PIN + 2 * HOUR).to_string();

    mmz(dir.path())
        .env("MMZ_NOW", &later)
        .arg("--status")
        .assert()
        .success()
        .stdout(predicate::str::contains("expired"))
        .stdout(predicate::str::contains("2h ago"));

    let json = mmz(dir.path())
        .env("MMZ_NOW", &later)
        .arg("--status=json")
        .output()
        .expect("status json");
    let report: serde_json::Value = serde_json::from_slice(&json.stdout).expect("valid json");
    let rule = report.pointer("/rules/0").expect("one rule in the report");
    assert_eq!(rule.get("state"), Some(&serde_json::json!("expired")));
    assert_eq!(rule.get("ttl"), Some(&serde_json::json!("1h")));
    assert_eq!(
        rule.get("expires_at"),
        Some(&serde_json::json!(PIN + HOUR)),
        "expires_at is the recorded run plus the ttl"
    );

    // An input change outranks age: the record would re-run whatever its age,
    // and `stale` is the fact that says where to look.
    fs::write(dir.path().join("a.txt"), b"two").expect("rewrite input");
    mmz(dir.path())
        .env("MMZ_NOW", &later)
        .arg("--status")
        .assert()
        .success()
        .stdout(predicate::str::contains("stale"))
        .stdout(predicate::str::contains("expired").not());
}

#[test]
fn a_rule_without_a_ttl_never_expires() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_manifest(
        dir.path(),
        "scopes:\n  src: [\"*.txt\"]\ncommands:\n  - name: sh\n    inputs: [src]\n",
    );
    fs::write(dir.path().join("a.txt"), b"one").expect("write input");
    run_at(dir.path(), PIN);
    run_at(dir.path(), PIN + 365 * DAY);
    assert_eq!(log_len(dir.path()), 1, "a year later it is still a hit");

    let json = mmz(dir.path())
        .env("MMZ_NOW", (PIN + 365 * DAY).to_string())
        .arg("--status=json")
        .output()
        .expect("status json");
    let text = String::from_utf8(json.stdout).expect("utf-8");
    assert!(text.contains("\"fresh\""), "{text}");
    assert!(
        !text.contains("ttl") && !text.contains("expires_at"),
        "a rule with no ttl adds no keys to the report: {text}"
    );
}

#[test]
fn a_ttl_rule_may_declare_no_inputs_at_all() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_manifest(dir.path(), "commands:\n  - name: sh\n    ttl: 1d\n");

    run_at(dir.path(), PIN);
    run_at(dir.path(), PIN + 23 * HOUR);
    assert_eq!(log_len(dir.path()), 1, "a time-only rule is memoized");

    mmz(dir.path())
        .env("MMZ_NOW", (PIN + 23 * HOUR).to_string())
        .args(["--is-fresh", "--", "sh"])
        .assert()
        .success();

    run_at(dir.path(), PIN + DAY);
    assert_eq!(log_len(dir.path()), 2, "and re-runs once its ttl elapses");
}

#[test]
fn a_ttl_does_not_excuse_declared_inputs_that_resolve_to_nothing() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_manifest(
        dir.path(),
        "scopes:\n  src: [\"*.txt\"]\ncommands:\n  - name: sh\n    inputs: [src]\n    ttl: 1d\n",
    );
    mmz(dir.path())
        .env("MMZ_NOW", PIN.to_string())
        .args(["sh", "-c", "exit 0"])
        .assert()
        .code(3)
        .stderr(predicate::str::contains("no input"));
}

#[test]
fn a_rule_with_no_inputs_and_no_ttl_is_still_refused() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_manifest(dir.path(), "commands:\n  - name: sh\n");
    mmz(dir.path())
        .args(["sh", "-c", "exit 0"])
        .assert()
        .code(3);
}

#[test]
fn a_malformed_ttl_is_a_load_error() {
    for bad in [
        "0h",
        "0s",
        "10",
        "h",
        "1h30m",
        "1.5h",
        "-1h",
        "+1h",
        "01h",
        "1H",
        "1 h",
        "1w",
        "\"\"",
        "99999999999999999999d",
        "999999999999999999d",
    ] {
        let dir = tempfile::tempdir().expect("tempdir");
        write_manifest(
            dir.path(),
            &format!("commands:\n  - name: sh\n    ttl: {bad}\n"),
        );
        mmz(dir.path())
            .args(["sh", "-c", "exit 0"])
            .assert()
            .code(4)
            .stderr(predicate::str::contains("ttl"));
    }
}

#[test]
fn dump_config_echoes_the_ttl_as_written() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_project(dir.path());
    mmz(dir.path())
        .arg("--dump-config")
        .assert()
        .success()
        .stdout(predicate::str::contains("    ttl: 1h\n"));
    mmz(dir.path())
        .arg("--dump-config=json")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"ttl\": \"1h\""));
}
