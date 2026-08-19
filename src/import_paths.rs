//! Resolves one `imports:` entry to the concrete manifest file paths it
//! names.
//!
//! An entry can name a file directly, in which case it names exactly that
//! file, or a directory, in which case it expands to every `*.yaml` and
//! `*.yml` file directly inside it (not recursive), sorted lexically by file
//! name — the drop-in ergonomics of a `conf.d` convention while keeping the
//! directory *declared*, so a stray file elsewhere never changes behaviour.
//! [`crate::compose`] resolves each entry through [`expand_import`], in
//! listed order, then loads each resulting file itself: path resolution and
//! the cycle/diamond bookkeeping that loading requires are deliberately
//! separate concerns, so this module owns none of the latter.

use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

/// Resolves one `imports:` entry declared in `importer` (whose directory is
/// `base_dir`) to the manifest files it names: itself, if it names a file, or
/// the `*.yaml`/`*.yml` files directly inside it, lexically sorted, if it
/// names a directory.
///
/// # Errors
///
/// Returns [`Error::ImportMissing`] if the resolved path is neither a file nor
/// a directory.
pub(crate) fn expand_import(importer: &Path, base_dir: &Path, entry: &str) -> Result<Vec<PathBuf>> {
    let candidate = PathBuf::from(entry);
    let resolved = if candidate.is_absolute() {
        candidate
    } else {
        base_dir.join(candidate)
    };
    if resolved.is_dir() {
        return list_yaml_files(&resolved);
    }
    if resolved.is_file() {
        return Ok(vec![resolved]);
    }
    Err(Error::ImportMissing {
        importer: importer.to_path_buf(),
        path: resolved,
    })
}

/// Lists the `*.yaml` and `*.yml` files directly inside `dir` (not recursive),
/// sorted lexically by file name.
///
/// # Errors
///
/// Returns [`Error::ImportNotReadable`] if `dir` (or an entry inside it)
/// cannot be read.
fn list_yaml_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let entries = std::fs::read_dir(dir).map_err(|source| Error::ImportNotReadable {
        path: dir.to_path_buf(),
        source,
    })?;
    for entry in entries {
        let entry = entry.map_err(|source| Error::ImportNotReadable {
            path: dir.to_path_buf(),
            source,
        })?;
        let candidate = entry.path();
        if !candidate.is_file() {
            continue;
        }
        let is_yaml = candidate
            .extension()
            .and_then(std::ffi::OsStr::to_str)
            .is_some_and(|extension| extension == "yaml" || extension == "yml");
        if is_yaml {
            files.push(candidate);
        }
    }
    files.sort();
    Ok(files)
}
