//! `exclude:` on a scope and on a rule, end to end: an excluded path is never
//! listed in `--status=json`, never hashed, and never a `{file:scope}` entry,
//! while a manifest with no `exclude` resolves exactly as it did before.
//!
//! The case that asked for the key: a gate globbing `examples/**` hashed a
//! cache tree another gate deleted mid-hash. That race cannot be staged here,
//! so a file mmz cannot read stands in for it. Both fail the same way, at the
//! hash, and an excluded path mmz never opens can do neither.

use std::fs;
use std::path::Path;

use predicates::prelude::{PredicateBooleanExt, predicate};

use crate::support::{mmz, write_manifest};

/// A tracked source and a cache file under it, both inside `examples/**`.
fn write_tree(dir: &Path) {
    fs::create_dir_all(dir.join("examples/demo/.cache")).expect("mkdir cache");
    fs::write(dir.join("examples/demo/task.typ"), b"task").expect("write source");
    fs::write(dir.join("examples/demo/.cache/blob.bin"), b"blob").expect("write cache");
}

/// The input paths `--status=json` reports for the first rule.
fn listed_inputs(dir: &Path) -> Vec<String> {
    let out = mmz(dir)
        .arg("--status=json")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let report: serde_json::Value = serde_json::from_slice(&out).expect("status json");
    report
        .pointer("/rules/0/inputs")
        .and_then(serde_json::Value::as_array)
        .expect("inputs array")
        .iter()
        .filter_map(|entry| entry.get("path").and_then(serde_json::Value::as_str))
        .map(str::to_owned)
        .collect()
}

fn run_len(dir: &Path) -> usize {
    fs::read(dir.join("runs.log")).map_or(0, |bytes| bytes.len())
}

fn run(dir: &Path) {
    mmz(dir)
        .args(["sh", "-c", "printf x >> runs.log"])
        .assert()
        .success();
}

#[test]
fn a_manifest_without_exclude_lists_every_match() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_tree(dir.path());
    write_manifest(
        dir.path(),
        "scopes:\n  examples: [\"examples/**\"]\ncommands:\n  - name: sh\n    inputs: [examples]\n",
    );

    assert_eq!(
        listed_inputs(dir.path()),
        vec![
            "examples/demo/.cache/blob.bin".to_owned(),
            "examples/demo/task.typ".to_owned(),
        ],
        "no `exclude`, so the cache tree is an input exactly as before"
    );
}

#[test]
fn a_scope_exclude_inside_a_recursive_glob_leaves_the_tree_out() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_tree(dir.path());
    write_manifest(
        dir.path(),
        concat!(
            "scopes:\n",
            "  examples:\n",
            "    globs: [\"examples/**\"]\n",
            "    exclude: [\"**/.cache\"]\n",
            "commands:\n  - name: sh\n    inputs: [examples]\n",
        ),
    );

    assert_eq!(
        listed_inputs(dir.path()),
        vec!["examples/demo/task.typ".to_owned()],
        "a directory matching `exclude` takes everything under it out"
    );

    run(dir.path());
    fs::write(dir.path().join("examples/demo/.cache/blob.bin"), b"moved").expect("rewrite");
    fs::write(dir.path().join("examples/demo/.cache/new.bin"), b"new").expect("add");
    run(dir.path());
    assert_eq!(
        run_len(dir.path()),
        1,
        "moving the excluded tree is not an input change"
    );

    fs::write(dir.path().join("examples/demo/task.typ"), b"edited").expect("edit");
    run(dir.path());
    assert_eq!(run_len(dir.path()), 2, "the rest of the glob still counts");
}

#[test]
fn a_scope_exclude_leaves_its_sibling_scopes_alone() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_tree(dir.path());
    write_manifest(
        dir.path(),
        concat!(
            "scopes:\n",
            "  examples:\n",
            "    globs: [\"examples/**\"]\n",
            "    exclude: [\"**/.cache\"]\n",
            "  blob: [\"examples/*/.cache/*.bin\"]\n",
            "commands:\n  - name: sh\n    inputs: [examples, blob]\n",
        ),
    );

    assert_eq!(
        listed_inputs(dir.path()),
        vec![
            "examples/demo/.cache/blob.bin".to_owned(),
            "examples/demo/task.typ".to_owned(),
        ],
        "the sibling scope names the cache file itself, and keeps it"
    );
}

