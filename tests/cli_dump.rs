//! `mmz --dump-config` and `--dump-config=json`, end to end: the flags parse,
//! the right bytes come out, an unrecognized `=suffix` is a usage error, and
//! an invalid manifest exits 4 with the merge error on stderr and nothing on
//! stdout. Provenance correctness (which file a scope, probe or command came
//! from) is unit-tested against the library directly in `src/dump_tests.rs`;
//! this file only exercises the CLI surface, the way `cli_schema.rs` does for
//! `--schema`.

use predicates::prelude::{PredicateBooleanExt, predicate};

mod support;
use support::{mmz, write_manifest};

#[test]
fn dump_config_prints_sources_and_entries() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_manifest(
        dir.path(),
        "scopes:\n  src: [\"*.txt\"]\ncommands:\n  - name: sh\n    inputs: [src]\n",
    );

    mmz(dir.path())
        .arg("--dump-config")
        .assert()
        .success()
        .stdout(
            predicate::str::contains("sources:\n  1  .mmz/config.yaml")
                .and(predicate::str::contains("src:"))
                .and(predicate::str::contains("sh:")),
        );
}

#[test]
fn dump_config_json_prints_sources_and_a_source_per_entry() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_manifest(
        dir.path(),
        "scopes:\n  src: [\"*.txt\"]\ncommands:\n  - name: sh\n    inputs: [src]\n",
    );

    mmz(dir.path())
        .arg("--dump-config=json")
        .assert()
        .success()
        .stdout(
            predicate::str::contains("\"sources\"")
                .and(predicate::str::contains("\".mmz/config.yaml\""))
                .and(predicate::str::contains("\"source\"")),
        );
}

#[test]
fn dump_config_with_an_unknown_format_is_a_usage_error() {
    mmz(&std::env::temp_dir())
        .arg("--dump-config=bogus")
        .assert()
        .code(2)
        .stderr(predicate::str::contains("--dump-config"));
}

#[test]
fn dump_config_takes_no_arguments() {
    mmz(&std::env::temp_dir())
        .args(["--dump-config", "extra"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("--dump-config"));
}

#[test]
fn an_invalid_manifest_exits_4_with_no_partial_dump() {
    let dir = tempfile::tempdir().expect("tempdir");
    // The rule's `inputs:` names `ghost`, which is declared as neither a
    // scope nor a probe — a manifest that parses fine but fails validation of
    // the merged model.
    write_manifest(dir.path(), "commands:\n  - name: sh\n    inputs: [ghost]\n");

    mmz(dir.path())
        .arg("--dump-config")
        .assert()
        .code(4)
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("ghost"));

    mmz(dir.path())
        .arg("--dump-config=json")
        .assert()
        .code(4)
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("ghost"));
}
