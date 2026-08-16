//! `mmz --prune`: delete cache records whose rule no longer exists.
//!
//! Renaming or removing a command rule leaves its record orphaned in the cache
//! directory. Pruning compares each stored record's command against the current
//! rule names and removes the ones no rule claims, so the cache cannot
//! accumulate dead state. Records for live rules are left untouched.

use std::collections::BTreeSet;
use std::path::Path;

use crate::error::Result;
use crate::manifest::Manifest;
use crate::{cache, parametric};

/// Prunes orphan cache records for the manifest governing `cwd`, returning a
/// human-readable summary of what was removed.
///
/// # Errors
///
/// Returns [`Error::NoManifest`] when none is found, a manifest error when one
/// cannot be loaded, or [`Error::Io`] if the cache directory cannot be swept.
pub fn prune(cwd: &Path) -> Result<String> {
    let located = Manifest::locate(cwd)?;
    let manifest = &located.manifest;
    let base = located.root.as_path();
    let cache_dir = base.join(&manifest.cache_dir);
    let mut live: BTreeSet<String> = BTreeSet::new();
    for rule in &manifest.commands {
        for hit in parametric::expand_rule(manifest, base, rule)? {
            live.insert(hit.exp.identity);
        }
    }
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

    fn manifest(root: &std::path::Path, body: &str) {
        let dir = root.join(".mmz");
        std::fs::create_dir_all(&dir).expect("mkdir .mmz");
        std::fs::write(dir.join("config.yaml"), body).expect("write manifest");
    }

    /// Writes a successful record for `command`, declaring no outputs and
    /// naming no probes.
    fn record(cache_dir: &std::path::Path, command: &str) {
        cache::write(
            cache_dir,
            command,
            &cache::Outcome {
                digest: "d",
                ok: true,
                ..cache::Outcome::default()
            },
        );
    }

    #[test]
    fn prunes_records_for_rules_not_in_the_manifest() {
        let dir = tempfile::tempdir().expect("tempdir");
        let base = dir.path();
        // The manifest knows only `cargo test`, in a custom cache directory.
        manifest(base, "cache_dir: .cache\ncommands:\n  - name: cargo test\n");
        let cache_dir = base.join(".cache");
        record(&cache_dir, "cargo test");
        record(&cache_dir, "cargo bench"); // orphan

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
    fn parametric_expansions_are_live_until_their_file_is_gone() {
        let dir = tempfile::tempdir().expect("tempdir");
        let base = dir.path();
        std::fs::create_dir(base.join("src")).expect("mkdir");
        std::fs::write(base.join("src/a.rs"), b"a").expect("a");
        std::fs::write(base.join("src/b.rs"), b"b").expect("b");
        manifest(
            base,
            "cache_dir: .cache\nscopes:\n  targets: [\"src/**/*.rs\"]\ncommands:\n  - name: \"run {targets}\"\n",
        );
        let cache_dir = base.join(".cache");
        record(&cache_dir, "run src/a.rs");
        record(&cache_dir, "run src/b.rs");

        // Removing b's source orphans its per-file record; a's stays live.
        std::fs::remove_file(base.join("src/b.rs")).expect("rm b");
        let pruned = prune(base).expect("prune");
        assert!(pruned.contains("run src/b.rs"), "orphan pruned: {pruned}");
        assert!(
            cache::read(&cache_dir, "run src/a.rs").is_some(),
            "live expansion kept"
        );
        assert!(
            cache::read(&cache_dir, "run src/b.rs").is_none(),
            "orphan expansion gone"
        );
    }

    #[test]
    fn missing_manifest_is_an_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        assert!(prune(dir.path()).is_err(), "no manifest is fatal");
    }
}
