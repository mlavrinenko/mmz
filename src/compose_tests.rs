//! Unit tests for [`crate::compose`]'s path resolution: how an `imports:`
//! entry — file or directory, relative or absolute — is expanded to the
//! concrete files that get loaded. Merge semantics live in
//! `compose_merge_tests.rs`; cycle and diamond detection live in
//! `compose_cycles_tests.rs`.

use std::path::{Path, PathBuf};

use super::load;
use crate::error::Error;
use crate::manifest::Manifest;

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

/// The command names in `manifest`, in declaration order.
fn names(manifest: &Manifest) -> Vec<&str> {
    manifest.commands.iter().map(|c| c.name.as_str()).collect()
}

#[test]
fn a_missing_import_file_errors_naming_the_path() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = write(dir.path(), "root.yaml", "imports: [ghost.yaml]\n");
    let err = load(&root).expect_err("missing import file");
    let text = err.to_string();
    assert!(
        text.contains("ghost.yaml"),
        "names the missing path: {text}"
    );
    assert!(matches!(err, Error::ImportMissing { .. }));
}

#[test]
fn a_missing_import_directory_errors_naming_the_path() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = write(dir.path(), "root.yaml", "imports: [conf.d/]\n");
    let err = load(&root).expect_err("missing import directory");
    let text = err.to_string();
    assert!(text.contains("conf.d"), "names the missing path: {text}");
    assert!(matches!(err, Error::ImportMissing { .. }));
}

#[test]
fn an_empty_import_directory_is_accepted() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(dir.path().join("conf.d")).expect("mkdir");
    let root = write(
        dir.path(),
        "root.yaml",
        "imports: [conf.d/]\ncommands: []\n",
    );
    let (manifest, _) = load(&root).expect("an empty declared directory is fine");
    assert!(manifest.commands.is_empty());
}

#[test]
fn a_directory_entry_sorts_lexically_and_ignores_non_yaml_files() {
    let dir = tempfile::tempdir().expect("tempdir");
    write(dir.path(), "conf.d/b.yaml", "commands:\n  - name: b\n");
    write(dir.path(), "conf.d/a.yml", "commands:\n  - name: a\n");
    write(dir.path(), "conf.d/notes.txt", "not yaml\n");
    let root = write(dir.path(), "root.yaml", "imports: [conf.d/]\n");
    let (manifest, _) = load(&root).expect("directory expands");
    assert_eq!(
        names(&manifest),
        vec!["a", "b"],
        "a.yml sorts before b.yaml; notes.txt is ignored"
    );
}

#[test]
fn relative_paths_resolve_against_the_importing_file_not_the_root() {
    let dir = tempfile::tempdir().expect("tempdir");
    write(
        dir.path(),
        "sub/sibling.yaml",
        "commands:\n  - name: sibling\n",
    );
    write(
        dir.path(),
        "sub/frag.yaml",
        "imports: [sibling.yaml]\ncommands:\n  - name: frag\n",
    );
    let root = write(dir.path(), "root.yaml", "imports: [sub/frag.yaml]\n");
    let (manifest, _) =
        load(&root).expect("frag.yaml's own import resolves against sub/, not the root's dir");
    assert_eq!(names(&manifest), vec!["frag", "sibling"]);
}

#[test]
fn an_absolute_path_outside_the_project_root_loads() {
    let project = tempfile::tempdir().expect("tempdir");
    let outside = tempfile::tempdir().expect("tempdir");
    let fragment = write(
        outside.path(),
        "rules.yaml",
        "commands:\n  - name: outside\n",
    );
    let root = write(
        project.path(),
        "root.yaml",
        &format!("imports: [{}]\n", fragment.display()),
    );
    let (manifest, _) = load(&root).expect("an absolute path is used as written");
    assert_eq!(names(&manifest), vec!["outside"]);
}
