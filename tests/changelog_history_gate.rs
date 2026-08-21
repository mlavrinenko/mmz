//! The changelog-history gate, exercised on the edit it was written against.
//!
//! `.just/scripts/check-changelog-history.sh` asserts that every `## [x.y.z]`
//! section of `CHANGELOG.md` still reads as the `vx.y.z` tag shipped it. The
//! failure it exists to catch really happened here — `4c57277` inserted a
//! `### Fixed` block under `## [Unreleased]` and wrote over the
//! `## [0.7.0] - 2026-08-17` heading, moving sixty-nine released lines into
//! `Unreleased` where ten green gates ran over them for thirteen commits. That
//! exact shape is `swallows_a_released_section_under_unreleased` below.
//!
//! Every case builds its own throwaway git repo, because the property under
//! test is a relationship between the working tree and a tag: a fixture that
//! is only a file cannot express it. A shell gate tested from the Rust suite is
//! the same deliberate category mix `docs_capture_paths.rs` makes — `just test`
//! is a `just check` arm, so this is the one harness that runs the assertions
//! below without anyone remembering to.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::TempDir;

/// The changelog as v0.1.0 shipped it: one released section, and the
/// `Unreleased` heading above it that the historic edit collided with.
const RELEASED: &str = "\
# Changelog

## [Unreleased]

## [0.1.0] - 2026-01-01

### Added

- The thing the tag shipped.
";

