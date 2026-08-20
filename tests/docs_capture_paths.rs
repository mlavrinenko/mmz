//! The docs-capture leak gate, exercised on both of the passes it makes.
//!
//! `.just/scripts/check-capture-paths.sh` is what stops a docs capture from
//! shipping the temp directory `www/generate.sh` ran the fixture in. A gate
//! that has only ever run green proves nothing about what it would catch, and
//! the one real leak it was written against stopped existing the moment it was
//! normalized — so the failing side is pinned here instead of being observed
//! once by hand.
//!
//! A shell gate tested from the Rust suite is a category mix, and deliberate:
//! `just test` is a `just check` arm, so this is the only harness in the repo
//! that runs the assertions below without anyone remembering to.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::TempDir;

/// Runs the gate over `dir`, passing each of `literals` as a directory the
/// caller claims this build created.
fn check(dir: &Path, literals: &[&str]) -> Output {
    let script =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".just/scripts/check-capture-paths.sh");
    Command::new("bash")
        .arg(script)
        .arg(dir)
        .args(literals)
        .output()
        .expect("gate should run")
}

/// A directory holding one capture with the given body.
fn capture(body: &str) -> TempDir {
    let dir = tempfile::tempdir().expect("temp dir");
    fs::write(dir.path().join("status-json.txt"), body).expect("write capture");
    dir
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

#[test]
fn passes_when_no_capture_names_a_temp_directory() {
    let dir = capture("{\n  \"manifest\": \"./.mmz/config.yaml\"\n}\n");
    let out = check(dir.path(), &[]);
    assert!(
        out.status.success(),
        "clean captures should pass: {}",
        stderr(&out)
    );
}

/// The shape pass. This is the leak as it actually shipped — a `--status=json`
/// capture naming the manifest by the absolute path it had inside that build's
/// `mktemp -d`.
#[test]
fn fails_on_an_mktemp_path_the_caller_did_not_pass_down() {
    let dir = capture("{\n  \"manifest\": \"/tmp/tmp.BIg78u4JqB/demo/.mmz/config.yaml\"\n}\n");
    let out = check(dir.path(), &[]);
    assert!(!out.status.success(), "an mktemp path should fail the gate");
    let err = stderr(&out);
    assert!(
        err.contains("status-json.txt"),
        "should name the file: {err}"
    );
    assert!(
        err.contains("/tmp/tmp.BIg78u4JqB"),
        "should quote the path: {err}"
    );
}

/// The literal pass, which is the honest half: a `$TMPDIR` pointing anywhere
/// but `/tmp` produces a path the shape pass cannot recognize, and only the
/// caller knows what it was.
#[test]
fn fails_on_a_caller_supplied_path_outside_the_mktemp_shape() {
    let leaked = "/var/folders/qz/mmz-fixture-9f2";
    let dir = capture(&format!("manifest: {leaked}/demo/.mmz/config.yaml\n"));

    let unarmed = check(dir.path(), &[]);
    assert!(
        unarmed.status.success(),
        "the shape pass alone cannot see this path, which is the point"
    );

    let out = check(dir.path(), &[leaked]);
    assert!(
        !out.status.success(),
        "the caller's own path should fail the gate"
    );
    assert!(
        stderr(&out).contains(leaked),
        "should quote the path: {}",
        stderr(&out)
    );
}

/// `/tmp` is a perfectly ordinary thing for a capture to mention: the gate
/// matches `mktemp`'s naming, not the temp root, so a doc example survives it.
#[test]
fn tolerates_a_capture_that_merely_mentions_the_temp_root() {
    let dir = capture("wrote /tmp/report.json\nreading /tmp/orders/\n");
    let out = check(dir.path(), &[]);
    assert!(
        out.status.success(),
        "plain /tmp is not a leak: {}",
        stderr(&out)
    );
}