#[test]
fn a_rule_exclude_leaves_a_path_out_of_every_scope() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_tree(dir.path());
    fs::write(dir.path().join("notes.bin"), b"notes").expect("write");
    write_manifest(
        dir.path(),
        concat!(
            "scopes:\n",
            "  examples: [\"examples/**\"]\n",
            "  notes: [\"notes.bin\"]\n",
            "commands:\n",
            "  - name: sh\n",
            "    inputs: [examples, notes]\n",
            "    exclude: [\"**/*.bin\"]\n",
        ),
    );

    assert_eq!(
        listed_inputs(dir.path()),
        vec!["examples/demo/task.typ".to_owned()],
        "the rule's `exclude` applies to globs and literals of every scope it names"
    );
}

#[test]
fn an_excluded_literal_is_left_out_too() {
    let dir = tempfile::tempdir().expect("tempdir");
    fs::write(dir.path().join("a.txt"), b"a").expect("write");
    fs::write(dir.path().join("b.txt"), b"b").expect("write");
    write_manifest(
        dir.path(),
        concat!(
            "scopes:\n",
            "  pair:\n",
            "    globs: [\"a.txt\", \"b.txt\"]\n",
            "    exclude: [\"b.txt\"]\n",
            "commands:\n  - name: sh\n    inputs: [pair]\n",
        ),
    );

    assert_eq!(listed_inputs(dir.path()), vec!["a.txt".to_owned()]);
}

#[test]
fn an_excluded_file_is_no_parametric_target() {
    let dir = tempfile::tempdir().expect("tempdir");
    fs::create_dir_all(dir.path().join("src/gen")).expect("mkdir");
    fs::write(dir.path().join("src/a.rs"), b"a").expect("write");
    fs::write(dir.path().join("src/gen/b.rs"), b"b").expect("write");
    write_manifest(
        dir.path(),
        concat!(
            "scopes:\n",
            "  targets:\n",
            "    globs: [\"src/**/*.rs\"]\n",
            "    exclude: [\"src/gen\"]\n",
            "commands:\n  - name: \"sh -c true sh {targets}\"\n",
        ),
    );

    mmz(dir.path()).arg("--status").assert().success().stdout(
        predicate::str::contains("src/a.rs").and(predicate::str::contains("src/gen/b.rs").not()),
    );
}

#[test]
fn an_invalid_exclude_pattern_is_a_manifest_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    fs::write(dir.path().join("a.txt"), b"a").expect("write");
    write_manifest(
        dir.path(),
        concat!(
            "scopes:\n",
            "  src:\n",
            "    globs: [\"*.txt\"]\n",
            "    exclude: [\"[\"]\n",
            "commands:\n  - name: sh\n    inputs: [src]\n",
        ),
    );

    mmz(dir.path())
        .arg("--status")
        .assert()
        .code(4)
        .stderr(predicate::str::contains("invalid pattern `[`"));
}

#[test]
#[cfg(unix)]
fn an_excluded_file_mmz_cannot_read_is_never_opened() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().expect("tempdir");
    write_tree(dir.path());
    let locked = dir.path().join("examples/demo/.cache/blob.bin");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).expect("chmod");
    if fs::read(&locked).is_ok() {
        return; // running as root: no refusal to observe
    }
    write_manifest(
        dir.path(),
        concat!(
            "scopes:\n",
            "  examples:\n",
            "    globs: [\"examples/**\"]\n",
            "    exclude: [\"**/.cache\"]\n",
            "commands:\n  - name: sh\n    inputs: [examples]\n",
        ),
    );

    assert_eq!(
        listed_inputs(dir.path()),
        vec!["examples/demo/task.typ".to_owned()]
    );
    run(dir.path());
    assert_eq!(
        run_len(dir.path()),
        1,
        "the run hashes nothing it cannot read"
    );
}
