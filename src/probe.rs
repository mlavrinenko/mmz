//! Narrowed inputs: named probes whose value joins a rule's input digest.
//!
//! A scope can only name whole files, so a rule that depends on part of a file
//! has to hash all of it — one recipe body in a `Justfile` costs every rule
//! sharing that file. A probe closes the gap: it produces bytes, they are
//! hashed, and that hash joins the input digest of every rule whose `inputs:`
//! names the probe, exactly as a scope name does.
//!
//! # Two sources, one of them free
//!
//! `file:` plus `json:` reads a file and selects out of it **with no process at
//! all** — nothing spawned, nothing on `PATH`, no shell quoting, and no
//! per-probe process on a `mmz --is-fresh` that gates every rule at once:
//!
//! ```yaml
//! probes:
//!   qahq-input:
//!     file: flake.lock
//!     json: '.nodes["qahq"]["locked"]["narHash"]'
//! ```
//!
//! `run:` is the other source: a shell command line whose stdout is the bytes.
//! It may carry a `json:` too, which selects out of that stdout instead of a
//! file — half the spawns of the `| jq …` spelling, and canonical by
//! construction (see below).
//!
//! ```yaml
//! probes:
//!   fmt-recipe:
//!     run: just --dump --dump-format json
//!     json: '.recipes["fmt-check"]'
//! commands:
//!   - name: just fmt-check
//!     inputs: [rust, fmt-recipe]
//! ```
//!
//! The two sources are mutually exclusive, and a probe must declare one. A
//! `file:` without a `json:` is refused too: hashing a whole file is what a
//! scope is for, and a second spelling of it here would quietly skip the
//! gitignore filter and the per-file digest a scope gives for free.
//!
//! # Why the selection is hashed, not the bytes
//!
//! A `json:` probe hashes mmz's own canonical rendering of the selected value
//! — object keys sorted, arrays left alone — never the input's bytes. So key
//! order stops being an input by construction rather than by every author
//! remembering `jq -S`, which is a convention someone can forget and this repo
//! once did. See [`crate::json`].
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
//! - A `file:` that does not exist, or cannot be read, is a hard error naming
//!   the probe and the path. The path resolves against the project root — the
//!   same base scope globs use.
//! - Bytes that are not one JSON value are a hard error, whether they came off
//!   disk or out of a command. A tool that logs a line before its JSON, or a
//!   half-written lockfile, is a state mmz refuses rather than hashes.
//! - A `json:` program that does not compile, or that raises against the
//!   document it was pointed at, is a hard error naming the probe and quoting
//!   the program.
//! - **A `json:` selector that yields nothing — no output at all, or a lone
//!   `null` — is a hard error.** This is the same refusal as empty stdout, at
//!   the place the selection happens, and it is why `jq -e` is load-bearing on
//!   every shelled-out probe here: a probe tracking `null` reports one digest
//!   whatever the document does, so the rule reads fresh forever against an
//!   input nobody is measuring. `false` is a value and passes; jq's `-e`
//!   conflates it with `null` only because a shell exit code cannot tell them
//!   apart.
//! - `allow_empty: true` opts into exactly that, on both sources: with `json:`
//!   it accepts a selection that yielded nothing or only `null`, and without it
//!   it accepts whitespace-only stdout. One key, one meaning — "empty really is
//!   a valid input here" — so a reader who knows it from `run:` already knows
//!   what it does beside `json:`.
//! - Content correctness is the consumer's. A probe that prints valid but wrong
//!   output, or that is not deterministic, is the manifest author's bug: pin the
//!   ordering, strip the timestamps, assert the shape in the probe itself
//!   (`jq -S -e`, a schema check) so a bad shape becomes a non-zero exit and hits
//!   the rule above. mmz does not validate meaning, and should not learn to.
//!
//! - The environment is the manifest's too. A `run` line resolves through
//!   whatever `PATH` the caller had, so the same probe measured under two
//!   shells can disagree without the project changing at all. `probe_shell`
//!   pins the argv the line is handed to — `["direnv", "exec", ".", "sh",
//!   "-c"]` and the like — so a probe reading project tooling measures the
//!   project's tooling rather than the operator's. mmz cannot detect the
//!   mismatch, which is why the key exists to prevent it.
//!
//! A wrong scope costs time; a wrong probe can lie. That asymmetry is why every
//! case above fails closed.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command as Process, Output, Stdio};

