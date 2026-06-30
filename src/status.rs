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

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

use crate::error::{Error, Result};
use crate::manifest::{Command as Rule, Manifest};
use crate::{cache, hashing, resolve};

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
///
/// # Errors
///
/// Returns [`Error::NoManifest`] when none is found, a manifest error when one
/// cannot be loaded, or a resolution error when a rule's globs are invalid.
pub fn report(cwd: &Path) -> Result<String> {
    let report = collect(cwd)?;
    if report.rules.is_empty() {
        return Ok(format!("no rules defined in {}\n", report.manifest));
    }
    Ok(render_text(&report))
}

/// Builds the `mmz --status=json` report: the same model as [`report`],
/// serialized to pretty JSON with each rule's resolved inputs and hashes.
///
/// # Errors
///
/// Same as [`report`], plus [`Error::Internal`] if serialization fails.
pub fn report_json(cwd: &Path) -> Result<String> {
    let report = collect(cwd)?;
    let text = serde_json::to_string_pretty(&report)
        .map_err(|err| Error::Internal(format!("serializing status json: {err}")))?;
    Ok(format!("{text}\n"))
}

/// Resolves the manifest and computes every rule's status once, for either
/// rendering to consume.
fn collect(cwd: &Path) -> Result<Report> {
    let located = Manifest::locate(cwd)?;
    let manifest = &located.manifest;
    let base = located.root.as_path();

    let cache_dir = base.join(&manifest.cache_dir);
    let mut rules = Vec::with_capacity(manifest.commands.len());
    for rule in &manifest.commands {
        rules.push(rule_status(manifest, rule, base, &cache_dir)?);
    }
    Ok(Report {
        manifest: located.path.display().to_string(),
        rules,
    })
}

