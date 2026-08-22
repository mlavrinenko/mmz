//! A discovered manifest and the project root it is anchored to. Split out of
//! `manifest.rs` once that file reached its own line cap — a cohesive seam,
//! since everything here is about pairing one config file with the root its
//! relative paths resolve against, not a slice taken for size alone.
//! [`Located`] stays reachable at its original `crate::manifest::Located` path
//! via the re-export there, so nothing outside has to know the split happened.

use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::manifest::Manifest;
use crate::provenance::Provenance;

/// A discovered manifest: the config file, the project root its relative paths
/// resolve against, the parsed, validated model, and the provenance of every
/// entry in it.
///
/// Config lives at `<root>/.mmz/config.yaml`, so the project root is the parent
/// of `.mmz`. Input globs and `cache_dir` resolve against `root`, never `.mmz`.
/// Both paths are canonical — see [`Located::at`] for why that is a property
/// rather than an accident.
#[derive(Debug)]
pub struct Located {
    /// The config file, canonicalized, for display and error messages.
    pub path: PathBuf,
    /// Project root: the directory that holds `.mmz`, canonicalized.
    pub root: PathBuf,
    /// The parsed, validated manifest.
    pub manifest: Manifest,
    /// Which file contributed each scope, probe and command in `manifest`.
    pub provenance: Provenance,
}

impl Located {
    /// Loads and validates the manifest at `config`, pairing it with the
    /// project root its relative paths resolve against: the parent of the
    /// `.mmz` directory holding it, canonicalized.
    ///
    /// Canonical is load-bearing rather than tidy. Every path [`Provenance`]
    /// records is canonical, and rendering one root-relative is a
    /// `strip_prefix` of the two against each other (see
    /// [`Provenance::display`]) — so a root left as discovered, reached
    /// through a symlink, would render an in-tree fragment as an absolute
    /// path. Deriving the root from the *canonicalized* config path makes the
    /// two agree by construction rather than by the platform's habit of
    /// handing back an already-resolved `current_dir`, which a library caller
    /// passing its own path never goes through. The loader is handed the same
    /// root, so a composition error names a file exactly as a report does.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ManifestUnreadable`] when `config` cannot be
    /// canonicalized,
    /// [`Error::Internal`] when it has no project root (it always does in
    /// practice — `<root>/.mmz/config.yaml`), or any parse, import, merge or
    /// validation error [`crate::compose::load`] raises.
    pub fn at(config: &Path) -> Result<Self> {
        let path = std::fs::canonicalize(config).map_err(|source| Error::ManifestUnreadable {
            path: config.to_path_buf(),
            source,
        })?;
        let root = path
            .parent()
            .and_then(Path::parent)
            .ok_or_else(|| Error::Internal("config path has no project root".to_owned()))?
            .to_path_buf();
        let (manifest, provenance) = crate::compose::load(&path, &root)?;
        Ok(Self {
            path,
            root,
            manifest,
            provenance,
        })
    }
}

#[cfg(test)]
#[path = "manifest_located_tests.rs"]
mod tests;
