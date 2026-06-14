//! Content hashing of inputs using `blake3`.

use std::fs::File;
use std::path::Path;

use serde::Serialize;

use crate::error::Result;

/// Name of the hash algorithm recorded in cache records.
pub const ALGORITHM: &str = "blake3";

/// Computes the `blake3` hash of a file, returned as a lowercase hex string.
///
/// The file is streamed, so large and binary inputs are handled without
/// loading them fully into memory.
///
/// # Errors
///
/// Returns [`crate::error::Error::Io`] if the file cannot be opened or read.
pub fn hash_file(path: &Path) -> Result<String> {
    let file = File::open(path)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update_reader(file)?;
    Ok(hasher.finalize().to_hex().as_str().to_owned())
}

/// A single input's relative path paired with its content hash.
#[derive(Serialize)]
pub struct FileHash {
    /// Path relative to the manifest root, with forward slashes.
    pub path: String,
    /// Lowercase hex `blake3` of the file's contents.
    pub hash: String,
}

/// Hashes each input in turn, preserving the order of `rel_paths`.
///
/// # Errors
///
/// Returns [`crate::error::Error::Io`] if any input cannot be read.
pub fn hash_each(base: &Path, rel_paths: &[String]) -> Result<Vec<FileHash>> {
    rel_paths
        .iter()
        .map(|rel| {
            Ok(FileHash {
                path: rel.clone(),
                hash: hash_file(&base.join(rel))?,
            })
        })
        .collect()
}

/// Folds a list of `(path, hash)` pairs into one order-dependent digest. Both
/// the path and the content feed it, so a rename, a deletion, or an edit all
/// change the result. The pairs are expected pre-sorted by path (as
/// [`crate::resolve::expand`] returns them) so the digest is stable.
#[must_use]
pub fn digest_hashes(files: &[FileHash]) -> String {
    let mut hasher = blake3::Hasher::new();
    for file in files {
        hasher.update(file.path.as_bytes());
        hasher.update(b"\0");
        hasher.update(file.hash.as_bytes());
        hasher.update(b"\n");
    }
    hasher.finalize().to_hex().as_str().to_owned()
}

/// Hashes `rel_paths` and folds them into a single digest. Convenience over
/// [`hash_each`] then [`digest_hashes`] for callers that need only the digest.
///
/// # Errors
///
/// Returns [`crate::error::Error::Io`] if any input cannot be read.
pub fn digest_files(base: &Path, rel_paths: &[String]) -> Result<String> {
    Ok(digest_hashes(&hash_each(base, rel_paths)?))
}

#[cfg(test)]
mod tests {
    use super::{digest_files, hash_file};

    #[test]
    fn hashes_are_stable_and_content_dependent() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("a.txt");
        std::fs::write(&path, b"hello").expect("write");

        let first = hash_file(&path).expect("hash");
        assert_eq!(first.len(), 64, "blake3 hex is 64 chars");
        assert_eq!(first, hash_file(&path).expect("hash"), "deterministic");

        std::fs::write(&path, b"world").expect("rewrite");
        assert_ne!(first, hash_file(&path).expect("hash"), "content matters");
    }

    #[test]
    fn digest_reflects_content_and_membership() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("a.txt"), b"one").expect("write");
        std::fs::write(dir.path().join("b.txt"), b"two").expect("write");

        let both = ["a.txt".to_owned(), "b.txt".to_owned()];
        let base = digest_files(dir.path(), &both).expect("digest");

        std::fs::write(dir.path().join("a.txt"), b"edited").expect("rewrite");
        assert_ne!(
            base,
            digest_files(dir.path(), &both).expect("digest"),
            "edit shifts digest"
        );

        let one = ["b.txt".to_owned()];
        assert_ne!(
            base,
            digest_files(dir.path(), &one).expect("digest"),
            "membership shifts digest"
        );
    }
}
