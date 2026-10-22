use std::fs;
use std::path::Path;

use assert_cmd::Command;
use predicates::prelude::predicate;

use crate::support::{mmz, write_manifest};

/// A `sh` rule over `*.txt`, plus one such input file.
fn write_project(dir: &Path) {
    write_manifest(
        dir,
        "scopes:\n  src: [\"*.txt\"]\ncommands:\n  - name: sh\n    inputs: [src]\n",
    );
    fs::write(dir.join("a.txt"), b"one").expect("write input");
}

fn log_len(dir: &Path) -> usize {
    fs::read(dir.join("runs.log")).map_or(0, |bytes| bytes.len())
}

#[test]
#[cfg(unix)]
fn a_signal_death_exits_128_plus_the_signal_without_caching_failure() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_project(dir.path());

    for (signal, expected) in [("KILL", 137), ("TERM", 143)] {
        let before = log_len(dir.path());
        for run in 1..=2 {
            mmz(dir.path())
                .args([
                    "sh",
                    "-c",
                    &format!("printf x >> runs.log; kill -{signal} $$"),
                ])
                .assert()
                .code(expected);
            assert_eq!(
                log_len(dir.path()),
                before + run,
                "a signal death records a failure, so run {run} executes"
            );
        }
    }
}

#[test]
fn help_names_the_signal_exit_code_beside_the_commands_own() {
    Command::cargo_bin("mmz")
        .expect("binary should build")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "own exit code, or 128 plus N when the\n    command dies by signal N",
        ));
}
