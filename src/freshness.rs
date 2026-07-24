//! `mmz --is-fresh`: assert a rule's cache is fresh without running it.
//!
//! The memoization engine ([`crate::engine`]) is run-or-skip: a stale rule runs.
//! A gate wants the opposite — to confirm a command was already memoized fresh
//! and otherwise fail, never launching the (often slow) command itself. That is
//! this module: resolve a rule's inputs, compare their digest to the record, and
//! report the verdict. It executes nothing.
//!
//! The reach-for case is a git hook. A pre-push that must not boot a VM, yet must
//! refuse a push whose checks were never run, calls `mmz --is-fresh -- just
//! check` and trusts the exit code. With no command it gates every rule at once.

use std::path::Path;

use crate::error::{Error, Result};
use crate::manifest::{Command, Manifest};
use crate::matcher;
use crate::status::{State, rule_state};

/// One rule's freshness, the unit a gate reports on.
pub struct Verdict {
    /// The rule's name (its cache identity).
    pub rule: String,
    state: State,
}

impl Verdict {
    /// True when the rule is fresh — its inputs are unchanged since it last
    /// succeeded, so a gate over it passes.
    #[must_use]
    pub const fn is_fresh(&self) -> bool {
        self.state.is_fresh()
    }

    /// The rule's freshness label (`fresh`, `stale`, `never`, `failed`,
    /// `no-inputs`), matching `mmz --status`.
    #[must_use]
    pub const fn state(&self) -> &'static str {
        self.state.label()
    }

    /// Why the rule is not fresh, for a gate's message; `None` when it is fresh.
    #[must_use]
    pub const fn reason(&self) -> Option<&'static str> {
        self.state.reason()
    }
}

/// Evaluates freshness against the nearest manifest above `cwd`, running nothing.
///
/// With `argv` given and `tags` empty, returns the single [`Verdict`] for the
/// rule `argv` matches. With `tags` non-empty, returns one verdict per rule
/// that carries every listed tag (an AND filter — a rule with no tags never
/// matches); `argv` must be `None` in that case, since a targeted command
/// already resolves to a single rule. With both empty, returns one verdict per
/// rule in manifest order, so a caller can gate the whole manifest at once.
///
/// # Errors
///
/// Returns [`Error::TagWithCommand`] when `tags` is non-empty and `argv` is
/// also given, [`Error::NoManifest`] when no manifest is found, a manifest
/// error when one cannot be loaded, [`Error::NoMatch`] when `argv` matches no
/// rule, or a resolution/hashing error when a rule's inputs cannot be read.
pub fn evaluate(cwd: &Path, argv: Option<&[String]>, tags: &[String]) -> Result<Vec<Verdict>> {
    let located = Manifest::locate(cwd)?;
    let manifest = &located.manifest;
    let base = located.root.as_path();
    let cache_dir = base.join(&manifest.cache_dir);

    if !tags.is_empty() {
        if argv.is_some() {
            return Err(Error::TagWithCommand);
        }
        return manifest
            .commands
            .iter()
            .filter(|rule| tags.iter().all(|tag| rule.tags.contains(tag)))
            .map(|rule| verdict_for(manifest, rule, base, &cache_dir))
            .collect();
    }

    match argv {
        Some(argv) => {
            let rule =
                matcher::first_match(&manifest.commands, argv).ok_or_else(|| Error::NoMatch {
                    command: argv.join(" "),
                })?;
            Ok(vec![verdict_for(manifest, rule, base, &cache_dir)?])
        }
        None => manifest
            .commands
            .iter()
            .map(|rule| verdict_for(manifest, rule, base, &cache_dir))
            .collect(),
    }
}

