//! `mmz --status`: report each rule's current freshness against its cache.
//!
//! For every rule the manifest declares, this resolves the rule's inputs,
//! recomputes their digest, and compares it to the stored record — answering
//! "would this rule skip or run right now, and why?" without running anything.
//! Every rule also carries its `source`: the file that declared it, from the
//! [`crate::provenance::Provenance`] the manifest merge already built — a
//! composed project's rule can come from any file in its `imports:` chain, and
//! a reader debugging a surprising skip needs to know which one.
//!
//! Two renderings share one model: a human table (`mmz --status`, rendered by
//! [`table`]) and a machine report (`mmz --status=json`) that also lists every
//! resolved input with its content hash, so an operator can diff runs or `jq`
//! out the changed file. The JSON shape is described by [`SCHEMA`], printed by
//! `mmz --status=json-schema`. This module owns the report model — resolving
//! every rule's inputs and verdict — and leaves rendering the table to
//! [`table`], the seam that split off it once a `SOURCE` column needed a home.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Serialize;

use crate::clock::Clock;
use crate::error::{Error, Result};
use crate::manifest::{Command, Manifest};
use crate::provenance::Provenance;
use crate::{cache, hashing, outputs, parametric, probe, resolve};

/// JSON Schema for the `mmz --status=json` output, emitted by
/// `mmz --status=json-schema`.
pub const SCHEMA: &str = include_str!("../schema/status.schema.json");

/// The full status report: the governing manifest, every probe mmz resolved,
/// and every rule's state.
#[derive(Serialize)]
struct Report {
    manifest: String,
    /// The clock the `AGE` column ages every record against, resolved once for
    /// the whole report so two rows can never be measured from two instants.
    ///
    /// Not serialized: the JSON reports each record's stored `ran_at` and lets
    /// the consumer pick its own reference point, so putting one in the payload
    /// would add a field the schema does not declare. Resolving it for both
    /// renderings is deliberate all the same — a malformed `MMZ_NOW` is refused
    /// by `--status=json` exactly as it is by the table.
    #[serde(skip)]
    now: Clock,
    /// Each resolved probe's current digest, by name, so a consumer can see
    /// exactly what mmz saw. Omitted when no rule in the report named one.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    probes: BTreeMap<String, String>,
    rules: Vec<RuleStatus>,
}

/// One rule's freshness, plus the inputs and digests behind the verdict.
#[derive(Serialize)]
struct RuleStatus {
    name: String,
    /// The file that declared this rule, rendered through
    /// [`Provenance::display`]: project-root-relative when it is under the
    /// project root, absolute otherwise. Always present, including in a
    /// single-file project with no `imports:` — the root manifest is the
    /// source of its own rules, not a special case (see
    /// [`crate::provenance`]).
    source: String,
    state: State,
    /// Digest of the current inputs; absent when the rule resolves to no files.
    #[serde(skip_serializing_if = "Option::is_none")]
    digest: Option<String>,
    /// The declared output that voided the record, present exactly when the
    /// state is [`State::MissingOutput`].
    #[serde(skip_serializing_if = "Option::is_none")]
    missing_output: Option<String>,
    /// The stored record, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    cached: Option<CachedInfo>,
    /// Every resolved input with its content hash, sorted by path.
    inputs: Vec<hashing::FileHash>,
}

/// The trusted view of a rule's stored cache record.
#[derive(Serialize)]
struct CachedInfo {
    digest: String,
    ok: bool,
    /// Unix seconds when the run was recorded.
    ran_at: u64,
    /// The outputs the rule declared when the run was recorded; omitted when
    /// it declared none.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    outputs: Vec<String>,
    /// The digest each named probe produced when the run was recorded, so a
    /// consumer can diff it against the report's current `probes`; omitted
    /// when the rule named none.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    probes: BTreeMap<String, String>,
}

/// A rule's freshness verdict.
#[derive(Serialize, Clone, Copy)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum State {
    Fresh,
    Stale,
    Never,
    Failed,
    NoInputs,
    MissingOutput,
}