use serde::Deserialize;

use crate::error::{Error, Result};
use crate::hashing;
use crate::json;
use crate::manifest::{Command, Manifest, Scope};

/// The shell a probe's `run` line is handed to when the manifest does not set
/// `probe_shell`, so a pipeline, a quoted argument, or a redirect all work as
/// written. Lives in [`crate::manifest::default_probe_shell`]; named here only
/// so the module that spawns it says which default it spawns.
pub(crate) const DEFAULT_SHELL: [&str; 2] = ["sh", "-c"];

/// Longest stderr excerpt carried in a failure message, in bytes. A runaway
/// probe should not bury its own first line under a megabyte of noise.
const STDERR_CAP: usize = 2000;

/// A named source of input bytes: a file to read, or a command to run, and
/// optionally a jq program to select out of it.
///
/// A `file` path and a `run` line both resolve against the project root — the
/// same base that scope globs resolve against, so a probe reading the project's
/// own files needs no path juggling. A `run` line is executed by `sh -c` with
/// stdin closed, so a probe that waits on input fails instead of hanging a
/// gate; stderr is captured and surfaced only when the probe fails.
///
/// Every field is optional here because the legal combinations are a rule about
/// the *set* of keys, not about any one of them — and refusing them at
/// [`validate`] rather than in a `serde` conversion is what lets the error name
/// the probe. See [`Probe::check_shape`].
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Probe {
    /// The command line whose stdout is the probe's bytes. Mutually exclusive
    /// with [`Probe::file`].
    #[serde(default)]
    pub run: Option<String>,
    /// A file, relative to the project root, whose contents are the probe's
    /// bytes. Requires [`Probe::json`]: hashing a whole file is a scope's job.
    #[serde(default)]
    pub file: Option<PathBuf>,
    /// A jq program selecting out of those bytes. The selected value is
    /// rendered canonically and hashed, so object key order never reaches the
    /// digest. See [`crate::json`].
    #[serde(default)]
    pub json: Option<String>,
    /// When true, a selection that yielded nothing — or, without `json`,
    /// stdout that is empty or only whitespace — is a valid input rather than
    /// an error. Default false; see the module docs for why.
    #[serde(default)]
    pub allow_empty: bool,
}

