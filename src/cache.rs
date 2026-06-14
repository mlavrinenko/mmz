//! Per-rule cache records under a gitignored cache directory (`.mmz/` by
//! default; see `cache_dir` in the manifest).
//!
//! Each command rule owns one record, keyed by the rule name (the cache
//! identity). A record is trusted only when its format, algorithm, command,
//! status, and input digest all match — anything else is a miss, so the command
//! re-runs. Records are derived, throwaway state and belong in `.gitignore`.

use std::collections::BTreeSet;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::hashing::ALGORITHM;

/// On-disk record format version. A mismatch invalidates the record.
const FORMAT: u32 = 1;

/// Maximum length of a slug's readable stem, before the disambiguating hash.
/// Caps the filename so a very long rule name cannot overrun the OS limit.
const SLUG_MAX: usize = 64;

/// Outcome of the last run, recorded so a failed run never counts as fresh.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Status {
    Ok,
    Failed,
}

/// A single command rule's memoization state.
#[derive(Debug, Serialize, Deserialize)]
struct Record {
    format: u32,
    algorithm: String,
    command: String,
    input_digest: String,
    status: Status,
    ran_at: u64,
}

/// A trusted view of a stored record, for inspection by `mmz --status`.
pub struct Cached {
    /// Whether the recorded run succeeded.
    pub ok: bool,
    /// The recorded input digest.
    pub digest: String,
    /// Unix seconds when the run was recorded.
    pub ran_at: u64,
}

/// Reads the record for `command` from `dir`, returning a [`Cached`] view only
/// when it is present, parseable, and compatible (format, algorithm, and
/// command match).
///
/// Any read, parse, or compatibility mismatch returns `None` (a miss), so the
/// worst case is an unnecessary re-run, never a wrongful skip.
#[must_use]
pub fn read(dir: &Path, command: &str) -> Option<Cached> {
    let text = fs::read_to_string(record_path(dir, command)).ok()?;
    let record = serde_yaml_ng::from_str::<Record>(&text).ok()?;
    if record.format != FORMAT || record.algorithm != ALGORITHM || record.command != command {
        return None;
    }
    Some(Cached {
        ok: record.status == Status::Ok,
        digest: record.input_digest,
        ran_at: record.ran_at,
    })
}

/// True when a trusted, successful record for `command` in `dir` matches `digest`.
#[must_use]
pub fn is_fresh(dir: &Path, command: &str, digest: &str) -> bool {
    read(dir, command).is_some_and(|cached| cached.ok && cached.digest == digest)
}

/// Records the outcome of a run under `dir`. Best-effort: a write failure is
/// logged, never propagated, because the command has already run and its exit
/// code stands.
pub fn write(dir: &Path, command: &str, digest: &str, ok: bool) {
    let record = Record {
        format: FORMAT,
        algorithm: ALGORITHM.to_owned(),
        command: command.to_owned(),
        input_digest: digest.to_owned(),
        status: if ok { Status::Ok } else { Status::Failed },
        ran_at: now_secs(),
    };
    if let Err(err) = try_write(dir, command, &record) {
        log::warn!("mmz: could not write cache for `{command}`: {err}");
    }
}

/// Removes records in `dir` whose stored command is not in `live`, returning the
/// pruned command names sorted. Leftover `.tmp` files from interrupted writes
/// are swept too. A record that cannot be read or parsed is left untouched, so
/// only confidently orphaned records are deleted. A missing directory is empty.
///
/// # Errors
///
/// Returns [`Error::Io`] if the directory cannot be listed or a record cannot
/// be deleted.
pub fn prune(dir: &Path, live: &BTreeSet<String>) -> Result<Vec<String>> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(err.into()),
    };
    let mut pruned = Vec::new();
    for entry in entries {
        let path = entry?.path();
        match path.extension().and_then(|ext| ext.to_str()) {
            Some("tmp") => {
                let _ = fs::remove_file(&path);
            }
            Some("yaml") => {
                if let Some(command) = record_command(&path) {
                    if !live.contains(command.as_str()) {
                        fs::remove_file(&path)?;
                        pruned.push(command);
                    }
                }
            }
            _ => {}
        }
    }
    pruned.sort();
    Ok(pruned)
}

/// Reads just the stored command name from a record file, or `None` when the
/// file is unreadable or not a parseable record.
fn record_command(path: &Path) -> Option<String> {
    let text = fs::read_to_string(path).ok()?;
    Some(serde_yaml_ng::from_str::<Record>(&text).ok()?.command)
}

