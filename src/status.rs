//! `mmz --status`: report each rule's current freshness against its cache.
//!
//! For every rule the manifest declares, this resolves the rule's inputs,
//! recomputes their digest, and compares it to the stored record — answering
//! "would this rule skip or run right now, and why?" without running anything.
//!
//! Two renderings share one model: a human table (`mmz --status`) and a machine
//! report (`mmz --status=json`) that also lists every resolved input with its
//! content hash, so an operator can diff runs or `jq` out the changed file. The
//! JSON shape is described by [`SCHEMA`], printed by `mmz --status=json-schema`.

use std::collections::BTreeMap;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

use crate::error::{Error, Result};
use crate::manifest::{Command, Manifest};
use crate::{cache, hashing, parametric, resolve};

/// JSON Schema for the `mmz --status=json` output, emitted by
/// `mmz --status=json-schema`.
pub const SCHEMA: &str = include_str!("../schema/status.schema.json");

/// The full status report: the governing manifest and every rule's state.
#[derive(Serialize)]
struct Report {
    manifest: String,
    rules: Vec<RuleStatus>,
}

/// One rule's freshness, plus the inputs and digests behind the verdict.
#[derive(Serialize)]
struct RuleStatus {
    name: String,
    state: State,
    /// Digest of the current inputs; absent when the rule resolves to no files.
    #[serde(skip_serializing_if = "Option::is_none")]
    digest: Option<String>,
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
        }
    }

    /// True only for [`State::Fresh`] — the sole state `mmz --is-fresh` passes.
    pub(crate) const fn is_fresh(self) -> bool {
        matches!(self, Self::Fresh)
    }

    /// Why a non-fresh rule would re-run, for the `--is-fresh` gate's message.
    /// `None` when the rule is fresh.
    pub(crate) const fn reason(self) -> Option<&'static str> {
        match self {
            Self::Fresh => None,
            Self::Stale => Some("inputs changed since it last passed"),
            Self::Never => Some("never run"),
            Self::Failed => Some("last run failed"),
            Self::NoInputs => Some("resolved no input files"),
        }
    }
}

/// Builds the human-readable status table for the manifest governing `cwd`.
/// When `tags` is non-empty, only rules carrying every listed tag are
/// reported (an AND filter; untagged rules are skipped).
///
/// # Errors
///
/// Returns [`Error::NoManifest`] when none is found, a manifest error when one
/// cannot be loaded, or a resolution error when a rule's globs are invalid.
pub fn report(cwd: &Path, tags: &[String]) -> Result<String> {
    let report = collect(cwd, tags)?;
    if report.rules.is_empty() {
        return Ok(format!("no rules defined in {}\n", report.manifest));
    }
    Ok(render_text(&report))
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
    let located = Manifest::locate(cwd)?;
    let manifest = &located.manifest;
    let base = located.root.as_path();

    let cache_dir = base.join(&manifest.cache_dir);
    let mut matches = Vec::with_capacity(manifest.commands.len());
    let mut shared_by_rule: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for rule in &manifest.commands {
        if !tags.is_empty() && !tags.iter().all(|tag| rule.tags.contains(tag)) {
            continue;
        }
        shared_by_rule.insert(rule.name.clone(), shared_inputs(manifest, rule, base)?);
        matches.extend(parametric::expand_rule(manifest, base, rule)?);
    }
    parametric::detect_collision(&matches)?;
    let mut rules = Vec::with_capacity(matches.len());
    for hit in &matches {
        let shared = shared_by_rule
            .get(hit.rule.name.as_str())
            .expect("shared inputs resolved for every kept rule");
        rules.push(rule_status(hit, shared, base, &cache_dir)?);
    }
    Ok(Report {
        manifest: located.path.display().to_string(),
        rules,
    })
}

/// Resolves a rule's shared `inputs` glob set: one filesystem walk, run once
/// per rule regardless of how many expansions (parametric fan-outs) it has.
/// [`expansion_files`] unions this cached set with each expansion's bound
/// file, so the walk itself never repeats per expansion.
///
/// # Errors
///
/// Returns a resolution error when the rule's globs are invalid.
pub(crate) fn shared_inputs(
    manifest: &Manifest,
    rule: &Command,
    base: &Path,
) -> Result<Vec<String>> {
    resolve::expand(&manifest.globs_for(rule)?, base, manifest.gitignore)
}

