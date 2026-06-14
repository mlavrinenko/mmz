//! `mmz --prune`: delete cache records whose rule no longer exists.
//!
//! Renaming or removing a command rule leaves its record orphaned in the cache
//! directory. Pruning compares each stored record's command against the current
//! rule names and removes the ones no rule claims, so the cache cannot
//! accumulate dead state. Records for live rules are left untouched.

use std::collections::BTreeSet;
use std::path::Path;

use crate::cache;
use crate::error::{Error, Result};
use crate::manifest::Manifest;

/// Prunes orphan cache records for the manifest governing `cwd`, returning a
/// human-readable summary of what was removed.
///
/// # Errors
///
/// Returns [`Error::NoManifest`] when none is found, a manifest error when one
/// cannot be loaded, or [`Error::Io`] if the cache directory cannot be swept.
pub fn prune(cwd: &Path) -> Result<String> {
    let manifest_path = Manifest::discover(cwd).ok_or_else(|| Error::NoManifest {
        start: cwd.to_path_buf(),
    })?;
    let manifest = Manifest::load(&manifest_path)?;
    let base = manifest_path
        .parent()
        .ok_or_else(|| Error::Internal("manifest path has no parent".to_owned()))?;
    let cache_dir = base.join(&manifest.cache_dir);
    let live: BTreeSet<String> = manifest
        .commands
        .iter()
        .map(|rule| rule.name.clone())
        .collect();
    let pruned = cache::prune(&cache_dir, &live)?;
    Ok(render(&pruned))
}

/// Summarizes a prune: the count and each removed rule name, or a no-op line.
fn render(pruned: &[String]) -> String {
    if pruned.is_empty() {
        return "no orphan records to prune\n".to_owned();
    }
    let mut out = format!("pruned {} orphan record(s):\n", pruned.len());
    for name in pruned {
        out.push_str(&format!("  {name}\n"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::prune;
    use crate::cache;

    fn write(dir: &std::path::Path, name: &str, body: &str) {
        std::fs::write(dir.join(name), body).expect("write");
    }

    #[test]
    fn prunes_records_for_rules_not_in_the_manifest() {
        let dir = tempfile::tempdir().expect("tempdir");
        let base = dir.path();
        // The manifest knows only `cargo test`, in a custom cache directory.
        write(
            base,
            "mmz.yaml",
            "cache_dir: .cache\ncommands:\n  - name: cargo test\n",
        );
        let cache_dir = base.join(".cache");
        cache::write(&cache_dir, "cargo test", "d", true);
        cache::write(&cache_dir, "cargo bench", "d", true); // orphan

        let first = prune(base).expect("prune");
        assert!(first.contains("cargo bench"), "orphan reported: {first}");
        assert!(first.contains("pruned 1"), "count reported: {first}");
        assert!(
            cache::read(&cache_dir, "cargo test").is_some(),
            "live record kept"
        );
        assert!(
            cache::read(&cache_dir, "cargo bench").is_none(),
            "orphan removed"
        );

        let again = prune(base).expect("prune");
        assert!(
            again.contains("no orphan"),
            "nothing left to prune: {again}"
        );
    }

    #[test]
    fn missing_manifest_is_an_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        assert!(prune(dir.path()).is_err(), "no manifest is fatal");
    }
}
