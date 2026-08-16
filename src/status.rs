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
use crate::{cache, hashing, outputs, parametric, resolve};

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
/// record, when there is one. The state alone cannot carry it — a reader told
/// only "stale" goes looking at the inputs, which is the wrong place.
pub(crate) struct Assessment {
    pub(crate) state: State,
    pub(crate) missing_output: Option<String>,
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
    /// that is what voided the record. `None` when the rule is fresh.
    pub(crate) fn reason(&self) -> Option<String> {
        match self.missing_output.as_deref() {
            Some(path) => Some(format!("declared output `{path}` is missing")),
            None => self.state.reason().map(str::to_owned),
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

/// Resolves a rule's shared `inputs` glob set, run once per rule regardless of
/// how many expansions (parametric fan-outs) it has. [`expansion_files`] unions
/// this cached set with each expansion's bound file, so the walk itself never
/// repeats per expansion. One walk, or two when the rule mixes scopes that
/// honour the gitignore filter with scopes that opted out.
///
/// # Errors
///
/// Returns a resolution error when the rule's globs are invalid.
pub(crate) fn shared_inputs(
    manifest: &Manifest,
    rule: &Command,
    base: &Path,
) -> Result<Vec<String>> {
    resolve::expand_groups(&manifest.glob_groups(rule)?, base)
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
            missing_output: None,
            cached,
            inputs: Vec::new(),
        });
    }
    let inputs = hashing::hash_each(base, &files)?;
    let digest = hashing::digest_hashes(&inputs);
    let missing = outputs::first_missing(base, &hit.rule.outputs);
    let assessed = verdict(cached.as_ref(), &digest, missing);
    Ok(RuleStatus {
        name: identity,
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
    shared: &[String],
    base: &Path,
    cache_dir: &Path,
) -> Result<Assessment> {
    let files = expansion_files(shared, hit);
    if files.is_empty() {
        return Ok(Assessment {
            state: State::NoInputs,
            missing_output: None,
        });
    }
    let digest = hashing::digest_files(base, &files)?;
    Ok(verdict(
        read_cached(cache_dir, &hit.exp.identity).as_ref(),
        &digest,
        outputs::first_missing(base, &hit.rule.outputs),
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
/// gone artifact rather than sending them to diff the inputs.
fn verdict(cached: Option<&CachedInfo>, digest: &str, missing: Option<String>) -> Assessment {
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
    Assessment {
        state,
        missing_output,
    }
}

/// Renders the aligned `RULE / STATE / AGE` table. AGE is the time since the
/// rule's record was written, blank when it has none.
///
/// A fourth `MISSING OUTPUT` column appears only when some rule's record was
/// voided by a gone artifact, naming it: the path is what a reader needs, and
/// a column that is blank in every ordinary report is noise. Without it the
/// table is byte-identical to what it has always been.
fn render_text(report: &Report) -> String {
    let now = now_secs();
    let ages: Vec<String> = report
        .rules
        .iter()
        .map(|rule| {
            rule.cached.as_ref().map_or_else(String::new, |record| {
                humanize_age(now.saturating_sub(record.ran_at))
            })
        })
        .collect();
    let voided = report
        .rules
        .iter()
        .any(|rule| rule.missing_output.is_some());
    let rule_width = column_width(
        report.rules.iter().map(|rule| rule.name.chars().count()),
        "RULE",
    );
    let state_width = column_width(
        report.rules.iter().map(|rule| rule.state.label().len()),
        "STATE",
    );
    let age_width = if voided {
        column_width(ages.iter().map(|age| age.chars().count()), "AGE")
    } else {
        0
    };

    let row = |rule: &str, state: &str, age: &str, missing: &str| {
        let line =
            format!("{rule:<rule_width$}  {state:<state_width$}  {age:<age_width$}  {missing}");
        format!("{}\n", line.trim_end())
    };
    let mut out = row(
        "RULE",
        "STATE",
        "AGE",
        if voided { "MISSING OUTPUT" } else { "" },
    );
    for (rule, age) in report.rules.iter().zip(&ages) {
        out.push_str(&row(
            &rule.name,
            rule.state.label(),
            age,
            rule.missing_output.as_deref().unwrap_or(""),
        ));
    }
    out
}

/// The width of a table column: the widest cell, never narrower than `header`.
fn column_width(cells: impl Iterator<Item = usize>, header: &str) -> usize {
    cells.max().unwrap_or(0).max(header.len())
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
