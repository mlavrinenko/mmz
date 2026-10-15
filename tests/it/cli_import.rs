//! `mmz --import <dir>`: carry cache records over from another checkout of the
//! same repo. The digest is built from repo-relative paths and content hashes,
//! so a record is portable, and mmz stays the judge of whether it still holds.

use std::fs;
use std::path::Path;

use predicates::prelude::{PredicateBooleanExt, predicate};

use crate::support::{mmz, write_manifest};

const MANIFEST: &str = "scopes:\n  src: [\"*.txt\"]\ncommands:\n  - name: sh\n    inputs: [src]\n";

/// A checkout holding `MANIFEST` and one input file with `content`.
fn checkout(content: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    write_manifest(dir.path(), MANIFEST);
    fs::write(dir.path().join("a.txt"), content).expect("write input");
    dir
}

/// Records a pass of the `sh` rule in `dir` at the pinned instant `now`.
fn record_pass(dir: &Path, now: &str) {
    mmz(dir)
        .env("MMZ_NOW", now)
        .args(["sh", "-c", "exit 0"])
        .assert()
        .success();
}

fn is_fresh(dir: &Path) -> assert_cmd::assert::Assert {
    mmz(dir).args(["--is-fresh", "--", "sh"]).assert()
}

fn import(here: &Path, other: &Path) -> assert_cmd::assert::Assert {
    mmz(here).arg("--import").arg(other).assert()
}

#[test]
fn a_record_fresh_in_the_other_checkout_reads_fresh_here_when_the_trees_match() {
    let other = checkout("one");
    let here = checkout("one");
    record_pass(other.path(), "1000");
    is_fresh(here.path()).code(1);

    import(here.path(), other.path())
        .success()
        .stdout(predicate::str::starts_with("mmz: imported 1 record from"));

    is_fresh(here.path()).success();
}

#[test]
fn a_record_for_different_inputs_stays_stale() {
    let other = checkout("one");
    let here = checkout("two");
    record_pass(other.path(), "1000");

    import(here.path(), other.path())
        .success()
        .stdout(predicate::str::contains("mmz: imported 1 record from"));

    is_fresh(here.path()).code(1);
}

#[test]
fn a_newer_local_record_is_kept() {
    let other = checkout("one");
    let here = checkout("two");
    record_pass(other.path(), "1000");
    record_pass(here.path(), "2000");

    import(here.path(), other.path())
        .success()
        .stdout(predicate::str::contains("mmz: imported 0 records from"));

    is_fresh(here.path()).success();
}

#[test]
fn an_older_local_record_is_replaced() {
    let other = checkout("two");
    let here = checkout("one");
    record_pass(here.path(), "1000");
    record_pass(other.path(), "2000");
    fs::write(here.path().join("a.txt"), "two").expect("edit input");
    is_fresh(here.path()).code(1);

    import(here.path(), other.path())
        .success()
        .stdout(predicate::str::contains("mmz: imported 1 record from"));

    is_fresh(here.path()).success();
}

#[test]
fn a_record_for_an_unknown_rule_is_skipped() {
    let other = checkout("one");
    let here = checkout("one");
    write_manifest(
        other.path(),
        &format!("{MANIFEST}  - name: echo\n    inputs: [src]\n"),
    );
    mmz(other.path()).args(["echo", "hi"]).assert().success();
    let cache = here.path().join(".mmz/cache");
    assert!(!cache.exists(), "nothing recorded here yet");

    import(here.path(), other.path())
        .success()
        .stdout(predicate::str::contains("echo").not())
        .stdout(predicate::str::contains("mmz: imported 0 records from"));

    assert!(!cache.exists(), "the unknown rule's record was not copied");
}

#[test]
fn importing_twice_has_nothing_left_to_import() {
    let other = checkout("one");
    let here = checkout("one");
    record_pass(other.path(), "1000");

    import(here.path(), other.path()).success();
    import(here.path(), other.path())
        .success()
        .stdout(predicate::str::contains("mmz: imported 0 records from"));
}

#[test]
fn a_directory_without_mmz_is_refused() {
    let here = checkout("one");
    let bare = tempfile::tempdir().expect("tempdir");

    import(here.path(), bare.path())
        .code(2)
        .stderr(predicate::str::contains("not an mmz checkout"));
}

#[test]
fn this_checkout_is_refused_however_it_is_spelled() {
    let here = checkout("one");
    let parent = tempfile::tempdir().expect("tempdir");
    let link = parent.path().join("alias");
    std::os::unix::fs::symlink(here.path(), &link).expect("symlink");

    import(here.path(), Path::new("."))
        .code(2)
        .stderr(predicate::str::contains("this checkout"));
    import(here.path(), &link)
        .code(2)
        .stderr(predicate::str::contains("this checkout"));
}

#[test]
fn import_takes_exactly_one_directory() {
    let here = checkout("one");
    mmz(here.path()).arg("--import").assert().code(2);
    mmz(here.path())
        .args(["--import", "a", "b"])
        .assert()
        .code(2);
}
