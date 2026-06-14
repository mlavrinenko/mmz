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
    let manifest_path = Manifest::discover(cwd).ok_or_else(|| Error::NoManifest {
        start: cwd.to_path_buf(),
    })?;
    let manifest = Manifest::load(&manifest_path)?;
    let base = manifest_path
        .parent()
        .ok_or_else(|| Error::Internal("manifest path has no parent".to_owned()))?;

    let mut rules = Vec::with_capacity(manifest.commands.len());
    for rule in &manifest.commands {
        rules.push(rule_status(&manifest, rule, base)?);
    }
    Ok(Report {
        manifest: manifest_path.display().to_string(),
        rules,
    })
}

/// Computes one rule's status: resolve its inputs, hash them, and compare the
/// digest against the stored record.
fn rule_status(manifest: &Manifest, rule: &Rule, base: &Path) -> Result<RuleStatus> {
    let globs = manifest.globs_for(rule)?;
    let files = resolve::expand(&globs, base, manifest.gitignore)?;
    let cached = cache::read(base, &rule.name).map(|cached| CachedInfo {
        digest: cached.digest,
        ok: cached.ok,
    });
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
    let state = match &cached {
        None => State::Never,
        Some(record) if !record.ok => State::Failed,
        Some(record) if record.digest == digest => State::Fresh,
        Some(_) => State::Stale,
    };
    Ok(RuleStatus {
        name: rule.name.clone(),
        state,
        digest: Some(digest),
        cached,
        inputs,
    })
}

/// Renders the aligned `RULE / STATE` table.
fn render_text(report: &Report) -> String {
    let width = report
        .rules
        .iter()
        .map(|rule| rule.name.chars().count())
        .max()
        .unwrap_or(0)
        .max("RULE".len());

    let mut out = format!("{:<width$}  STATE\n", "RULE");
    for rule in &report.rules {
        out.push_str(&format!("{:<width$}  {}\n", rule.name, rule.state.label()));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{SCHEMA, report, report_json};

    fn write(dir: &std::path::Path, name: &str, body: &str) {
        std::fs::write(dir.join(name), body).expect("write");
    }

    #[test]
    fn reports_never_then_fresh_then_stale() {
        let dir = tempfile::tempdir().expect("tempdir");
        let base = dir.path();
        write(base, "a.txt", "one");
        write(
            base,
            "mmz.yaml",
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
    fn reports_no_inputs_for_empty_scopes() {
        let dir = tempfile::tempdir().expect("tempdir");
        let base = dir.path();
        write(
            base,
            "mmz.yaml",
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
        write(
            base,
            "mmz.yaml",
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
        for key in ["manifest", "rules", "state", "inputs", "no-inputs"] {
            assert!(SCHEMA.contains(key), "schema mentions `{key}`");
        }
    }
}