/// Computes one rule's status: resolve its inputs, hash them, and compare the
/// digest against the stored record.
fn rule_status(
    manifest: &Manifest,
    rule: &Rule,
    base: &Path,
    cache_dir: &Path,
) -> Result<RuleStatus> {
    let globs = manifest.globs_for(rule)?;
    let files = resolve::expand(&globs, base, manifest.gitignore)?;
    let cached = read_cached(cache_dir, &rule.name);
    if files.is_empty() {
        return Ok(RuleStatus {
            name: rule.name.clone(),
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
        name: rule.name.clone(),
        state,
        digest: Some(digest),
        cached,
        inputs,
    })
}

/// Computes one rule's freshness without the per-input detail [`rule_status`]
/// gathers: resolve the scopes, digest them, and compare to the record. The
/// per-rule core the `mmz --is-fresh` gate evaluates.
///
/// # Errors
///
/// Returns a resolution or hashing error when a rule's globs are invalid or an
/// input cannot be read.
pub(crate) fn rule_state(
    manifest: &Manifest,
    rule: &Rule,
    base: &Path,
    cache_dir: &Path,
) -> Result<State> {
    let globs = manifest.globs_for(rule)?;
    let files = resolve::expand(&globs, base, manifest.gitignore)?;
    if files.is_empty() {
        return Ok(State::NoInputs);
    }
    let digest = hashing::digest_files(base, &files)?;
    Ok(verdict(
        read_cached(cache_dir, &rule.name).as_ref(),
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
mod tests {
    use super::{SCHEMA, humanize_age, report, report_json};

    fn write(dir: &std::path::Path, name: &str, body: &str) {
        std::fs::write(dir.join(name), body).expect("write");
    }

    fn manifest(root: &std::path::Path, body: &str) {
        let dir = root.join(".mmz");
        std::fs::create_dir_all(&dir).expect("mkdir .mmz");
        std::fs::write(dir.join("config.yaml"), body).expect("write manifest");
    }

    #[test]
    fn humanize_age_scales_by_unit() {
        assert_eq!(humanize_age(5), "5s ago");
        assert_eq!(humanize_age(90), "1m ago");
        assert_eq!(humanize_age(3 * 3600), "3h ago");
        assert_eq!(humanize_age(2 * 86_400), "2d ago");
    }

    #[test]
    fn reports_never_then_fresh_then_stale() {
        let dir = tempfile::tempdir().expect("tempdir");
        let base = dir.path();
        write(base, "a.txt", "one");
        manifest(
            base,
            "scopes:\n  src: [\"*.txt\"]\ncommands:\n  - name: sh\n    inputs: [src]\n",
        );

        let never = report(base).expect("report");
        assert!(never.contains("sh") && never.contains("never"));

        let argv = ["sh".to_owned(), "-c".to_owned(), "exit 0".to_owned()];
        crate::run(&argv, base).expect("recorded run");
        assert!(
            report(base).expect("report").contains("fresh"),
            "fresh after a recorded run"
        );

        write(base, "a.txt", "two");
        assert!(
            report(base).expect("report").contains("stale"),
            "stale after an input changes"
        );
    }

    #[test]
    fn text_shows_age_after_a_run_and_json_reports_ran_at() {
        let dir = tempfile::tempdir().expect("tempdir");
        let base = dir.path();
        write(base, "a.txt", "one");
        manifest(
            base,
            "scopes:\n  src: [\"*.txt\"]\ncommands:\n  - name: sh\n    inputs: [src]\n",
        );
        let argv = ["sh".to_owned(), "-c".to_owned(), "exit 0".to_owned()];
        crate::run(&argv, base).expect("recorded run");

        assert!(
            report(base).expect("report").contains("ago"),
            "table shows a record age once a run is recorded"
        );
        let json: serde_json::Value =
            serde_json::from_str(&report_json(base).expect("json")).expect("valid json");
        let ran_at = json
            .pointer("/rules/0/cached/ran_at")
            .expect("ran_at present");
        assert!(
            ran_at.as_u64().is_some_and(|secs| secs > 0),
            "ran_at is a unix timestamp"
        );
    }

    #[test]
    fn reports_no_inputs_for_empty_scopes() {
        let dir = tempfile::tempdir().expect("tempdir");
        let base = dir.path();
        manifest(
            base,
            "scopes:\n  none: [\"*.none\"]\ncommands:\n  - name: sh\n    inputs: [none]\n",
        );
        let report = report(base).expect("report");
        assert!(report.contains("sh") && report.contains("no-inputs"));
    }

    #[test]
    fn missing_manifest_is_an_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        assert!(report(dir.path()).is_err());
    }

    #[test]
    fn json_lists_inputs_with_hashes_and_state() {
        let dir = tempfile::tempdir().expect("tempdir");
        let base = dir.path();
        write(base, "a.txt", "one");
        manifest(
            base,
            "scopes:\n  src: [\"*.txt\"]\ncommands:\n  - name: sh\n    inputs: [src]\n",
        );

        let json: serde_json::Value =
            serde_json::from_str(&report_json(base).expect("json")).expect("valid json");
        let rule = json.pointer("/rules/0").expect("first rule");
        let str_at = |value: &serde_json::Value, key: &str| {
            value
                .get(key)
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        };
        assert_eq!(str_at(rule, "name").as_deref(), Some("sh"));
        assert_eq!(str_at(rule, "state").as_deref(), Some("never"));
        let input = rule.pointer("/inputs/0").expect("first input");
        assert_eq!(str_at(input, "path").as_deref(), Some("a.txt"));
        assert_eq!(
            str_at(input, "hash").as_deref().map(str::len),
            Some(64),
            "per-file blake3 hex is reported"
        );
        assert!(
            rule.get("cached").is_none(),
            "no record yet, cached omitted"
        );
    }

    #[test]
    fn schema_is_valid_json_describing_the_output() {
        let schema: serde_json::Value = serde_json::from_str(SCHEMA).expect("schema is json");
        assert_eq!(
            schema.get("$schema").and_then(serde_json::Value::as_str),
            Some("https://json-schema.org/draft/2020-12/schema")
        );
        for key in [
            "manifest",
            "rules",
            "state",
            "inputs",
            "no-inputs",
            "ran_at",
        ] {
            assert!(SCHEMA.contains(key), "schema mentions `{key}`");
        }
    }
}
