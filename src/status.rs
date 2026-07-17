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
use crate::manifest::Manifest;
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
enum State {
    Fresh,
    Stale,
    Never,
    Failed,
    NoInputs,
}

impl State {
    /// The label used in the human table; matches the JSON enum spelling.
    const fn label(self) -> &'static str {
        match self {
            Self::Fresh => "fresh",
            Self::Stale => "stale",
            Self::Never => "never",
            Self::Failed => "failed",
            Self::NoInputs => "no-inputs",
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
    let mut matches = Vec::with_capacity(manifest.commands.len());
    for rule in &manifest.commands {
        matches.extend(parametric::expand_rule(manifest, base, rule)?);
    }
    parametric::detect_collision(&matches)?;
    let mut rules = Vec::with_capacity(matches.len());
    for hit in &matches {
        rules.push(rule_status(manifest, hit, base, &cache_dir)?);
    }
    Ok(Report {
        manifest: located.path.display().to_string(),
        rules,
    })
}

/// Computes one expansion's status: resolve its inputs (shared pins plus any
/// bound file), hash them, and compare the digest against the stored record.
fn rule_status(
    manifest: &Manifest,
    hit: &parametric::Match,
    base: &Path,
    cache_dir: &Path,
) -> Result<RuleStatus> {
    let identity = hit.exp.identity.clone();
    let globs = manifest.globs_for(hit.rule)?;
    let mut files = resolve::expand(&globs, base, manifest.gitignore)?;
    if let Some(file) = &hit.exp.file {
        files.push(file.clone());
        files.sort();
        files.dedup();
    }
    let cached = cache::read(cache_dir, &identity).map(|cached| CachedInfo {
        digest: cached.digest,
        ok: cached.ok,
        ran_at: cached.ran_at,
    });
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
    let state = match &cached {
        None => State::Never,
        Some(record) if !record.ok => State::Failed,
        Some(record) if record.digest == digest => State::Fresh,
        Some(_) => State::Stale,
    };
    Ok(RuleStatus {
        name: identity,
        state,
        digest: Some(digest),
        cached,
        inputs,
    })
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
    fn parametric_rule_enumerates_one_row_per_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let base = dir.path();
        std::fs::create_dir(base.join("src")).expect("mkdir");
        write(&base.join("src"), "a.rs", "a");
        write(&base.join("src"), "b.rs", "b");
        manifest(
            base,
            "scopes:\n  targets: [\"src/**/*.rs\"]\ncommands:\n  - name: \"lint {targets}\"\n",
        );
        let report = report(base).expect("report");
        assert!(report.contains("lint src/a.rs"), "row for a: {report}");
        assert!(report.contains("lint src/b.rs"), "row for b: {report}");
        assert!(report.contains("never"), "each expansion has a verdict");
    }

    #[test]
    fn colliding_expansions_are_an_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        let base = dir.path();
        write(base, "a.rs", "x");
        manifest(
            base,
            "scopes:\n  wide: [\"*.rs\"]\n  narrow: [\"a.rs\"]\ncommands:\n  - name: \"do {wide}\"\n  - name: \"do {narrow}\"\n",
        );
        assert!(
            report(base).is_err(),
            "status surfaces a colliding-identity config proactively"
        );
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
