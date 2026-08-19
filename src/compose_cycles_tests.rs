//! Unit tests for [`crate::compose`]'s cycle and diamond detection, both
//! measured over canonicalized paths on the current import chain. Path
//! resolution lives in `compose_tests.rs`; merge semantics live in
//! `compose_merge_tests.rs`.

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
fn import_cycle_errors_with_the_chain() {
    let dir = tempfile::tempdir().expect("tempdir");
    write(dir.path(), "a.yaml", "imports: [b.yaml]\n");
    write(dir.path(), "b.yaml", "imports: [a.yaml]\n");
    let root = write(dir.path(), "root.yaml", "imports: [a.yaml]\n");
    let err = load(&root).expect_err("import cycle");
    let text = err.to_string();
    assert!(text.contains("root.yaml"), "chain starts at root: {text}");
    assert!(text.contains("a.yaml"), "chain includes a: {text}");
    assert!(text.contains("b.yaml"), "chain includes b: {text}");
    assert!(matches!(err, Error::ImportCycle { .. }));
}

#[test]
fn a_diamond_loads_once_and_does_not_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    write(dir.path(), "shared.yaml", "scopes:\n  rust: [\"src/**\"]\n");
    write(
        dir.path(),
        "a.yaml",
        "imports: [shared.yaml]\ncommands:\n  - name: a\n",
    );
    write(
        dir.path(),
        "b.yaml",
        "imports: [shared.yaml]\ncommands:\n  - name: b\n",
    );
    let root = write(dir.path(), "root.yaml", "imports: [a.yaml, b.yaml]\n");
    let (manifest, provenance) = load(&root).expect("a diamond loads once rather than erroring");
    assert!(manifest.scopes.contains_key("rust"));
    assert_eq!(names(&manifest), vec!["a", "b"]);
    assert!(provenance.scopes.contains_key("rust"));
}
