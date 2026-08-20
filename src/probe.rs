//! Command-driven inputs: named probes whose stdout joins a rule's input digest.
//!
//! A scope can only name whole files, so a rule that depends on part of a file
//! has to hash all of it — one recipe body in a `Justfile` costs every rule
//! sharing that file. A probe closes the gap: `run` is a shell command line, its
//! stdout is hashed, and that hash joins the input digest of every rule whose
//! `inputs:` names the probe, exactly as a scope name does.
//!
//! ```yaml
//! probes:
//!   fmt-recipe:
//!     run: just --dump --dump-format json | jq -S -e -c '.recipes["fmt-check"]'
//! commands:
//!   - name: just fmt-check
//!     inputs: [rust, fmt-recipe]
//! ```
//!
//! # Failure modes, and where each one is owned
//!
//! The primitive can lie in a way a file hash cannot, so the boundary is the
//! design:
//!
//! - A probe that exits non-zero is a hard error naming the probe, its exit
//!   code, and its stderr. mmz exits without consuming the output and without
//!   writing a record — a failed command never reaches the hasher.
//! - A probe that cannot be spawned is the same error.
//! - Empty stdout is an error by default, with `allow_empty: true` to opt in. It
//!   is the cheapest catch for a selector that matched nothing.
//! - Content correctness is the consumer's. A probe that prints valid but wrong
//!   output, or that is not deterministic, is the manifest author's bug: pin the
//!   ordering, strip the timestamps, assert the shape in the probe itself
//!   (`jq -S -e`, a schema check) so a bad shape becomes a non-zero exit and hits
//!   the rule above. mmz does not validate meaning, and should not learn to.
//!
//! A wrong scope costs time; a wrong probe can lie. That asymmetry is why every
//! case above fails closed.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::{Command as Process, Output, Stdio};

use serde::Deserialize;

use crate::error::{Error, Result};
use crate::hashing;
use crate::manifest::{Command, Manifest, Scope};

/// The shell a probe's `run` line is handed to, so a pipeline, a quoted
/// argument, or a redirect all work as written.
const SHELL: &str = "sh";

/// Longest stderr excerpt carried in a failure message, in bytes. A runaway
/// probe should not bury its own first line under a megabyte of noise.
const STDERR_CAP: usize = 2000;

/// A named command whose stdout is an input.
///
/// The `run` line is executed by `sh -c` from the project root — the same base
/// that scope globs resolve against, so a probe reading the project's own files
/// needs no path juggling. stdin is closed, so a probe that waits on input fails
/// instead of hanging a gate; stderr is captured and surfaced only when the
/// probe fails.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Probe {
    /// The command line whose stdout is hashed.
    pub run: String,
    /// When true, stdout that is empty (or only whitespace) is a valid input
    /// rather than an error. Default false — see the module docs.
    #[serde(default)]
    pub allow_empty: bool,
}

/// Rejects a probe that shares a name with a scope.
///
/// `inputs:` has one namespace, so a reader must never have to guess which kind
/// a name is — and mmz must never have to pick one.
///
/// # Errors
///
/// Returns [`Error::NameCollision`] naming the doubly-claimed name.
pub fn validate(probes: &BTreeMap<String, Probe>, scopes: &BTreeMap<String, Scope>) -> Result<()> {
    match probes.keys().find(|name| scopes.contains_key(*name)) {
        Some(name) => Err(Error::NameCollision { name: name.clone() }),
        None => Ok(()),
    }
}

/// The first probe whose digest moved since a record was written, comparing the
/// record's stored map against the current one.
///
/// A stale rule whose probe changed should say so: sending a reader to diff the
/// files when no file moved is the same wrong-place failure a missing output
/// would cause. A probe absent from the record (newly added to the rule) counts
/// as changed. Names compare in sorted order, so the answer is stable.
#[must_use]
pub fn first_changed(
    recorded: &BTreeMap<String, String>,
    current: &BTreeMap<String, String>,
) -> Option<String> {
    current
        .iter()
        .find(|(name, hash)| recorded.get(name.as_str()) != Some(*hash))
        .map(|(name, _)| name.clone())
}