/// A verdict plus the detail behind it: the declared output that voided the
/// record, or the probe whose output moved, when there is one. The state alone
/// cannot carry either — a reader told only "stale" goes looking at the input
/// files, which in both cases is the wrong place.
pub(crate) struct Assessment {
    pub(crate) state: State,
    pub(crate) missing_output: Option<String>,
    pub(crate) changed_probe: Option<String>,
}

impl State {
    /// The label used in the human table; matches the JSON enum spelling.
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Fresh => "fresh",
            Self::Stale => "stale",
            Self::Never => "never",
            Self::Failed => "failed",
            Self::NoInputs => "no-inputs",
            Self::MissingOutput => "missing-output",
        }
    }

    /// True only for [`State::Fresh`] — the sole state `mmz --is-fresh` passes.
    pub(crate) const fn is_fresh(self) -> bool {
        matches!(self, Self::Fresh)
    }

    /// True for the non-fresh states a recorded pass can clear: `Stale`,
    /// `Never`, `Failed`, and `MissingOutput` — re-running the command under
    /// mmz regenerates the artifact and records the pass. `NoInputs` never has
    /// inputs to digest, so a wrapped run records nothing (or is refused under
    /// the `no_inputs` strictness) and the rule stays `NoInputs` — its remedy
    /// is fixing the manifest, not re-running under mmz.
    pub(crate) const fn is_remediable(self) -> bool {
        matches!(
            self,
            Self::Stale | Self::Never | Self::Failed | Self::MissingOutput
        )
    }

    /// Why a non-fresh rule would re-run, for the `--is-fresh` gate's message.
    /// `None` when the rule is fresh. The `MissingOutput` wording here is the
    /// fallback; the caller names the path (see [`Assessment`]).
    pub(crate) const fn reason(self) -> Option<&'static str> {
        match self {
            Self::Fresh => None,
            Self::Stale => Some("inputs changed since it last passed"),
            Self::Never => Some("never run"),
            Self::Failed => Some("last run failed"),
            Self::NoInputs => Some("resolved no input files"),
            Self::MissingOutput => Some("a declared output is missing"),
        }
    }
}

impl Assessment {
    /// The verdict's one-line explanation, naming the missing artifact when
    /// that is what voided the record, or the probe when that is what moved.
    /// `None` when the rule is fresh.
    pub(crate) fn reason(&self) -> Option<String> {
        if let Some(path) = self.missing_output.as_deref() {
            return Some(format!("declared output `{path}` is missing"));
        }
        if let Some(name) = self.changed_probe.as_deref() {
            return Some(format!("probe `{name}` changed since it last passed"));
        }
        self.state.reason().map(str::to_owned)
    }
}

/// Builds the human-readable status table for the manifest governing `cwd`.
/// When `tags` is non-empty, only rules carrying every listed tag are
/// reported (an AND filter; untagged rules are skipped).
///
/// # Errors
///
/// Returns [`Error::NoManifest`] when none is found, a manifest error when one
/// cannot be loaded, a resolution error when a rule's globs are invalid, or
/// [`Error::InvalidNow`] when `MMZ_NOW` is not a Unix epoch.
pub fn report(cwd: &Path, tags: &[String]) -> Result<String> {
    let report = collect(cwd, tags)?;
    if report.rules.is_empty() {
        return Ok(format!("no rules defined in {}\n", report.manifest));
    }
    Ok(table::render_text(&report))
}

/// Builds the `mmz --status=json` report: the same model as [`report`],
/// serialized to pretty JSON with each rule's resolved inputs and hashes.
/// `tags` filters as in [`report`].
///
/// # Errors
///
/// Same as [`report`], plus [`Error::Internal`] if serialization fails.
pub fn report_json(cwd: &Path, tags: &[String]) -> Result<String> {
    let report = collect(cwd, tags)?;
    let text = serde_json::to_string_pretty(&report)
        .map_err(|err| Error::Internal(format!("serializing status json: {err}")))?;
    Ok(format!("{text}\n"))
}

