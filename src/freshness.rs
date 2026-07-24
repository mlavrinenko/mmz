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
use crate::parametric;
use crate::status::{State, expansion_state};

/// One expansion's freshness, the unit a gate reports on. For a parametric
/// rule this is one per-file expansion; for a static rule, the rule itself.
pub struct Verdict {
    /// The expansion's cache identity: the bare rule name for a static rule,
    /// or the rule name with its `{scope}` macro substituted for a
    /// parametric expansion.
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

/// Evaluates freshness against the nearest manifest above `cwd`, running
/// nothing. A parametric rule (a `{scope}`-fanned `name`) expands exactly as
/// `mmz --status` does: one [`Verdict`] per file its scope resolves to,
/// keyed on that expansion's concrete identity rather than the rule's literal
/// template name.
///
/// With `argv` given and `tags` empty, resolves the single expansion `argv`
/// binds to (via [`parametric::resolve_matches`]) and returns its lone
/// verdict — for a parametric rule this gates the one per-file expansion
/// `argv` names, not the whole rule. With `tags` non-empty, expands every
/// rule that carries every listed tag (an AND filter — a rule with no tags
/// never matches) and returns one verdict per expansion; `argv` must be
/// `None` in that case, since a targeted command already resolves to a
/// single expansion. With both empty, expands every rule in manifest order
/// and returns one verdict per expansion, so a caller can gate the whole
/// manifest at once.
///
/// # Errors
///
/// Returns [`Error::TagWithCommand`] when `tags` is non-empty and `argv` is
/// also given, [`Error::NoManifest`] when no manifest is found, a manifest
/// error when one cannot be loaded, [`Error::NoMatch`] when `argv` matches no
/// rule, [`Error::CollidingIdentity`] when two expansions share a cache
/// identity, or a resolution/hashing error when a rule's inputs cannot be
/// read.
pub fn evaluate(cwd: &Path, argv: Option<&[String]>, tags: &[String]) -> Result<Vec<Verdict>> {
    let located = Manifest::locate(cwd)?;
    let manifest = &located.manifest;
    let base = located.root.as_path();
    let cache_dir = base.join(&manifest.cache_dir);

    if !tags.is_empty() {
        if argv.is_some() {
            return Err(Error::TagWithCommand);
        }
        let matches = expand_matching(manifest, base, |rule| {
            tags.iter().all(|tag| rule.tags.contains(tag))
        })?;
        return verdicts_for(manifest, &matches, base, &cache_dir);
    }

    if let Some(argv) = argv {
        let matches = parametric::resolve_matches(manifest, base, argv)?;
        parametric::detect_collision(&matches)?;
        let hit = matches.first().ok_or_else(|| Error::NoMatch {
            command: argv.join(" "),
        })?;
        return Ok(vec![verdict_for(manifest, hit, base, &cache_dir)?]);
    }

    let matches = expand_matching(manifest, base, |_| true)?;
    verdicts_for(manifest, &matches, base, &cache_dir)
}

/// Expands every rule passing `keep` into its parametric matches (one per
/// domain file, or the rule itself when static), then checks the collected
/// expansions for a colliding identity — the untargeted and tag-filtered
/// gates share this shape, differing only in which rules they keep.
fn expand_matching<'a>(
    manifest: &'a Manifest,
    base: &Path,
    keep: impl Fn(&Command) -> bool,
) -> Result<Vec<parametric::Match<'a>>> {
    let mut matches = Vec::new();
    for rule in manifest.commands.iter().filter(|rule| keep(rule)) {
        matches.extend(parametric::expand_rule(manifest, base, rule)?);
    }
    parametric::detect_collision(&matches)?;
    Ok(matches)
}

/// Builds one [`Verdict`] per expansion in `matches`.
fn verdicts_for(
    manifest: &Manifest,
    matches: &[parametric::Match],
    base: &Path,
    cache_dir: &Path,
) -> Result<Vec<Verdict>> {
    matches
        .iter()
        .map(|hit| verdict_for(manifest, hit, base, cache_dir))
        .collect()
}

