//! An input a rule declared but mmz cannot read must name the file and exit 8.
//!
//! The failure this pins was a pathless `mmz: i/o error: No such file or
//! directory (os error 2)` at exit 70 — an unreadable input and a bug in mmz
//! reported as the same number, with the one fact needed to act on either
//! (which file) dropped on the floor. See
//! `tasks/mmz-an-i-o-error-while-hashing-an-input-names-no-path.typ`.
//!
//! Unix-only: the deterministic way to make a resolved input unreadable is a
//! mode the walker still lists and `File::open` still refuses. The other half
//! of the fix — an input that vanishes between the walk and the hash — is a
//! race no CLI test can stage, and is covered against `hash_each` directly in
//! `src/hashing.rs`.
#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use predicates::prelude::{PredicateBooleanExt, predicate};

mod support;
use support::{mmz, write_manifest};

/// Writes a project whose `sh` rule hashes every `*.txt`, with one input the
/// running user cannot read.
///
/// Returns false when the mode does not bite — running as root, where there is
/// no refusal to observe and nothing to assert.
fn write_locked_project(dir: &Path) -> bool {
    write_manifest(
        dir,
        "scopes:\n  src: [\"*.txt\"]\ncommands:\n  - name: sh\n    inputs: [src]\n",
    );
    let path = dir.join("locked.txt");
    fs::write(&path, b"one").expect("write input");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o000)).expect("chmod");
    fs::read(&path).is_err()
}

#[test]
fn an_unreadable_input_names_the_file_and_exits_8() {
    let dir = tempfile::tempdir().expect("tempdir");
    if !write_locked_project(dir.path()) {
        return;
    }

    mmz(dir.path())
        .args(["sh", "-c", "printf x >> ran.log"])
        .assert()
        .code(8)
        .stderr(
            predicate::str::contains("locked.txt").and(predicate::str::contains("i/o error").not()),
        );

    assert!(
        !dir.path().join("ran.log").exists(),
        "hashing fails before the wrapped command is spawned"
    );
    assert!(
        !dir.path().join(".mmz/cache").exists(),
        "an input mmz could not read leaves no record behind"
    );
}

#[test]
fn a_read_only_action_over_an_unreadable_input_names_it_too() {
    let dir = tempfile::tempdir().expect("tempdir");
    if !write_locked_project(dir.path()) {
        return;
    }

    // `--status` hashes the same inputs the wrapped run would, so it must
    // answer the same way rather than falling back to the pathless variant.
    mmz(dir.path())
        .arg("--status")
        .assert()
        .code(8)
        .stderr(predicate::str::contains("locked.txt"));
}
