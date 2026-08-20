//! Unit tests for [`crate::manifest::Located`]: the canonical root it anchors
//! a manifest to, and how the loader it drives names a file in an error.
//! Discovery itself is covered in `manifest_tests.rs`; the merge rules those
//! errors come out of live in `compose_merge_tests.rs`.

use std::path::{Path, PathBuf};

use crate::manifest::Manifest;
use crate::provenance::Provenance;

/// Writes `body` to `relative` under `dir`, creating parent directories as
/// needed, and returns the written path.
fn write(dir: &Path, relative: &str, body: &str) -> PathBuf {
    let path = dir.join(relative);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("mkdir");
    }
    std::fs::write(&path, body).expect("write");
    path
}

/// A temp directory's own canonical path: the comparison every test here
/// makes is against a canonical root, and a temp directory is reached through
/// a symlink on some platforms (macOS's `/var`) even before a test builds one
/// deliberately.
fn canonical(dir: &tempfile::TempDir) -> PathBuf {
    dir.path().canonicalize().expect("canonicalize")
}

/// The invariant the whole type exists to hold: a root reached through a
/// symlink is canonicalized, so it still strips off the canonical paths
/// [`Provenance`] records and an in-tree fragment renders relative. Left
/// uncanonicalized, `strip_prefix` fails and the fragment renders as an
/// absolute path — cosmetic, but held by luck rather than by construction,
/// since on Linux `current_dir()` hands back a resolved path and hides it.
/// A library caller passing its own path never goes through `current_dir()`,
/// which is what the symlink below stands in for.
#[test]
#[cfg(unix)]
fn a_root_reached_through_a_symlink_is_canonicalized() {
    let dir = tempfile::tempdir().expect("tempdir");
    let base = canonical(&dir);
    let physical = base.join("project");
    write(
        &physical,
        ".mmz/conf.d/lint.yaml",
        "commands:\n  - name: lint\n",
    );
    write(&physical, ".mmz/config.yaml", "imports: [conf.d/]\n");
    let via_link = base.join("link");
    std::os::unix::fs::symlink(&physical, &via_link).expect("symlink");

    let located = Manifest::locate(&via_link).expect("locate through the symlink");
    assert_eq!(
        located.root, physical,
        "the root is the resolved directory, not the route taken to it"
    );
    let source = located
        .provenance
        .commands
        .get("lint")
        .expect("provenance records the fragment that declared `lint`");
    assert_eq!(
        Provenance::display(source, &located.root),
        ".mmz/conf.d/lint.yaml",
        "an in-tree fragment renders relative however the root was reached"
    );
}

/// A composition error names its files the way `--status` and `--dump-config`
/// name theirs, because it renders them with the same rule: the loader is
/// handed the project root rather than left to print whatever absolute path it
/// happens to hold.
#[test]
fn a_composition_error_names_an_in_root_fragment_relative() {
    let dir = tempfile::tempdir().expect("tempdir");
    let base = canonical(&dir);
    write(
        &base,
        ".mmz/conf.d/lint.yaml",
        "scopes:\n  rust: [\"src/**\"]\n",
    );
    write(
        &base,
        ".mmz/config.yaml",
        "imports: [conf.d/]\nscopes:\n  rust: [\"src/**\"]\n",
    );

    let err = Manifest::locate(&base).expect_err("`rust` is declared in two files");
    assert_eq!(
        err.to_string(),
        "scope `rust` is declared in both .mmz/config.yaml and .mmz/conf.d/lint.yaml",
    );
}

/// The other half of the same rule: a fragment outside the root — the
/// store-path case composition exists to support — keeps its whole path,
/// because that is the only form of it a reader can act on.
#[test]
fn a_composition_error_keeps_the_whole_path_for_a_fragment_outside_the_root() {
    let project = tempfile::tempdir().expect("tempdir");
    let store = tempfile::tempdir().expect("tempdir");
    let base = canonical(&project);
    let fragment = write(
        &canonical(&store),
        "rules.yaml",
        "probes:\n  git:\n    run: \"echo x\"\n",
    );
    write(
        &base,
        ".mmz/config.yaml",
        &format!(
            "imports: [{}]\nprobes:\n  git:\n    run: \"echo x\"\n",
            fragment.display()
        ),
    );

    let err = Manifest::locate(&base).expect_err("`git` is declared in two files");
    assert_eq!(
        err.to_string(),
        format!(
            "probe `git` is declared in both .mmz/config.yaml and {}",
            fragment.display()
        ),
    );
}
