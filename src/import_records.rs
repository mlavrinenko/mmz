//! `mmz --import <dir>`: carry cache records over from another checkout.
//!
//! Delegated work runs in a sibling git worktree, which keeps its own
//! gitignored cache. Landing it makes this checkout the same tree, yet every
//! rule still reads stale here, because the passes were recorded next door. A
//! record's input digest folds repo-relative paths and content hashes (see
//! [`crate::hashing::digest_all`]), never an absolute path, so a record is as
//! valid in a twin checkout as where it was written. Importing copies records
//! and judges none of them: one recorded against other inputs still reads stale
//! here, so an import can never make a stale rule look fresh.

use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::manifest::Manifest;
use crate::{cache, prune};

/// Imports into the checkout governing `cwd` the cache records of the checkout
/// at `from` (absolute, or relative to `cwd`), returning a human-readable
/// summary: one line per imported rule, then the count.
///
/// A record is taken when its rule exists in this checkout's merged manifest
/// and the record held here is not newer. Everything else is skipped silently.
///
/// # Errors
///
/// Returns [`Error::NoManifest`] or a manifest error when this checkout's
/// manifest cannot be loaded, [`Error::ImportNotCheckout`] when `from` has no
/// `.mmz/` directory, [`Error::ImportSelf`] when it resolves to this checkout,
/// or [`Error::CacheIo`] when a record cannot be listed or written.
pub fn import(cwd: &Path, from: &Path) -> Result<String> {
    let located = Manifest::locate(cwd)?;
    let manifest = &located.manifest;
    let base = located.root.as_path();
    let source = checkout(cwd, from, base)?;
    let live = prune::live_identities(manifest, base)?;
    let imported = cache::import(
        &source.join(&manifest.cache_dir),
        &base.join(&manifest.cache_dir),
        &live,
    )?;
    let mut out = String::new();
    for rule in &imported {
        out.push_str(rule);
        out.push('\n');
    }
    out.push_str(&format!(
        "mmz: imported {} records from {}\n",
        imported.len(),
        from.display()
    ));
    Ok(out)
}

/// Resolves `from` to the canonical directory of another checkout.
///
/// Canonical on both sides, so a symlink, a `.` or a trailing slash cannot
/// disguise this checkout as another.
fn checkout(cwd: &Path, from: &Path, base: &Path) -> Result<PathBuf> {
    let not_checkout = || Error::ImportNotCheckout {
        dir: from.to_path_buf(),
    };
    let dir = cwd.join(from).canonicalize().map_err(|_| not_checkout())?;
    if !dir.join(".mmz").is_dir() {
        return Err(not_checkout());
    }
    if base.canonicalize().is_ok_and(|own| own == dir) {
        return Err(Error::ImportSelf {
            dir: from.to_path_buf(),
        });
    }
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::import;
    use crate::cache::{self, Outcome};
    use crate::clock::Clock;
    use crate::error::Error;

    /// A checkout holding a one-rule manifest, with `sh` recorded at `ran_at`
    /// when given.
    fn checkout(ran_at: Option<u64>) -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("tempdir");
        let config = dir.path().join(".mmz");
        std::fs::create_dir_all(&config).expect("mkdir .mmz");
        std::fs::write(config.join("config.yaml"), "commands:\n  - name: sh\n")
            .expect("write manifest");
        if let Some(stamp) = ran_at {
            cache::write(
                &config.join("cache"),
                "sh",
                Clock::pinned(stamp),
                &Outcome {
                    digest: "d",
                    ok: true,
                    ..Outcome::default()
                },
            );
        }
        dir
    }

    #[test]
    fn lists_each_imported_rule_then_the_count() {
        let here = checkout(None);
        let other = checkout(Some(10));
        let text = import(here.path(), other.path()).expect("import");
        assert!(text.starts_with("sh\n"), "the rule is named: {text}");
        assert!(text.contains("imported 1 records from"), "counted: {text}");
        let again = import(here.path(), other.path()).expect("import");
        assert!(
            again.starts_with("mmz: imported 0"),
            "nothing left: {again}"
        );
    }

    #[test]
    fn a_relative_directory_is_read_from_the_working_directory() {
        let here = checkout(None);
        let other = checkout(Some(10));
        // Both tempdirs sit in one parent, so the other is a sibling.
        let name = other.path().file_name().expect("tempdir name");
        let relative = Path::new("..").join(name);
        let text = import(here.path(), &relative).expect("import");
        assert!(text.contains("imported 1 records"), "resolved: {text}");
    }

    #[test]
    fn refuses_a_bare_directory_and_this_checkout() {
        let here = checkout(None);
        let bare = tempfile::tempdir().expect("tempdir");
        assert!(matches!(
            import(here.path(), bare.path()),
            Err(Error::ImportNotCheckout { .. })
        ));
        assert!(matches!(
            import(here.path(), here.path()),
            Err(Error::ImportSelf { .. })
        ));
    }
}
