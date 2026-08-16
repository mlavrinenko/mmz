//! Content hashing of inputs using `blake3`.

use std::collections::BTreeMap;
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

/// Computes the `blake3` hash of `bytes` as a lowercase hex string — the probe
/// counterpart to [`hash_file`], for input bytes that never touch the disk.
#[must_use]
pub fn hash_bytes(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().as_str().to_owned()
}

/// Folds a list of `(path, hash)` pairs into one order-dependent digest. Both
/// the path and the content feed it, so a rename, a deletion, or an edit all
/// change the result. The pairs are expected pre-sorted by path (as
/// [`crate::resolve::expand`] returns them) so the digest is stable.
#[must_use]
pub fn digest_hashes(files: &[FileHash]) -> String {
    digest_all(files, &BTreeMap::new())
}

/// Folds a rule's whole input set — its files, then its probes — into one
/// digest.
///
/// Probe digests fold after the files behind a `probe` marker, so a probe named
/// `x` and a file path `x` can never contribute the same bytes. The map is
/// sorted by name (it is a [`BTreeMap`]), so reordering a rule's `inputs:` does
/// not bust its record. With no probes the fold is byte-identical to what
/// [`digest_hashes`] always produced, so records written before probes existed
/// stay valid.
#[must_use]
pub fn digest_all(files: &[FileHash], probes: &BTreeMap<String, String>) -> String {
    let mut hasher = blake3::Hasher::new();
    for file in files {
        hasher.update(file.path.as_bytes());
        hasher.update(b"\0");
        hasher.update(file.hash.as_bytes());
        hasher.update(b"\n");
    }
    for (name, hash) in probes {
        hasher.update(b"probe\0");
        hasher.update(name.as_bytes());
        hasher.update(b"\0");
        hasher.update(hash.as_bytes());
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

/// Hashes `rel_paths` and folds them together with `probes` into one digest —
/// the whole-input-set counterpart to [`digest_files`].
///
/// # Errors
///
/// Returns [`crate::error::Error::Io`] if any input cannot be read.
pub fn digest_with(
    base: &Path,
    rel_paths: &[String],
    probes: &BTreeMap<String, String>,
) -> Result<String> {
    Ok(digest_all(&hash_each(base, rel_paths)?, probes))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{FileHash, digest_all, digest_files, digest_hashes, hash_bytes, hash_file};

    fn pair(path: &str, hash: &str) -> FileHash {
        FileHash {
            path: path.to_owned(),
            hash: hash.to_owned(),
        }
    }

    #[test]
    fn probes_fold_after_files_without_disturbing_a_probe_free_digest() {
        let files = [pair("a.txt", "h1"), pair("b.txt", "h2")];
        let none = BTreeMap::new();
        assert_eq!(
            digest_all(&files, &none),
            digest_hashes(&files),
            "no probes must fold byte-identically to the old file-only digest, so records written before probes existed stay valid"
        );

        let one: BTreeMap<String, String> =
            [("tool".to_owned(), "p1".to_owned())].into_iter().collect();
        let with_probe = digest_all(&files, &one);
        assert_ne!(
            with_probe,
            digest_hashes(&files),
            "a probe feeds the digest"
        );

        let moved: BTreeMap<String, String> =
            [("tool".to_owned(), "p2".to_owned())].into_iter().collect();
        assert_ne!(
            with_probe,
            digest_all(&files, &moved),
            "and its output shifts it"
        );

        // A probe named `x` and a file path `x` must not collide.
        let named: BTreeMap<String, String> = [("a.txt".to_owned(), "h1".to_owned())]
            .into_iter()
            .collect();
        assert_ne!(
            digest_all(&[], &named),
            digest_hashes(&[pair("a.txt", "h1")]),
            "the probe marker keeps a probe name from imitating a file path"
        );
    }

    #[test]
    fn byte_hashing_matches_file_hashing_of_the_same_bytes() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("probe-output");
        std::fs::write(&path, b"hello").expect("write");
        assert_eq!(
            hash_bytes(b"hello"),
            hash_file(&path).expect("hash"),
            "a probe's stdout hashes exactly as the same bytes on disk would"
        );
    }

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
