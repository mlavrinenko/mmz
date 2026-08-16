//! The per-scope `gitignore` override, end to end: a scope naming a build
//! artifact opts out of the filter, so the rule it feeds tracks a file git
//! ignores, while every sibling scope in the same rule keeps filtering.

use std::fs;
use std::path::Path;

use predicates::prelude::{PredicateBooleanExt, predicate};

mod support;
use support::{mmz, write_manifest};

/// A scope naming the artifact tree, opting out of the gitignore filter.
const OPTED_OUT: &str = concat!(
    "scopes:\n",
    "  src: [\"*.txt\"]\n",
    "  artifact:\n",
    "    gitignore: false\n",
    "    globs: [\"target/**\"]\n",
    "commands:\n  - name: sh\n    inputs: [src, artifact]\n",
);

/// The same two scopes in today's array form, both inheriting the default.
const INHERITED: &str = concat!(
    "scopes:\n",
    "  src: [\"*.txt\"]\n",
    "  artifact: [\"target/**\"]\n",
    "commands:\n  - name: sh\n    inputs: [src, artifact]\n",
);

/// Writes a project whose `/target` tree and `secret.txt` are git-ignored: one
/// tracked input, one build artifact under the ignored tree, and one ignored
/// file the `src` scope's glob would otherwise reach.
fn write_project(dir: &Path, manifest: &str) {
    write_manifest(dir, manifest);
    fs::write(dir.join(".gitignore"), "/target\nsecret.txt\n").expect("write .gitignore");
    fs::write(dir.join("a.txt"), b"one").expect("write input");
    fs::write(dir.join("secret.txt"), b"hidden").expect("write ignored input");
    fs::create_dir(dir.join("target")).expect("mkdir target");
    fs::write(dir.join("target/out.bin"), b"built").expect("write artifact");
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
fn an_opted_out_scope_tracks_a_git_ignored_artifact() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_project(dir.path(), OPTED_OUT);

    run(dir.path());
    assert_eq!(run_len(dir.path()), 1, "first run executes");

    run(dir.path());
    assert_eq!(run_len(dir.path()), 1, "unchanged inputs are a cache hit");

    fs::write(dir.path().join("target/out.bin"), b"rebuilt").expect("rewrite artifact");
    run(dir.path());
    assert_eq!(
        run_len(dir.path()),
        2,
        "the artifact is an input, so touching it re-runs the command"
    );
}

#[test]
fn the_array_form_still_filters_the_same_artifact() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_project(dir.path(), INHERITED);

    run(dir.path());
    assert_eq!(run_len(dir.path()), 1, "first run executes");

    fs::write(dir.path().join("target/out.bin"), b"rebuilt").expect("rewrite artifact");
    run(dir.path());
    assert_eq!(
        run_len(dir.path()),
        1,
        "without the override the artifact is filtered out and the rule stays fresh"
    );
}

#[test]
fn a_sibling_scope_in_the_same_rule_keeps_filtering() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_project(dir.path(), OPTED_OUT);

    mmz(dir.path())
        .arg("--status=json")
        .assert()
        .success()
        .stdout(
            predicate::str::contains("\"target/out.bin\"")
                .and(predicate::str::contains("\"a.txt\""))
                .and(predicate::str::contains("secret.txt").not()),
        );
}