/// Writes the record atomically: serialize to a per-process temp file in the
/// cache directory, then rename it over the final path. A reader therefore sees
/// either the old record or the complete new one, never a half-written file —
/// so a crash mid-write or a concurrent writer can never produce a truncated
/// record that would parse wrong. A failed rename cleans up its temp file.
fn try_write(dir: &Path, command: &str, record: &Record) -> Result<()> {
    fs::create_dir_all(dir)?;
    let text = serde_yaml_ng::to_string(record).map_err(|err| Error::Serialize(Box::new(err)))?;
    let tmp = dir.join(format!("{}.{}.tmp", slug(command), std::process::id()));
    fs::write(&tmp, text)?;
    if let Err(err) = fs::rename(&tmp, record_path(dir, command)) {
        let _ = fs::remove_file(&tmp);
        return Err(err.into());
    }
    Ok(())
}

fn record_path(dir: &Path, command: &str) -> PathBuf {
    dir.join(format!("{}.yaml", slug(command)))
}

/// Builds a readable, collision-resistant filename stem from a command name: a
/// lowercased ascii-alphanumeric slug (capped, never empty) plus a short hash of
/// the full name. The hash disambiguates names that share a slug, so a slug
/// collision is at worst a harmless re-run, never a wrongful skip.
fn slug(command: &str) -> String {
    let mut readable = String::new();
    let mut last_dash = false;
    for ch in command.chars() {
        if ch.is_ascii_alphanumeric() {
            readable.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            readable.push('-');
            last_dash = true;
        }
    }
    let capped: String = readable.trim_matches('-').chars().take(SLUG_MAX).collect();
    let stem = match capped.trim_matches('-') {
        "" => "cmd",
        trimmed => trimmed,
    };
    let digest = blake3::hash(command.as_bytes()).to_hex();
    let short = digest.as_str().get(..16).unwrap_or(digest.as_str());
    format!("{stem}-{short}")
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{is_fresh, prune, read, slug, write};

    #[test]
    fn fresh_only_for_matching_successful_record() {
        let dir = tempfile::tempdir().expect("tempdir");
        let base = dir.path();
        assert!(!is_fresh(base, "sh", "digest-a"), "no record yet");

        write(base, "sh", "digest-a", true);
        assert!(
            is_fresh(base, "sh", "digest-a"),
            "matching ok record is fresh"
        );
        assert!(!is_fresh(base, "sh", "digest-b"), "different digest misses");

        write(base, "sh", "digest-a", false);
        assert!(
            !is_fresh(base, "sh", "digest-a"),
            "failed record is never fresh"
        );
    }

    #[test]
    fn distinct_commands_get_distinct_records() {
        let dir = tempfile::tempdir().expect("tempdir");
        let base = dir.path();
        write(base, "cargo test", "d1", true);
        write(base, "cargo build", "d2", true);
        assert!(is_fresh(base, "cargo test", "d1"));
        assert!(is_fresh(base, "cargo build", "d2"));
    }

    #[test]
    fn read_surfaces_recorded_time() {
        let dir = tempfile::tempdir().expect("tempdir");
        write(dir.path(), "sh", "d", true);
        let cached = read(dir.path(), "sh").expect("record");
        assert!(cached.ran_at > 0, "ran_at is recorded and surfaced");
    }

    #[test]
    fn slug_is_readable_distinct_capped_and_never_empty() {
        assert!(slug("cargo test").starts_with("cargo-test-"));
        assert_ne!(slug("cargo test"), slug("cargo build"));
        assert!(
            slug("+++").starts_with("cmd-"),
            "all-symbol name gets a stem"
        );
        let long = "x".repeat(500);
        let stem = slug(&long);
        // 64-char stem + '-' + 16-char hash.
        assert_eq!(stem.len(), super::SLUG_MAX + 1 + 16, "stem is capped");
    }

    #[test]
    fn write_is_atomic_and_leaves_no_temp_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let base = dir.path();
        write(base, "cargo test", "digest-a", true);

        let temps: Vec<_> = std::fs::read_dir(base)
            .expect("cache dir")
            .filter_map(Result::ok)
            .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "tmp"))
            .collect();
        assert!(temps.is_empty(), "rename leaves no .tmp behind");
        assert!(
            is_fresh(base, "cargo test", "digest-a"),
            "record is readable"
        );
    }

    #[test]
    fn prune_drops_only_orphan_records() {
        let dir = tempfile::tempdir().expect("tempdir");
        let base = dir.path();
        write(base, "cargo test", "d1", true);
        write(base, "cargo build", "d2", true);

        let live: BTreeSet<String> = ["cargo test".to_owned()].into_iter().collect();
        let pruned = prune(base, &live).expect("prune");
        assert_eq!(pruned, vec!["cargo build".to_owned()], "orphan removed");
        assert!(is_fresh(base, "cargo test", "d1"), "live record kept");
        assert!(read(base, "cargo build").is_none(), "orphan record gone");
    }

    #[test]
    fn prune_on_missing_dir_is_empty() {
        let dir = tempfile::tempdir().expect("tempdir");
        let pruned = prune(&dir.path().join("absent"), &BTreeSet::new()).expect("prune");
        assert!(pruned.is_empty(), "missing cache dir prunes nothing");
    }
}
