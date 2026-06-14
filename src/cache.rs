//! Per-rule cache records under a gitignored `.mmz/` directory.
//!
//! Each command rule owns one record, keyed by the rule name (the cache
//! identity). A record is trusted only when its format, algorithm, command,
//! status, and input digest all match — anything else is a miss, so the command
//! re-runs. Records are derived, throwaway state and belong in `.gitignore`.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::hashing::ALGORITHM;

/// On-disk record format version. A mismatch invalidates the record.
const FORMAT: u32 = 1;

/// Cache directory name, relative to the manifest root.
const DIR: &str = ".mmz";

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
}

/// Reads the record for `command`, returning a [`Cached`] view only when it is
/// present, parseable, and compatible (format, algorithm, and command match).
///
/// Any read, parse, or compatibility mismatch returns `None` (a miss), so the
/// worst case is an unnecessary re-run, never a wrongful skip.
#[must_use]
pub fn read(base: &Path, command: &str) -> Option<Cached> {
    let text = fs::read_to_string(record_path(base, command)).ok()?;
    let record = serde_yaml_ng::from_str::<Record>(&text).ok()?;
    if record.format != FORMAT || record.algorithm != ALGORITHM || record.command != command {
        return None;
    }
    Some(Cached {
        ok: record.status == Status::Ok,
        digest: record.input_digest,
    })
}

/// True when a trusted, successful record for `command` matches `digest`.
#[must_use]
pub fn is_fresh(base: &Path, command: &str, digest: &str) -> bool {
    read(base, command).is_some_and(|cached| cached.ok && cached.digest == digest)
}

/// Records the outcome of a run. Best-effort: a write failure is logged, never
/// propagated, because the command has already run and its exit code stands.
pub fn write(base: &Path, command: &str, digest: &str, ok: bool) {
    let record = Record {
        format: FORMAT,
        algorithm: ALGORITHM.to_owned(),
        command: command.to_owned(),
        input_digest: digest.to_owned(),
        status: if ok { Status::Ok } else { Status::Failed },
        ran_at: now_secs(),
    };
    if let Err(err) = try_write(base, command, &record) {
        log::warn!("mmz: could not write cache for `{command}`: {err}");
    }
}

fn try_write(base: &Path, command: &str, record: &Record) -> Result<()> {
    fs::create_dir_all(base.join(DIR))?;
    let text = serde_yaml_ng::to_string(record).map_err(|err| Error::Serialize(Box::new(err)))?;
    fs::write(record_path(base, command), text)?;
    Ok(())
}

fn record_path(base: &Path, command: &str) -> PathBuf {
    base.join(DIR).join(format!("{}.yaml", slug(command)))
}

/// Builds a readable, collision-resistant filename stem from a command name:
/// a lowercased ascii-alphanumeric slug plus a short hash of the full name.
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
    let digest = blake3::hash(command.as_bytes()).to_hex();
    let short = digest.as_str().get(..16).unwrap_or(digest.as_str());
    format!("{}-{short}", readable.trim_matches('-'))
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

#[cfg(test)]
mod tests {
    use super::{is_fresh, slug, write};

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
    fn slug_is_readable_and_distinct() {
        assert!(slug("cargo test").starts_with("cargo-test-"));
        assert_ne!(slug("cargo test"), slug("cargo build"));
    }
}