/// Resolves the manifest and computes every rule's status once, for either
/// rendering to consume. When `tags` is non-empty, a rule is skipped unless
/// it carries every listed tag.
fn collect(cwd: &Path, tags: &[String]) -> Result<Report> {
    let now = Clock::resolve()?;
    let located = Manifest::locate(cwd)?;
    let manifest = &located.manifest;
    let base = located.root.as_path();

    let cache_dir = base.join(&manifest.cache_dir);
    let mut matches = Vec::with_capacity(manifest.commands.len());
    let mut shared_by_rule: BTreeMap<String, Shared> = BTreeMap::new();
    let mut probes = probe::Resolver::new(manifest, base);
    for rule in &manifest.commands {
        if !tags.is_empty() && !tags.iter().all(|tag| rule.tags.contains(tag)) {
            continue;
        }
        shared_by_rule.insert(
            rule.name.clone(),
            shared_inputs(manifest, rule, base, &mut probes)?,
        );
        matches.extend(parametric::expand_rule(manifest, base, rule)?);
    }
    parametric::detect_collision(&matches)?;
    let mut rules = Vec::with_capacity(matches.len());
    for hit in &matches {
        let shared = shared_by_rule
            .get(hit.rule.name.as_str())
            .expect("shared inputs resolved for every kept rule");
        let source = located
            .provenance
            .commands
            .get(hit.rule.name.as_str())
            .map(|path| Provenance::display(path, base))
            .expect("provenance recorded for every declared command");
        rules.push(rule_status(hit, shared, base, &cache_dir, source)?);
    }
    Ok(Report {
        manifest: located.path.display().to_string(),
        now,
        probes: probes.resolved().clone(),
        rules,
    })
}

/// A rule's resolved shared inputs: the files its `inputs:` scopes expand to,
/// plus the digest of every probe those same `inputs:` name.
///
/// Resolved once per rule — the file set is unioned with each expansion's bound
/// file by [`expansion_files`], and a rule's probe digests are identical across
/// all of its expansions.
pub(crate) struct Shared {
    files: Vec<String>,
    probes: BTreeMap<String, String>,
}

/// Resolves a rule's shared `inputs` — its glob set and its probes — run once
/// per rule regardless of how many expansions (parametric fan-outs) it has.
/// [`expansion_files`] unions the cached file set with each expansion's bound
/// file, so the walk itself never repeats per expansion. One walk, or two when
/// the rule mixes scopes that honour the gitignore filter with scopes that
/// opted out. `probes` carries its memo across rules, so a probe eighteen rules
/// share still runs once.
///
/// # Errors
///
/// Returns a resolution error when the rule's globs are invalid, or a probe
/// error when one of its probes fails, cannot be run, or prints nothing.
pub(crate) fn shared_inputs(
    manifest: &Manifest,
    rule: &Command,
    base: &Path,
    probes: &mut probe::Resolver,
) -> Result<Shared> {
    Ok(Shared {
        files: resolve::expand_groups(&manifest.glob_groups(rule)?, base)?,
        probes: probes.for_rule(rule)?,
    })
}

/// Combines a rule's pre-resolved shared files with one expansion's bound
/// file (if any), sorted and deduped. Pure: it does no filesystem walk of its
/// own — callers resolve `shared` once per rule via [`shared_inputs`] and
/// reuse it across every expansion. The file set both [`rule_status`] and
/// [`expansion_state`] digest.
fn expansion_files(shared: &Shared, hit: &parametric::Match) -> Vec<String> {
    let mut files = shared.files.clone();
    if let Some(file) = &hit.exp.file {
        files.push(file.clone());
        files.sort();
        files.dedup();
    }
    files
}