/// What a probe reads, once the shape rules have ruled out the combinations
/// that name neither source or both.
enum Source<'a> {
    /// The stdout of a command line.
    Run(&'a str),
    /// The contents of a file, relative to the project root.
    File(&'a Path),
}

/// Which combinations of `run:`, `file:` and `json:` are legal, and what each
/// illegal one says — split out to `probe_shape.rs` once this file reached its
/// line cap. Still `impl Probe`, so no call site knows the split happened.
#[path = "probe_shape.rs"]
mod shape;

/// Rejects a probe that shares a name with a scope, or whose source keys do
/// not describe one readable thing.
///
/// `inputs:` has one namespace, so a reader must never have to guess which kind
/// a name is — and mmz must never have to pick one. The shape check runs over
/// every declared probe, not only the ones a rule names, so a malformed probe
/// is refused the moment the manifest loads rather than on whichever
/// invocation first happens to reach it.
///
/// # Errors
///
/// Returns [`Error::EmptyProbeShell`], [`Error::NameCollision`] naming the
/// doubly-claimed name, or [`Error::ProbeSource`] naming the malformed probe.
pub fn validate(
    probes: &BTreeMap<String, Probe>,
    scopes: &BTreeMap<String, Scope>,
    shell: &[String],
) -> Result<()> {
    if shell.is_empty() {
        return Err(Error::EmptyProbeShell);
    }
    if let Some(name) = probes.keys().find(|name| scopes.contains_key(*name)) {
        return Err(Error::NameCollision { name: name.clone() });
    }
    for (name, probe) in probes {
        probe.check_shape(name)?;
    }
    Ok(())
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
        let fresh = digest(name, probe, self.base, &self.manifest.probe_shell)?;
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

/// Resolves one probe from `base` and hashes what it produced.
///
/// Every failure path returns before the hash, so a command that failed, a file
/// that would not read, a document that would not parse, and a selector that
/// matched nothing all stop here rather than contributing bytes to an input
/// digest.
fn digest(name: &str, probe: &Probe, base: &Path, shell: &[String]) -> Result<String> {
    let bytes = match probe.source(name)? {
        Source::Run(run) => stdout_of(name, run, base, shell)?,
        Source::File(path) => read_file(name, path, base)?,
    };
    let Some(program) = &probe.json else {
        return hash_raw(name, probe, &bytes);
    };
    hash_selection(name, probe, program, &bytes)
}

/// Runs the probe's `run` line and returns its stdout, refusing a non-zero
/// exit before the bytes are looked at.
fn stdout_of(name: &str, run: &str, base: &Path, shell: &[String]) -> Result<Vec<u8>> {
    let output = capture(name, run, base, shell)?;
    if output.status.success() {
        return Ok(output.stdout);
    }
    Err(Error::ProbeFailed {
        name: name.to_owned(),
        run: run.to_owned(),
        code: output.status.code().unwrap_or(1),
        stderr: excerpt(&output.stderr),
    })
}

/// Reads the probe's `file`, relative to the project root — the base scope
/// globs resolve against, so the two kinds of input name a path the same way.
fn read_file(name: &str, path: &Path, base: &Path) -> Result<Vec<u8>> {
    std::fs::read(base.join(path)).map_err(|source| Error::ProbeFileUnreadable {
        name: name.to_owned(),
        path: path.to_path_buf(),
        source,
    })
}

/// Hashes the bytes as they came, for a probe with no `json:`. Only a `run:`
/// probe reaches this, so the empty-output message can quote its command line.
fn hash_raw(name: &str, probe: &Probe, bytes: &[u8]) -> Result<String> {
    if !probe.allow_empty && bytes.iter().all(u8::is_ascii_whitespace) {
        return Err(Error::ProbeEmpty {
            name: name.to_owned(),
            run: probe.run.clone().unwrap_or_default(),
        });
    }
    Ok(hashing::hash_bytes(bytes))
}

/// Selects out of the bytes with `program` and hashes the canonical rendering
/// of every value it produced.
///
/// The outputs are joined one per line, the shape `jq -c` itself prints, so a
/// two-output selector (`.a, .b`) has a stable digest and one that grew a
/// second output has a different one. Ordering is jq's own evaluation order,
/// which is deterministic.
fn hash_selection(name: &str, probe: &Probe, program: &str, bytes: &[u8]) -> Result<String> {
    let selected = crate::json::select(program, bytes).map_err(|failure| match failure {
        json::Failure::Input(reason) => Error::ProbeJsonInput {
            name: name.to_owned(),
            origin: probe.origin(),
            reason,
        },
        json::Failure::Program(reason) | json::Failure::Run(reason) => Error::ProbeJsonFailed {
            name: name.to_owned(),
            program: program.to_owned(),
            origin: probe.origin(),
            reason,
        },
    })?;
    if !probe.allow_empty && selected_nothing(&selected) {
        return Err(Error::ProbeJsonEmpty {
            name: name.to_owned(),
            program: program.to_owned(),
            origin: probe.origin(),
        });
    }
    let mut joined = Vec::new();
    for value in &selected {
        joined.extend_from_slice(value);
        joined.push(b'\n');
    }
    Ok(hashing::hash_bytes(&joined))
}

/// Whether a selection measured nothing: no outputs at all, or the single
/// output `null`. See the module docs for why `false` is not in this list.
fn selected_nothing(selected: &[Vec<u8>]) -> bool {
    match selected {
        [] => true,
        [only] => only.as_slice() == b"null",
        _ => false,
    }
}

/// Spawns the probe under `shell` from the project root, with stdin closed and
/// both output streams captured.
///
/// `shell` is the manifest's `probe_shell` (default [`DEFAULT_SHELL`]): its
/// first element is the program, the rest are fixed arguments, and the probe's
/// `run` line is appended as one final argument. [`validate`] has already
/// refused an empty list, so the indexing below cannot panic.
fn capture(name: &str, run: &str, base: &Path, shell: &[String]) -> Result<Output> {
    let (program, leading) = shell
        .split_first()
        .expect("probe_shell is non-empty; validate rejects an empty list at load");
    Process::new(program)
        .args(leading)
        .arg(run)
        .current_dir(base)
        .stdin(Stdio::null())
        .output()
        .map_err(|source| Error::ProbeSpawn {
            name: name.to_owned(),
            run: run.to_owned(),
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

#[cfg(test)]
#[path = "probe_json_tests.rs"]
mod json_tests;
