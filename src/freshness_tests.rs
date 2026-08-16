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
    assert_eq!(verdict.reason().as_deref(), Some("never run"));

    record_ok(base);
    let fresh = evaluate(base, Some(&argv), &[]).expect("evaluate");
    let verdict = fresh.first().expect("one verdict");
    assert!(verdict.is_fresh(), "fresh after a recorded run");
    assert_eq!(verdict.reason(), None, "fresh carries no reason");
    assert!(
        verdict.is_fresh(),
        "a rule declaring no outputs is unaffected by the outputs check"
    );

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

/// A rule that produces `out/artifact.bin` and declares it, plus the one input
/// keying it. The command writes the artifact, so a wrapped run is a real
/// producer run.
fn producing_project(dir: &Path) {
    write_manifest(
        dir,
        concat!(
            "scopes:\n  src: [\"*.txt\"]\n",
            "commands:\n  - name: sh\n    inputs: [src]\n",
            "    outputs:\n      - out/artifact.bin\n",
        ),
    );
    std::fs::write(dir.join("a.txt"), b"one").expect("write input");
    std::fs::create_dir_all(dir.join("out")).expect("mkdir out");
}

fn record_producing_run(dir: &Path) {
    let argv = [
        "sh".to_owned(),
        "-c".to_owned(),
        "printf built > out/artifact.bin".to_owned(),
    ];
    crate::run(&argv, dir).expect("recorded run");
}

#[test]
fn a_present_output_stays_fresh_and_a_deleted_one_names_the_path() {
    let dir = tempfile::tempdir().expect("tempdir");
    let base = dir.path();
    producing_project(base);
    record_producing_run(base);

    let fresh = evaluate(base, None, &[]).expect("evaluate");
    let verdict = fresh.first().expect("one verdict");
    assert!(
        verdict.is_fresh(),
        "inputs unchanged and the artifact is on disk"
    );

    // The artifact is deleted; not one input byte moves.
    std::fs::remove_file(base.join("out/artifact.bin")).expect("delete artifact");
    let voided = evaluate(base, None, &[]).expect("evaluate");
    let verdict = voided.first().expect("one verdict");
    assert!(!verdict.is_fresh(), "a missing output voids the record");
    assert_eq!(verdict.state(), "missing-output");
    assert_eq!(
        verdict.reason().as_deref(),
        Some("declared output `out/artifact.bin` is missing"),
        "the reason names the artifact, not the inputs"
    );
    assert!(
        verdict.is_remediable(),
        "re-running the command under mmz regenerates it"
    );
}

#[test]
fn a_missing_output_outranks_a_changed_input_in_the_reason() {
    let dir = tempfile::tempdir().expect("tempdir");
    let base = dir.path();
    producing_project(base);
    record_producing_run(base);

    std::fs::write(base.join("a.txt"), b"two").expect("rewrite input");
    std::fs::remove_file(base.join("out/artifact.bin")).expect("delete artifact");

    let verdicts = evaluate(base, None, &[]).expect("evaluate");
    let verdict = verdicts.first().expect("one verdict");
    assert_eq!(
        verdict.state(),
        "missing-output",
        "with both true, the verdict names the fact a reader cannot guess"
    );
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

#[test]
fn a_changed_probe_busts_the_rule_and_the_reason_names_it() {
    let dir = tempfile::tempdir().expect("tempdir");
    let base = dir.path();
    std::fs::write(base.join("a.txt"), b"one").expect("write input");
    std::fs::write(base.join("pinned.txt"), b"v1").expect("write probe source");
    write_manifest(
        base,
        concat!(
            "scopes:\n  src: [\"a.txt\"]\n",
            "probes:\n  tool:\n    run: cat pinned.txt\n",
            "commands:\n  - name: sh\n    inputs: [src, tool]\n",
        ),
    );
    record_ok(base);
    let argv = ["sh".to_owned()];
    assert!(
        evaluate(base, Some(&argv), &[])
            .expect("evaluate")
            .first()
            .is_some_and(super::Verdict::is_fresh),
        "a stable probe leaves the rule fresh"
    );

    // Only the probe's output moves; every input file stays byte-identical.
    std::fs::write(base.join("pinned.txt"), b"v2").expect("rewrite probe source");
    let stale = evaluate(base, Some(&argv), &[]).expect("evaluate");
    let verdict = stale.first().expect("one verdict");
    assert!(!verdict.is_fresh(), "the probe's output feeds the digest");
    assert_eq!(verdict.state(), "stale");
    assert_eq!(
        verdict.reason().as_deref(),
        Some("probe `tool` changed since it last passed"),
        "naming the probe keeps a reader from diffing files that never moved"
    );
}

#[test]
fn a_changed_file_does_not_blame_an_unchanged_probe() {
    let dir = tempfile::tempdir().expect("tempdir");
    let base = dir.path();
    std::fs::write(base.join("a.txt"), b"one").expect("write input");
    write_manifest(
        base,
        concat!(
            "scopes:\n  src: [\"a.txt\"]\n",
            "probes:\n  tool:\n    run: printf stable\n",
            "commands:\n  - name: sh\n    inputs: [src, tool]\n",
        ),
    );
    record_ok(base);

    std::fs::write(base.join("a.txt"), b"two").expect("rewrite input");
    let stale = evaluate(base, Some(&["sh".to_owned()]), &[]).expect("evaluate");
    let verdict = stale.first().expect("one verdict");
    assert_eq!(verdict.state(), "stale");
    assert_eq!(
        verdict.reason().as_deref(),
        Some("inputs changed since it last passed"),
        "an unchanged probe is never blamed for a file edit"
    );
}

#[test]
fn a_failing_probe_stops_the_gate_rather_than_reporting_stale() {
    let dir = tempfile::tempdir().expect("tempdir");
    let base = dir.path();
    std::fs::write(base.join("a.txt"), b"one").expect("write input");
    write_manifest(
        base,
        concat!(
            "scopes:\n  src: [\"a.txt\"]\n",
            "probes:\n  broken:\n    run: exit 7\n",
            "commands:\n  - name: sh\n    inputs: [src, broken]\n",
        ),
    );
    let Err(err) = evaluate(base, None, &[]) else {
        panic!("a broken probe is a hard error, not a verdict");
    };
    assert!(
        matches!(err, Error::ProbeFailed { .. }),
        "fail closed: the gate cannot honestly answer without the probe, got {err}"
    );
}
