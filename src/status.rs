//! `mmz --status`: report each rule's current freshness against its cache.
//!
//! For every rule the manifest declares, this resolves the rule's inputs,
//! recomputes their digest, and compares it to the stored record — answering
//! "would this rule skip or run right now, and why?" without running anything.

use std::path::Path;

use crate::error::{Error, Result};
use crate::manifest::{Command as Rule, Manifest};
use crate::{cache, hashing, resolve};

/// Builds the human-readable status report for the manifest governing `cwd`.
///
/// # Errors
///
/// Returns [`Error::NoManifest`] when none is found, a manifest error when one
/// cannot be loaded, or a resolution error when a rule's globs are invalid.
pub fn report(cwd: &Path) -> Result<String> {
    let manifest_path = Manifest::discover(cwd).ok_or_else(|| Error::NoManifest {
        start: cwd.to_path_buf(),
    })?;
    let manifest = Manifest::load(&manifest_path)?;
    let base = manifest_path
        .parent()
        .ok_or_else(|| Error::Internal("manifest path has no parent".to_owned()))?;

    if manifest.commands.is_empty() {
        return Ok(format!("no rules defined in {}\n", manifest_path.display()));
    }

    let width = manifest
        .commands
        .iter()
        .map(|rule| rule.name.chars().count())
        .max()
        .unwrap_or(0)
        .max("RULE".len());

    let mut out = format!("{:<width$}  STATE\n", "RULE");
    for rule in &manifest.commands {
        let state = rule_state(&manifest, rule, base)?;
        out.push_str(&format!("{:<width$}  {state}\n", rule.name));
    }
    Ok(out)
}

/// One rule's freshness label: `fresh`, `stale`, `never`, `failed`, or
/// `no-inputs`.
fn rule_state(manifest: &Manifest, rule: &Rule, base: &Path) -> Result<&'static str> {
    let globs = manifest.globs_for(rule)?;
    let files = resolve::expand(&globs, base, manifest.gitignore)?;
    if files.is_empty() {
        return Ok("no-inputs");
    }
    let digest = hashing::digest_files(base, &files)?;
    Ok(match cache::read(base, &rule.name) {
        None => "never",
        Some(cached) if !cached.ok => "failed",
        Some(cached) if cached.digest == digest => "fresh",
        Some(_) => "stale",
    })
}

#[cfg(test)]
mod tests {
    use super::report;

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
}