/// Builds one rule's [`Verdict`] — the step every branch of [`evaluate`] needs
/// once it has settled on which rules to report.
fn verdict_for(
    manifest: &Manifest,
    rule: &Command,
    base: &Path,
    cache_dir: &Path,
) -> Result<Verdict> {
    Ok(Verdict {
        rule: rule.name.clone(),
        state: rule_state(manifest, rule, base, cache_dir)?,
    })
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::evaluate;
    use crate::Error;

    fn write_manifest(dir: &Path, body: &str) {
        let cfg = dir.join(".mmz");
        std::fs::create_dir_all(&cfg).expect("mkdir .mmz");
        std::fs::write(cfg.join("config.yaml"), body).expect("write manifest");
    }

    /// A one-rule manifest (`sh`, keyed on `*.txt`) plus one input file.
    fn project(dir: &Path) {
        write_manifest(
            dir,
            "scopes:\n  src: [\"*.txt\"]\ncommands:\n  - name: sh\n    inputs: [src]\n",
        );
        std::fs::write(dir.join("a.txt"), b"one").expect("write input");
    }

    fn record_ok(dir: &Path) {
        let argv = ["sh".to_owned(), "-c".to_owned(), "exit 0".to_owned()];
        crate::run(&argv, dir).expect("recorded run");
    }

    #[test]
    fn targeted_tracks_never_then_fresh_then_stale() {
        let dir = tempfile::tempdir().expect("tempdir");
        let base = dir.path();
        project(base);
        let argv = ["sh".to_owned()];

        let never = evaluate(base, Some(&argv), &[]).expect("evaluate");
        assert_eq!(never.len(), 1);
        let verdict = never.first().expect("one verdict");
        assert!(!verdict.is_fresh(), "never run is not fresh");
        assert_eq!(verdict.state(), "never");
        assert_eq!(verdict.reason(), Some("never run"));

        record_ok(base);
        let fresh = evaluate(base, Some(&argv), &[]).expect("evaluate");
        let verdict = fresh.first().expect("one verdict");
        assert!(verdict.is_fresh(), "fresh after a recorded run");
        assert_eq!(verdict.reason(), None, "fresh carries no reason");

        std::fs::write(base.join("a.txt"), b"two").expect("rewrite input");
        let stale = evaluate(base, Some(&argv), &[]).expect("evaluate");
        let verdict = stale.first().expect("one verdict");
        assert!(!verdict.is_fresh(), "changed input is stale");
        assert_eq!(verdict.state(), "stale");
    }

    #[test]
    fn untargeted_gates_every_rule() {
        let dir = tempfile::tempdir().expect("tempdir");
        let base = dir.path();
        project(base);
        record_ok(base);

        let all = evaluate(base, None, &[]).expect("evaluate");
        assert_eq!(all.len(), 1, "one verdict per rule");
        let verdict = all.first().expect("one verdict");
        assert_eq!(verdict.rule, "sh");
        assert!(verdict.is_fresh());
    }

    #[test]
    fn unmatched_command_is_a_no_match_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        let base = dir.path();
        project(base);
        let argv = ["cargo".to_owned(), "test".to_owned()];
        assert!(
            matches!(evaluate(base, Some(&argv), &[]), Err(Error::NoMatch { .. })),
            "a command no rule matches errors, regardless of strict"
        );
    }

    #[test]
    fn no_inputs_rule_is_not_fresh() {
        let dir = tempfile::tempdir().expect("tempdir");
        let base = dir.path();
        write_manifest(
            base,
            "scopes:\n  none: [\"*.none\"]\ncommands:\n  - name: sh\n    inputs: [none]\n",
        );
        let verdicts = evaluate(base, None, &[]).expect("evaluate");
        let verdict = verdicts.first().expect("one verdict");
        assert!(!verdict.is_fresh(), "a rule with no inputs cannot be fresh");
        assert_eq!(verdict.state(), "no-inputs");
    }

    #[test]
    fn missing_manifest_errors() {
        let dir = tempfile::tempdir().expect("tempdir");
        assert!(
            evaluate(dir.path(), None, &[]).is_err(),
            "no manifest is fatal"
        );
    }

    /// A manifest with a tagged rule (`gate`), an untagged rule, and a rule
    /// tagged with two labels — enough to exercise single- and multi-tag AND
    /// filtering.
    fn tagged_project(dir: &Path) {
        write_manifest(
            dir,
            "scopes:\n  src: [\"*.txt\"]\ncommands:\n  - name: sh\n    inputs: [src]\n    tags: [gate]\n  - name: cat\n    inputs: [src]\n  - name: env\n    inputs: [src]\n    tags: [gate, slow]\n",
        );
        std::fs::write(dir.join("a.txt"), b"one").expect("write input");
    }

    #[test]
    fn tag_filter_narrows_to_matching_rules_only() {
        let dir = tempfile::tempdir().expect("tempdir");
        let base = dir.path();
        tagged_project(base);

        let gated = evaluate(base, None, &["gate".to_owned()]).expect("evaluate");
        let mut names: Vec<&str> = gated.iter().map(|verdict| verdict.rule.as_str()).collect();
        names.sort_unstable();
        assert_eq!(names, ["env", "sh"], "only rules tagged `gate` come back");
    }

    #[test]
    fn untagged_rules_are_excluded_under_a_tag_filter() {
        let dir = tempfile::tempdir().expect("tempdir");
        let base = dir.path();
        tagged_project(base);

        let gated = evaluate(base, None, &["gate".to_owned()]).expect("evaluate");
        assert!(
            gated.iter().all(|verdict| verdict.rule != "cat"),
            "the untagged rule never matches a --tag filter"
        );
    }

    #[test]
    fn two_tags_are_anded_not_ored() {
        let dir = tempfile::tempdir().expect("tempdir");
        let base = dir.path();
        tagged_project(base);

        let both = evaluate(base, None, &["gate".to_owned(), "slow".to_owned()]).expect("evaluate");
        assert_eq!(both.len(), 1, "only the rule carrying both tags matches");
        assert_eq!(both.first().expect("one verdict").rule, "env");
    }

    #[test]
    fn tag_and_command_together_is_an_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        let base = dir.path();
        tagged_project(base);
        let argv = ["sh".to_owned()];

        assert!(
            matches!(
                evaluate(base, Some(&argv), &["gate".to_owned()]),
                Err(Error::TagWithCommand)
            ),
            "a tag filter plus a targeted command is redundant, so it errors"
        );
    }
}
