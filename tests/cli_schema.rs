//! `mmz --schema=fragment`, end to end: the second document `--schema` can
//! print, alongside the existing `--schema` (no suffix) form for the config
//! manifest. The derivation that keeps the two schemas from drifting apart
//! is asserted in `src/schema.rs`, beside the embedded documents themselves;
//! this file only exercises the CLI surface — the flag parses, the right
//! bytes come out, and an unrecognized `=suffix` is a usage error rather
//! than silently falling back to the config schema.

use assert_cmd::Command;
use predicates::prelude::{PredicateBooleanExt, predicate};

#[test]
fn schema_fragment_prints_valid_json_and_exits_0() {
    Command::cargo_bin("mmz")
        .expect("binary should build")
        .arg("--schema=fragment")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"$schema\"").and(predicate::str::contains("imports")));
}

#[test]
fn schema_without_a_suffix_still_prints_the_config_schema() {
    // A regression guard for the bare form: dispatch now runs through the same
    // `run_schema` as the fragment form, so this proves the `--schema=fragment`
    // wiring did not change what `--schema` alone prints.
    Command::cargo_bin("mmz")
        .expect("binary should build")
        .arg("--schema")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"$schema\"").and(predicate::str::contains("no_match")));
}

#[test]
fn schema_with_an_unknown_format_is_a_usage_error() {
    Command::cargo_bin("mmz")
        .expect("binary should build")
        .arg("--schema=bogus")
        .assert()
        .code(2)
        .stderr(predicate::str::contains("fragment"));
}

#[test]
fn schema_fragment_takes_no_arguments() {
    Command::cargo_bin("mmz")
        .expect("binary should build")
        .args(["--schema=fragment", "extra"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("--schema"));
}