/// Resolves probes for one mmz invocation, running each declared probe at most
/// once however many rules name it.
///
/// A bare `mmz --is-fresh` gates every rule in the manifest and runs in git
/// hooks, so the shape that matters is many rules sharing one probe: eighteen
/// references must cost one process, not eighteen. The memo is per-[`Resolver`]
/// rather than a process-global static on purpose — a long-lived library caller
/// would otherwise be pinned forever to the first digest it ever saw, which is
/// exactly the stale input mmz exists to prevent. `mmz` builds one resolver per
/// invocation, so for the CLI the two readings coincide.
pub struct Resolver<'a> {
    manifest: &'a Manifest,
    base: &'a Path,
    seen: BTreeMap<String, String>,
}

impl<'a> Resolver<'a> {
    /// A resolver for `manifest`, running probes from the project root `base`.
    #[must_use]
    pub const fn new(manifest: &'a Manifest, base: &'a Path) -> Self {
        Self {
            manifest,
            base,
            seen: BTreeMap::new(),
        }
    }

    /// Digests every probe `rule` names in `inputs`, keyed by probe name.
    ///
    /// Entries naming a scope are skipped here — [`Manifest::glob_groups`] owns
    /// those — and a probe an earlier rule already resolved is reused rather
    /// than re-run.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ProbeSpawn`], [`Error::ProbeFailed`], or
    /// [`Error::ProbeEmpty`]. Each is a hard stop: the caller never reaches the
    /// hasher, so no record is written against an output mmz could not trust.
    pub fn for_rule(&mut self, rule: &Command) -> Result<BTreeMap<String, String>> {
        let mut named = BTreeMap::new();
        for name in &rule.inputs {
            let Some(probe) = self.manifest.probes.get(name) else {
                continue;
            };
            let hash = self.memoized(name, probe)?;
            named.insert(name.clone(), hash);
        }
        Ok(named)
    }

    /// The digest of the probe named `name`, running it only when this
    /// resolver has not already seen it. The whole point of the type.
    fn memoized(&mut self, name: &str, probe: &Probe) -> Result<String> {
        if let Some(known) = self.seen.get(name) {
            return Ok(known.clone());
        }
        let fresh = digest(name, probe, self.base)?;
        self.seen.insert(name.to_owned(), fresh.clone());
        Ok(fresh)
    }

    /// Every probe resolved so far, name to digest — what `mmz --status=json`
    /// reports as the current view. A declared probe that no rule names is
    /// never run, so it is absent here: mmz reports what it saw.
    #[must_use]
    pub const fn resolved(&self) -> &BTreeMap<String, String> {
        &self.seen
    }
}

/// Runs one probe from `base` and hashes its stdout.
///
/// Every failure path returns before the hash, so a command that failed, could
/// not start, or printed nothing never contributes bytes to an input digest.
fn digest(name: &str, probe: &Probe, base: &Path) -> Result<String> {
    let output = capture(name, probe, base)?;
    if !output.status.success() {
        return Err(Error::ProbeFailed {
            name: name.to_owned(),
            run: probe.run.clone(),
            code: output.status.code().unwrap_or(1),
            stderr: excerpt(&output.stderr),
        });
    }
    if !probe.allow_empty && output.stdout.iter().all(u8::is_ascii_whitespace) {
        return Err(Error::ProbeEmpty {
            name: name.to_owned(),
            run: probe.run.clone(),
        });
    }
    Ok(hashing::hash_bytes(&output.stdout))
}

/// Spawns the probe under [`SHELL`] from the project root, with stdin closed and
/// both output streams captured.
fn capture(name: &str, probe: &Probe, base: &Path) -> Result<Output> {
    Process::new(SHELL)
        .arg("-c")
        .arg(&probe.run)
        .current_dir(base)
        .stdin(Stdio::null())
        .output()
        .map_err(|source| Error::ProbeSpawn {
            name: name.to_owned(),
            run: probe.run.clone(),
            source,
        })
}

/// Renders a probe's captured stderr for a failure message: trimmed, capped at
/// [`STDERR_CAP`], and named explicitly when the probe said nothing at all (so
/// the message never trails off into blank space).
fn excerpt(stderr: &[u8]) -> String {
    let text = String::from_utf8_lossy(stderr);
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return "(none)".to_owned();
    }
    match trimmed.char_indices().nth(STDERR_CAP) {
        Some((cut, _)) => format!("{}…", &trimmed[..cut]),
        None => trimmed.to_owned(),
    }
}

#[cfg(test)]
#[path = "probe_tests.rs"]
mod tests;
