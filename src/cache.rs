//! Per-rule cache records under a gitignored cache directory (`.mmz/cache` by
//! default; see `cache_dir` in the manifest).
//!
//! Each command rule owns one record, keyed by the rule name (the cache
//! identity). A record is trusted only when its format, algorithm, command,
//! status, and input digest all match — anything else is a miss, so the command
//! re-runs. Records are derived, throwaway state and belong in `.gitignore`.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::clock::Clock;
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
///
/// `outputs` is the artifact list the rule declared when the run was recorded,
/// so `mmz --status` can report a missing artifact against the run that
/// promised it. `probes` is the same idea for command-driven inputs: the digest
/// each named probe produced at that moment, so a later stale verdict can name
/// the probe that moved instead of sending a reader to diff files that did not.
/// Both default to empty, which is exactly what a record written before the
/// field existed (or by a rule declaring neither) means, so neither needs a
/// [`FORMAT`] bump: freshness is decided against the manifest's current
/// declaration, never against the stored lists.
#[derive(Debug, Serialize, Deserialize)]
struct Record {
    format: u32,
    algorithm: String,
    command: String,
    input_digest: String,
    status: Status,
    ran_at: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    outputs: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    probes: BTreeMap<String, String>,
}

/// What a completed run records: the digest it was measured against, whether it
/// succeeded, the artifacts its rule declared, and the probe digests that fed
/// the input digest.
///
/// A struct rather than five positional arguments so a new recorded fact costs
/// a field, not another parameter at every call site.
#[derive(Default)]
pub struct Outcome<'a> {
    /// The input digest the run was measured against.
    pub digest: &'a str,
    /// Whether the run exited 0.
    pub ok: bool,
    /// Artifact paths the rule declared when the run was recorded.
    pub outputs: &'a [PathBuf],
    /// Digest of each probe the rule names, by probe name.
    pub probes: BTreeMap<String, String>,
}

/// A trusted view of a stored record, for inspection by `mmz --status`.
pub struct Cached {
    /// Whether the recorded run succeeded.
    pub ok: bool,
    /// The recorded input digest.
    pub digest: String,
    /// Unix seconds when the run was recorded.
    pub ran_at: u64,
    /// The outputs the rule declared when this run was recorded, as written.
    /// Empty for a rule that declares none.
    pub outputs: Vec<String>,
    /// The digest each named probe produced when this run was recorded. Empty
    /// for a rule that names none.
    pub probes: BTreeMap<String, String>,
    /// Every stored record field as a string, keyed by field name, for cache-hit
    /// notice macros (`{cache:<field>}`). Driven off serialization, so a new
    /// record field becomes a macro automatically.
    pub fields: BTreeMap<String, String>,
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
    let fields = record_fields(&record);
    Some(Cached {
        ok: record.status == Status::Ok,
        digest: record.input_digest,
        ran_at: record.ran_at,
        outputs: record.outputs,
        probes: record.probes,
        fields,
    })
}

/// Flattens a record into a `field -> string` map for notice macro expansion.
/// Built from serialization, so every record field is exposed as a
/// `{cache:<field>}` macro without per-field wiring; non-scalar shapes are
/// skipped.
fn record_fields(record: &Record) -> BTreeMap<String, String> {
    use serde_yaml_ng::Value;
    let mut fields = BTreeMap::new();
    let Ok(Value::Mapping(map)) = serde_yaml_ng::to_value(record) else {
        return fields;
    };
    for (key, value) in map {
        let Some(name) = key.as_str() else { continue };
        let text = match value {
            Value::String(text) => text,
            Value::Bool(flag) => flag.to_string(),
            Value::Number(number) => number.to_string(),
            Value::Null => String::new(),
            _ => continue,
        };
        fields.insert(name.to_owned(), text);
    }
    fields
}

/// True when a trusted, successful record for `command` in `dir` matches `digest`.
#[must_use]
pub fn is_fresh(dir: &Path, command: &str, digest: &str) -> bool {
    read(dir, command).is_some_and(|cached| cached.ok && cached.digest == digest)
}

/// Records the outcome of a run under `dir`, along with the artifact paths and
/// probe digests the rule carried at that moment. Best-effort: a write failure
/// is logged, never propagated, because the command has already run and its exit
/// code stands.
///
/// `clock` stamps `ran_at`. It is passed in rather than read here so the whole
/// process agrees on one instant (see [`crate::clock`]) — which is also what
/// makes a captured record reproducible under `MMZ_NOW`.
pub fn write(dir: &Path, command: &str, clock: Clock, outcome: &Outcome) {
    let record = Record {
        format: FORMAT,
        algorithm: ALGORITHM.to_owned(),
        command: command.to_owned(),
        input_digest: outcome.digest.to_owned(),
        status: if outcome.ok {
            Status::Ok
        } else {
            Status::Failed
        },
        ran_at: clock.now_secs(),
        outputs: outcome
            .outputs
            .iter()
            .map(|output| output.display().to_string())
            .collect(),
        probes: outcome.probes.clone(),
    };
    if let Err(err) = try_write(dir, command, &record) {
        log::warn!("mmz: could not write cache for `{command}`: {err}");
    }
}

/// Names the record or directory an operation refused, so a cache failure says
/// which path it was about rather than only what the errno was.
fn cache_io(path: &Path, source: std::io::Error) -> Error {
    Error::CacheIo {
        path: path.to_path_buf(),
        source,
    }
}

/// Removes records in `dir` whose stored command is not in `live`, returning the
/// pruned command names sorted. Leftover `.tmp` files from interrupted writes
/// are swept too. A record that cannot be read or parsed is left untouched, so
/// only confidently orphaned records are deleted. A missing directory is empty.
///
/// # Errors
///
/// Returns [`Error::CacheIo`] if the directory cannot be listed or a record
/// cannot be deleted, naming the path the operation was on.
pub fn prune(dir: &Path, live: &BTreeSet<String>) -> Result<Vec<String>> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => return Err(cache_io(dir, source)),
    };
    let mut pruned = Vec::new();
    for entry in entries {
        let path = entry.map_err(|source| cache_io(dir, source))?.path();
        match path.extension().and_then(|ext| ext.to_str()) {
            Some("tmp") => {
                let _ = fs::remove_file(&path);
            }
            Some("yaml") => {
                if let Some(command) = record_command(&path) {
                    if !live.contains(command.as_str()) {
                        fs::remove_file(&path).map_err(|source| cache_io(&path, source))?;
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
    fs::create_dir_all(dir).map_err(|source| cache_io(dir, source))?;
    let text = serde_yaml_ng::to_string(record).map_err(|err| Error::Serialize(Box::new(err)))?;
    let tmp = dir.join(format!("{}.{}.tmp", slug(command), std::process::id()));
    fs::write(&tmp, text).map_err(|source| cache_io(&tmp, source))?;
    let final_path = record_path(dir, command);
    if let Err(source) = fs::rename(&tmp, &final_path) {
        let _ = fs::remove_file(&tmp);
        return Err(cache_io(&final_path, source));
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

#[cfg(test)]
#[path = "cache_tests.rs"]
mod tests;