/// Computes one expansion's status: combine its pre-resolved shared inputs
/// with any bound file, hash them, and compare the digest against the stored
/// record. `source` is the rule's declaring file, already resolved by the
/// caller — every expansion of a rule shares one source, so resolving it once
/// per rule (rather than per expansion, like [`Shared`]) needs no memoization.
fn rule_status(
    hit: &parametric::Match,
    shared: &Shared,
    base: &Path,
    cache_dir: &Path,
    source: String,
) -> Result<RuleStatus> {
    let identity = hit.exp.identity.clone();
    let files = expansion_files(shared, hit);
    let cached = read_cached(cache_dir, &identity);
    if files.is_empty() && shared.probes.is_empty() {
        return Ok(RuleStatus {
            name: identity,
            source,
            state: State::NoInputs,
            digest: None,
            missing_output: None,
            cached,
            inputs: Vec::new(),
        });
    }
    let inputs = hashing::hash_each(base, &files)?;
    let digest = hashing::digest_all(&inputs, &shared.probes);
    let missing = outputs::first_missing(base, &hit.rule.outputs);
    let assessed = verdict(cached.as_ref(), &digest, missing, &shared.probes);
    Ok(RuleStatus {
        name: identity,
        source,
        state: assessed.state,
        digest: Some(digest),
        missing_output: assessed.missing_output,
        cached,
        inputs,
    })
}

/// Computes one expansion's freshness without the per-input detail
/// [`rule_status`] gathers: combine its pre-resolved shared inputs with any
/// bound file, digest them, and compare to the record keyed on the
/// expansion's identity. The per-expansion core the `mmz --is-fresh` gate
/// evaluates, keying static and parametric rules alike on
/// [`parametric::Match`] — `parametric::expand_rule` yields a single match
/// whose identity is the bare rule name for a static rule, so this collapses
/// to `rule_state`'s old behaviour there.
///
/// # Errors
///
/// Returns a hashing error when an input cannot be read.
pub(crate) fn expansion_state(
    hit: &parametric::Match,
    shared: &Shared,
    base: &Path,
    cache_dir: &Path,
) -> Result<Assessment> {
    let files = expansion_files(shared, hit);
    if files.is_empty() && shared.probes.is_empty() {
        return Ok(Assessment {
            state: State::NoInputs,
            missing_output: None,
            changed_probe: None,
        });
    }
    let digest = hashing::digest_with(base, &files, &shared.probes)?;
    Ok(verdict(
        read_cached(cache_dir, &hit.exp.identity).as_ref(),
        &digest,
        outputs::first_missing(base, &hit.rule.outputs),
        &shared.probes,
    ))
}

/// Reads `name`'s record from `cache_dir` as the trusted view shared by the
/// status report and the freshness gate.
fn read_cached(cache_dir: &Path, name: &str) -> Option<CachedInfo> {
    cache::read(cache_dir, name).map(|cached| CachedInfo {
        digest: cached.digest,
        ok: cached.ok,
        ran_at: cached.ran_at,
        outputs: cached.outputs,
        probes: cached.probes,
    })
}

/// The freshness verdict for `digest` against a rule's stored record: fresh
/// only when the record is present, succeeded, its digest matches, and every
/// output the rule declares is still on disk (`missing` is the first one that
/// is not).
///
/// A missing artifact outranks a digest mismatch. Both would re-run the rule,
/// but only one of them is a fact a reader would otherwise never guess — an
/// input change is the assumption they already hold — so the verdict names the
/// gone artifact rather than sending them to diff the inputs. A stale verdict
/// gets the same treatment for probes: when the record's stored probe digests
/// show one moved, `probes` is compared against them and the culprit is named,
/// because no file changed and diffing the files would find nothing.
fn verdict(
    cached: Option<&CachedInfo>,
    digest: &str,
    missing: Option<String>,
    probes: &BTreeMap<String, String>,
) -> Assessment {
    let state = match cached {
        None => State::Never,
        Some(record) if !record.ok => State::Failed,
        Some(_) if missing.is_some() => State::MissingOutput,
        Some(record) if record.digest == digest => State::Fresh,
        Some(_) => State::Stale,
    };
    let missing_output = match state {
        State::MissingOutput => missing,
        _ => None,
    };
    let changed_probe = match state {
        State::Stale => cached.and_then(|record| probe::first_changed(&record.probes, probes)),
        _ => None,
    };
    Assessment {
        state,
        missing_output,
        changed_probe,
    }
}

#[path = "status_table.rs"]
mod table;

#[cfg(test)]
#[path = "status_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "status_source_tests.rs"]
mod source_tests;
