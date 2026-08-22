//! The files mmz reads and writes for itself — the manifest, `--init`'s
//! scaffold, the cache — must name themselves when the filesystem refuses.
//!
//! 0.8.1 closed this for a rule's declared inputs. The same bare `?`-on-io
//! shape survived at four more sites, all reporting through `Error::Io`'s
//! pathless `i/o error: {0}` at exit 70: an unreadable manifest, its second
//! read for `--dump-config`, `--init`'s writes, and the cache sweep. See
//! `tasks/mmz-an-i-o-failure-outside-hashing-still-names-no-path.typ`.
//!
//! Unix-only, for the reason `cli_unreadable_inputs.rs` is: a mode is the
//! deterministic way to make a path the tool has already found unreadable.
#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use predicates::prelude::{PredicateBooleanExt, predicate};

mod support;
use support::{mmz, write_manifest};

/// Sets `path`'s mode, returning false when it does not bite — running as
/// root, where there is no refusal to observe and nothing to assert.
fn lock(path: &Path, mode: u32, still_denied: impl Fn() -> bool) -> bool {
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).expect("chmod");
    still_denied()
}

/// Restores a mode so `TempDir`'s own cleanup can remove the tree.
fn unlock(path: &Path) {
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("chmod back");
}

#[test]
fn a_manifest_that_cannot_be_read_names_it_and_exits_4() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_manifest(dir.path(), "commands:\n  - name: sh\n");
    let config = dir.path().join(".mmz/config.yaml");
    if !lock(&config, 0o000, || fs::read(&config).is_err()) {
        return;
    }

    // 4 is what the table already promises: "mmz will not memoize against a
    // manifest it could not read or could not validate" — not 70, which asks
    // for a bug report about a permission bit.
    mmz(dir.path())
        .args(["sh", "-c", "exit 0"])
        .assert()
        .code(4)
        .stderr(
            predicate::str::contains("config.yaml")
                .and(predicate::str::contains("i/o error").not()),
        );

    unlock(&config);
}

#[test]
fn a_read_only_action_over_an_unreadable_manifest_answers_alike() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_manifest(dir.path(), "commands:\n  - name: sh\n");
    let config = dir.path().join(".mmz/config.yaml");
    if !lock(&config, 0o000, || fs::read(&config).is_err()) {
        return;
    }

    // `--dump-config` reads the manifest a second time for its policy keys, so
    // it is the other route to the same file and must not answer differently.
    mmz(dir.path())
        .arg("--dump-config")
        .assert()
        .code(4)
        .stderr(predicate::str::contains("config.yaml"));

    unlock(&config);
}

#[test]
fn an_init_that_cannot_write_names_the_path_and_exits_8() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().to_path_buf();
    if !lock(&root, 0o555, || fs::create_dir(root.join("probe")).is_err()) {
        unlock(&root);
        return;
    }

    mmz(&root)
        .arg("--init")
        .assert()
        .code(8)
        .stderr(predicate::str::contains(".mmz").and(predicate::str::contains("i/o error").not()));

    unlock(&root);
}

#[test]
fn a_prune_that_cannot_remove_a_record_names_it_and_exits_8() {
    let dir = tempfile::tempdir().expect("tempdir");
    let base = dir.path();
    write_manifest(
        base,
        "scopes:\n  src: [\"*.txt\"]\ncommands:\n  - name: sh\n    inputs: [src]\n",
    );
    fs::write(base.join("a.txt"), b"one").expect("write input");
    mmz(base).args(["sh", "-c", "exit 0"]).assert().success();

    // Drop the rule so its record is an orphan the sweep will try to delete,
    // then take write permission off the directory holding it.
    write_manifest(base, "scopes:\n  src: [\"*.txt\"]\ncommands: []\n");
    let cache = base.join(".mmz/cache");
    if !lock(&cache, 0o555, || {
        fs::write(cache.join("probe"), b"x").is_err()
    }) {
        unlock(&cache);
        return;
    }

    mmz(base).arg("--prune").assert().code(8).stderr(
        predicate::str::contains(".mmz/cache").and(predicate::str::contains("i/o error").not()),
    );

    unlock(&cache);
}