/// Runs `git` in `dir` with a hermetic identity, so the test does not depend on
/// the developer's `~/.gitconfig` (or on there being one).
fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .current_dir(dir)
        .args(args)
        .env("GIT_AUTHOR_NAME", "mmz tests")
        .env("GIT_AUTHOR_EMAIL", "tests@example.invalid")
        .env("GIT_COMMITTER_NAME", "mmz tests")
        .env("GIT_COMMITTER_EMAIL", "tests@example.invalid")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .output()
        .unwrap_or_else(|e| panic!("git {args:?} should run: {e}"));
    assert!(
        out.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// A repo whose only commit is `RELEASED`, tagged `v0.1.0`.
fn repo_at_v0_1_0() -> TempDir {
    let dir = tempfile::tempdir().expect("temp dir");
    git(dir.path(), &["init", "-q", "-b", "main", "."]);
    write(dir.path(), RELEASED);
    git(dir.path(), &["add", "-A"]);
    git(dir.path(), &["commit", "-qm", "release 0.1.0"]);
    git(dir.path(), &["tag", "-a", "v0.1.0", "-m", "v0.1.0"]);
    dir
}

fn write(dir: &Path, body: &str) {
    fs::write(dir.join("CHANGELOG.md"), body).expect("write CHANGELOG.md");
}

/// Runs the gate (or, with `args`, the `--waive` side of it) in `dir`.
fn check(dir: &Path, args: &[&str]) -> Output {
    let script =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".just/scripts/check-changelog-history.sh");
    Command::new("bash")
        .arg(script)
        .args(args)
        .current_dir(dir)
        .output()
        .expect("gate should run")
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn passes_when_every_released_section_matches_its_tag() {
    let repo = repo_at_v0_1_0();
    let out = check(repo.path(), &[]);
    assert!(
        out.status.success(),
        "an untouched history should pass: {}",
        stderr(&out)
    );
    assert!(
        stdout(&out).contains("1 released section"),
        "should say what it compared: {}",
        stdout(&out)
    );
}

/// Appending to `Unreleased` is the everyday edit, and it must stay free.
#[test]
fn tolerates_edits_above_the_released_sections() {
    let repo = repo_at_v0_1_0();
    write(
        repo.path(),
        &RELEASED.replace(
            "## [Unreleased]\n",
            "## [Unreleased]\n\n### Fixed\n\n- something new\n",
        ),
    );
    let out = check(repo.path(), &[]);
    assert!(
        out.status.success(),
        "an Unreleased entry is not a rewrite of history: {}",
        stderr(&out)
    );
}

/// The historic defect, reproduced: the inserted block overwrites the released
/// heading, so every line the tag shipped becomes part of `Unreleased`.
#[test]
fn swallows_a_released_section_under_unreleased() {
    let repo = repo_at_v0_1_0();
    write(
        repo.path(),
        "\
# Changelog

## [Unreleased]

### Fixed

- something new

### Added

- The thing the tag shipped.
",
    );
    let out = check(repo.path(), &[]);
    assert!(!out.status.success(), "the swallowed section should fail");
    let err = stderr(&out);
    assert!(
        err.contains("[0.1.0]") && err.contains("GONE"),
        "should name the version and say the section is gone: {err}"
    );
    assert!(
        err.contains("git show v0.1.0:CHANGELOG.md"),
        "should say how to restore it: {err}"
    );
}

/// A section still present but edited — the quieter half of the same failure.
#[test]
fn fails_on_an_edit_inside_a_released_section() {
    let repo = repo_at_v0_1_0();
    write(
        repo.path(),
        &RELEASED.replace("the tag shipped", "we meant"),
    );
    let out = check(repo.path(), &[]);
    assert!(!out.status.success(), "an edited section should fail");
    let err = stderr(&out);
    assert!(
        err.contains("differs from what v0.1.0 shipped"),
        "should name the tag it differs from: {err}"
    );
    assert!(
        err.contains("-- The thing the tag shipped.") && err.contains("+- The thing we meant."),
        "should diff the two versions: {err}"
    );
}

/// The escape hatch: a recorded hash of the rewritten bytes, in the spirit of
/// `outdatty.lock`.
#[test]
fn a_recorded_waiver_covers_the_bytes_it_was_recorded_against() {
    let repo = repo_at_v0_1_0();
    write(
        repo.path(),
        &RELEASED.replace("the tag shipped", "we meant"),
    );

    let waive = check(
        repo.path(),
        &["--waive", "0.1.0", "the entry named the wrong thing"],
    );
    assert!(
        waive.status.success(),
        "recording a waiver should succeed: {}",
        stderr(&waive)
    );
    let recorded = fs::read_to_string(repo.path().join("CHANGELOG.waivers")).expect("waivers");
    assert!(
        recorded.contains("0.1.0 ") && recorded.contains("# the entry named the wrong thing"),
        "the waiver should carry the version and the reason: {recorded}"
    );

    let out = check(repo.path(), &[]);
    assert!(
        out.status.success(),
        "the waived section should pass: {}",
        stderr(&out)
    );
    assert!(
        stdout(&out).contains("1 waived"),
        "should report the waiver rather than hide it: {}",
        stdout(&out)
    );
}

/// The property that makes the waiver a watermark rather than a switch: it
/// covers one set of bytes, so the NEXT corruption of a waived section fails
/// exactly as the first would have.
#[test]
fn a_waiver_does_not_cover_a_later_edit_to_the_same_section() {
    let repo = repo_at_v0_1_0();
    write(
        repo.path(),
        &RELEASED.replace("the tag shipped", "we meant"),
    );
    check(repo.path(), &["--waive", "0.1.0", "reviewed"]);

    write(
        repo.path(),
        &RELEASED.replace("the tag shipped", "nobody reviewed"),
    );
    let out = check(repo.path(), &[]);
    assert!(!out.status.success(), "a later edit should fail again");
    assert!(
        stderr(&out).contains("recorded against other bytes"),
        "should say the waiver does not cover this: {}",
        stderr(&out)
    );
}

/// A waiver with nothing left to cover is a waiver nobody rereads, and the next
/// edit to that section would land underneath it unreviewed.
#[test]
fn fails_on_a_waiver_whose_section_matches_its_tag_again() {
    let repo = repo_at_v0_1_0();
    write(
        repo.path(),
        &RELEASED.replace("the tag shipped", "we meant"),
    );
    check(repo.path(), &["--waive", "0.1.0", "reviewed"]);

    write(repo.path(), RELEASED);
    let out = check(repo.path(), &[]);
    assert!(!out.status.success(), "a stale waiver should fail");
    assert!(
        stderr(&out).contains("needs no waiver"),
        "should say the entry has nothing to cover: {}",
        stderr(&out)
    );
}

/// Refusing to waive a section that matches its tag, so a waiver can never be
/// recorded pre-emptively — the hash would be the tag's own bytes and would
/// cover the next edit silently.
#[test]
fn refuses_to_waive_a_section_that_still_matches_its_tag() {
    let repo = repo_at_v0_1_0();
    let out = check(repo.path(), &["--waive", "0.1.0", "just in case"]);
    assert!(!out.status.success(), "there is nothing to waive");
    assert!(
        stderr(&out).contains("nothing to waive"),
        "should say so: {}",
        stderr(&out)
    );
    assert!(
        !repo.path().join("CHANGELOG.waivers").exists(),
        "a refused waiver should write no file"
    );
}

/// The silent-green shape this whole gate exists to avoid: with a shallow
/// clone, `--merged HEAD` resolves nothing and a naive implementation would
/// report success over an empty tag list.
#[test]
fn refuses_a_shallow_clone_rather_than_checking_nothing() {
    let origin = repo_at_v0_1_0();
    let clone = tempfile::tempdir().expect("temp dir");
    let target = clone.path().join("shallow");
    git(
        clone.path(),
        &[
            "clone",
            "-q",
            "--depth",
            "1",
            &format!("file://{}", origin.path().display()),
            target.to_str().expect("utf-8 path"),
        ],
    );

    let out = check(&target, &[]);
    assert!(
        !out.status.success(),
        "a shallow clone should fail the gate"
    );
    assert!(
        stderr(&out).contains("fetch-depth: 0"),
        "should name the fix a CI checkout needs: {}",
        stderr(&out)
    );
}

/// A tag from before the changelog carried a section of its own has no shipped
/// record to protect. Reported, not failed — and not counted as checked.
#[test]
fn reports_a_tag_that_shipped_no_section_of_its_own() {
    let dir = tempfile::tempdir().expect("temp dir");
    git(dir.path(), &["init", "-q", "-b", "main", "."]);
    write(dir.path(), "# Changelog\n\n## [Unreleased]\n");
    git(dir.path(), &["add", "-A"]);
    git(dir.path(), &["commit", "-qm", "before the section existed"]);
    git(dir.path(), &["tag", "-a", "v0.1.0", "-m", "v0.1.0"]);
    write(dir.path(), RELEASED);

    let out = check(dir.path(), &[]);
    assert!(
        out.status.success(),
        "there is no released record to protect: {}",
        stderr(&out)
    );
    assert!(
        stderr(&out).contains("shipped no [0.1.0] section"),
        "should say which tag it skipped: {}",
        stderr(&out)
    );
    assert!(
        stdout(&out).contains("1 without one"),
        "should count it as unchecked, not as a pass: {}",
        stdout(&out)
    );
}