/// Builds one expansion's [`Verdict`] — the step every branch of [`evaluate`]
/// needs once it has settled on which expansion(s) to report.
fn verdict_for(
    manifest: &Manifest,
    hit: &parametric::Match,
    base: &Path,
    cache_dir: &Path,
) -> Result<Verdict> {
    Ok(Verdict {
        rule: hit.exp.identity.clone(),
        state: expansion_state(manifest, hit, base, cache_dir)?,
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

    /// A parametric rule (`sh -c true sh {targets}`, fanned over `src/*.rs`)
    /// plus two candidate files, neither yet recorded.
    fn parametric_project(dir: &Path) {
        write_manifest(
            dir,
            "scopes:\n  targets: [\"src/*.rs\"]\ncommands:\n  - name: \"sh -c true sh {targets}\"\n",
        );
        std::fs::create_dir_all(dir.join("src")).expect("mkdir src");
        std::fs::write(dir.join("src/a.rs"), b"a").expect("write a");
        std::fs::write(dir.join("src/b.rs"), b"b").expect("write b");
    }

    /// Records a successful run of the parametric rule bound to `file`.
    fn record_file(dir: &Path, file: &str) {
        let argv = [
            "sh".to_owned(),
            "-c".to_owned(),
            "true".to_owned(),
            "sh".to_owned(),
            file.to_owned(),
        ];
        crate::run(&argv, dir).expect("recorded run");
    }

    #[test]
    fn untargeted_parametric_gate_reports_one_verdict_per_expansion() {
        let dir = tempfile::tempdir().expect("tempdir");
        let base = dir.path();
        parametric_project(base);
        record_file(base, "src/a.rs");

        let verdicts = evaluate(base, None, &[]).expect("evaluate");
        assert_eq!(verdicts.len(), 2, "one verdict per per-file expansion");
        assert!(
            verdicts
                .iter()
                .all(|verdict| !verdict.rule.contains("{targets}")),
            "no verdict is keyed on the literal template: {:?}",
            verdicts
                .iter()
                .map(|verdict| &verdict.rule)
                .collect::<Vec<_>>()
        );

        let a = verdicts
            .iter()
            .find(|verdict| verdict.rule == "sh -c true sh src/a.rs")
            .expect("a verdict for the recorded file");
        assert!(a.is_fresh(), "the recorded file is fresh");

        let b = verdicts
            .iter()
            .find(|verdict| verdict.rule == "sh -c true sh src/b.rs")
            .expect("a verdict for the unrecorded sibling");
        assert!(!b.is_fresh(), "the unrecorded sibling is not fresh");
        assert_eq!(b.state(), "never");
    }

    #[test]
    fn targeted_parametric_gate_matches_the_bound_expansion() {
        let dir = tempfile::tempdir().expect("tempdir");
        let base = dir.path();
        parametric_project(base);
        record_file(base, "src/a.rs");

        let argv_a = [
            "sh".to_owned(),
            "-c".to_owned(),
            "true".to_owned(),
            "sh".to_owned(),
            "src/a.rs".to_owned(),
        ];
        let fresh = evaluate(base, Some(&argv_a), &[]).expect("evaluate");
        assert_eq!(fresh.len(), 1, "one verdict for the resolved expansion");
        assert!(
            fresh.first().expect("one verdict").is_fresh(),
            "the recorded expansion is fresh"
        );

        let argv_b = [
            "sh".to_owned(),
            "-c".to_owned(),
            "true".to_owned(),
            "sh".to_owned(),
            "src/b.rs".to_owned(),
        ];
        let never = evaluate(base, Some(&argv_b), &[]).expect("evaluate");
        assert!(
            !never.first().expect("one verdict").is_fresh(),
            "the unrecorded expansion is not fresh"
        );
    }

    /// A parametric rule tagged `gate` (fanned over `src/*.rs`) plus an
    /// untagged static rule, so a tag filter must expand only the former.
    fn tagged_parametric_project(dir: &Path) {
        write_manifest(
            dir,
            "scopes:\n  targets: [\"src/*.rs\"]\ncommands:\n  - name: \"sh -c true sh {targets}\"\n    tags: [gate]\n  - name: cat\n",
        );
        std::fs::create_dir_all(dir.join("src")).expect("mkdir src");
        std::fs::write(dir.join("src/a.rs"), b"a").expect("write a");
        std::fs::write(dir.join("src/b.rs"), b"b").expect("write b");
    }

    #[test]
    fn tag_filter_expands_a_parametric_rule_per_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let base = dir.path();
        tagged_parametric_project(base);
        record_file(base, "src/a.rs");

        let gated = evaluate(base, None, &["gate".to_owned()]).expect("evaluate");
        assert_eq!(
            gated.len(),
            2,
            "one verdict per file expansion under the tag filter"
        );
        assert!(
            gated
                .iter()
                .all(|verdict| verdict.rule.starts_with("sh -c true sh src/")),
            "the untagged `cat` rule never contributes a verdict"
        );
        let a = gated
            .iter()
            .find(|verdict| verdict.rule == "sh -c true sh src/a.rs")
            .expect("a verdict");
        assert!(a.is_fresh(), "the recorded file is fresh under the filter");
    }
}