/// Combines a rule's pre-resolved shared inputs with one expansion's bound
/// file (if any), sorted and deduped. Pure: it does no filesystem walk of its
/// own — callers resolve `shared` once per rule via [`shared_inputs`] and
/// reuse it across every expansion. The file set both [`rule_status`] and
/// [`expansion_state`] digest.
fn expansion_files(shared: &[String], hit: &parametric::Match) -> Vec<String> {
    let mut files = shared.to_vec();
    if let Some(file) = &hit.exp.file {
        files.push(file.clone());
        files.sort();
        files.dedup();
    }
    files
}

/// Computes one expansion's status: combine its pre-resolved shared inputs
/// with any bound file, hash them, and compare the digest against the stored
/// record.
fn rule_status(
    hit: &parametric::Match,
    shared: &[String],
    base: &Path,
    cache_dir: &Path,
) -> Result<RuleStatus> {
    let identity = hit.exp.identity.clone();
    let files = expansion_files(shared, hit);
    let cached = read_cached(cache_dir, &identity);
    if files.is_empty() {
        return Ok(RuleStatus {
            name: identity,
            state: State::NoInputs,
            digest: None,
            cached,
            inputs: Vec::new(),
        });
    }
    let inputs = hashing::hash_each(base, &files)?;
    let digest = hashing::digest_hashes(&inputs);
    let state = verdict(cached.as_ref(), &digest);
    Ok(RuleStatus {
        name: identity,
        state,
        digest: Some(digest),
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
    shared: &[String],
    base: &Path,
    cache_dir: &Path,
) -> Result<State> {
    let files = expansion_files(shared, hit);
    if files.is_empty() {
        return Ok(State::NoInputs);
    }
    let digest = hashing::digest_files(base, &files)?;
    Ok(verdict(
        read_cached(cache_dir, &hit.exp.identity).as_ref(),
        &digest,
    ))
}

/// Reads `name`'s record from `cache_dir` as the trusted view shared by the
/// status report and the freshness gate.
fn read_cached(cache_dir: &Path, name: &str) -> Option<CachedInfo> {
    cache::read(cache_dir, name).map(|cached| CachedInfo {
        digest: cached.digest,
        ok: cached.ok,
        ran_at: cached.ran_at,
    })
}

/// The freshness verdict for `digest` against a rule's stored record: fresh only
/// when the record is present, succeeded, and its digest matches.
fn verdict(cached: Option<&CachedInfo>, digest: &str) -> State {
    match cached {
        None => State::Never,
        Some(record) if !record.ok => State::Failed,
        Some(record) if record.digest == digest => State::Fresh,
        Some(_) => State::Stale,
    }
}

/// Renders the aligned `RULE / STATE / AGE` table. AGE is the time since the
/// rule's record was written, blank when it has none.
fn render_text(report: &Report) -> String {
    let now = now_secs();
    let rule_width = report
        .rules
        .iter()
        .map(|rule| rule.name.chars().count())
        .max()
        .unwrap_or(0)
        .max("RULE".len());
    let state_width = report
        .rules
        .iter()
        .map(|rule| rule.state.label().len())
        .max()
        .unwrap_or(0)
        .max("STATE".len());

    let row = |rule: &str, state: &str, age: &str| {
        let line = format!("{rule:<rule_width$}  {state:<state_width$}  {age}");
        format!("{}\n", line.trim_end())
    };
    let mut out = row("RULE", "STATE", "AGE");
    for rule in &report.rules {
        let age = rule.cached.as_ref().map_or_else(String::new, |record| {
            humanize_age(now.saturating_sub(record.ran_at))
        });
        out.push_str(&row(&rule.name, rule.state.label(), &age));
    }
    out
}

/// Renders a record's age as a coarse, human-readable span (`5s`, `3m`, `2h`,
/// `4d` ago).
fn humanize_age(secs: u64) -> String {
    const MINUTE: u64 = 60;
    const HOUR: u64 = 60 * MINUTE;
    const DAY: u64 = 24 * HOUR;
    if secs < MINUTE {
        format!("{secs}s ago")
    } else if secs < HOUR {
        format!("{}m ago", secs / MINUTE)
    } else if secs < DAY {
        format!("{}h ago", secs / HOUR)
    } else {
        format!("{}d ago", secs / DAY)
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

#[cfg(test)]
#[path = "status_tests.rs"]
mod tests;
