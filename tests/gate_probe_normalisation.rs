//! The gate probes' JSON normalisation, from both ends.
//!
//! Every gate rule in `.mmz/conf.d/` pins its recipe body through a probe that
//! selects out of `just --dump --dump-format json`. The digest is a hash of
//! that selection's bytes, so any byte the renderer is free to move — object
//! key order, most of all — is an input the probe never meant to declare.
//!
//! It bit once for real: just 1.43.1 on a host PATH and just 1.51.0 in the dev
//! shell render byte-different, content-identical JSON, so a `mt done` outside
//! `nix develop` read every gate stale against a worktree whose checks had just
//! passed. `jq -S` sorts keys and removes that degree of freedom.
//!
//! The honest test runs one probe under two just versions and asserts one
//! digest, which needs two just versions on hand. These are the two halves that
//! do not: that mmz's digest really is order-stable once a probe sorts, and
//! that every probe this repo ships actually sorts.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

mod support;
use support::{mmz, write_manifest};

/// Longest a probe may take before the test calls it hung rather than slow.
const PATIENCE: Duration = Duration::from_secs(30);

/// The same object, rendered with its keys in the two orders a `just` bump
/// might pick between. Content-identical; byte-different.
const KEYS_ONE: &str = r#"{"body":"cargo clippy","doc":"lint","name":"clippy"}"#;
const KEYS_TWO: &str = r#"{"name":"clippy","doc":"lint","body":"cargo clippy"}"#;

/// A project whose rule takes one probe piping `dump.json` through `jq` with
/// the given flags, plus a command that appends a byte per real execution.
fn project(flags: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    write_manifest(
        dir.path(),
        &format!(
            concat!(
                "probes:\n  recipe:\n    run: jq {} '.' dump.json\n",
                "commands:\n  - name: sh\n    inputs: [recipe]\n",
            ),
            flags
        ),
    );
    dir
}

fn build(dir: &Path) {
    mmz(dir)
        .timeout(PATIENCE)
        .args(["sh", "-c", "printf x >> runs.log"])
        .assert()
        .success();
}

fn runs(dir: &Path) -> usize {
    fs::read(dir.join("runs.log")).map_or(0, |bytes| bytes.len())
}

/// Records a pass against one key order, then re-reads against the other.
fn runs_after_reorder(flags: &str) -> usize {
    let dir = project(flags);
    fs::write(dir.path().join("dump.json"), KEYS_ONE).expect("write dump");
    build(dir.path());

    fs::write(dir.path().join("dump.json"), KEYS_TWO).expect("reorder dump");
    build(dir.path());

    runs(dir.path())
}

#[test]
fn a_sorting_probe_holds_its_digest_across_a_reordered_object() {
    assert_eq!(
        runs_after_reorder("-S -e -c"),
        1,
        "`jq -S` sorts keys, so the same content in a different order is the \
         same digest and the rule stays fresh"
    );
}

#[test]
fn an_unsorted_probe_busts_on_key_order_alone() {
    // The control. Without it the test above passes for the wrong reason if
    // mmz ever started normalising probe output itself — which it must not,
    // since it hashes bytes it cannot parse.
    assert_eq!(
        runs_after_reorder("-e -c"),
        2,
        "without `-S` the digest tracks key order, which is the bug this \
         normalisation exists to remove"
    );
}

#[test]
fn every_gate_probe_that_pipes_through_jq_sorts_its_keys() {
    let conf = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".mmz/conf.d");
    let mut checked = 0_usize;

    for entry in fs::read_dir(&conf).expect("conf.d should exist") {
        let path = entry.expect("read conf.d entry").path();
        if path.extension().is_none_or(|ext| ext != "yaml") {
            continue;
        }
        let body = fs::read_to_string(&path).expect("read fragment");
        for line in body.lines() {
            let Some(run) = line.trim().strip_prefix("run:") else {
                continue;
            };
            if !run.contains("jq ") {
                continue;
            }
            checked += 1;
            assert!(
                run.contains("-S"),
                "{}: probe `{}` hashes unsorted JSON, so its digest depends on \
                 the key order of whichever renderer is on PATH",
                path.display(),
                run.trim()
            );
        }
    }

    assert!(
        checked >= 11,
        "expected the gate probes to be found and checked, saw {checked} — \
         the scan above has stopped matching the fragments"
    );
}
