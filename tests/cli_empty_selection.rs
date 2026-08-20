//! A gate over no rule at all: `mmz --is-fresh` must refuse it (exit 7) rather
//! than exit 0 on the strength of having checked nothing.
//!
//! The reach-for case is a typo'd or renamed tag. `mmz --is-fresh --tag gats`
//! used to be indistinguishable from a passing build, which is the false green
//! the tool exists to refuse — reached through the door rather than the wall.
//! `--status` keeps exiting 0 on the same selection: it asserts nothing, so an
//! empty report is honest as long as the line explaining it is true.

use std::fs;
use std::path::Path;

use predicates::prelude::{PredicateBooleanExt, predicate};

mod support;
use support::{mmz, write_manifest};

/// Two rules on their own scopes, tagged `gate` and `bench` respectively, so a
/// filter can select one, the other, both, or (with both tags at once) neither.
fn write_tagged_project(dir: &Path) {
    write_manifest(
        dir,
        "scopes:\n  a: [\"a.txt\"]\n  b: [\"b.txt\"]\ncommands:\n  - name: sh\n    inputs: [a]\n    tags: [gate]\n  - name: env\n    inputs: [b]\n    tags: [bench]\n",
    );
    fs::write(dir.join("a.txt"), b"one").expect("write a.txt");
    fs::write(dir.join("b.txt"), b"one").expect("write b.txt");
}

#[test]
fn a_tag_no_rule_carries_is_refused_rather_than_gating_nothing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let base = dir.path();
    write_tagged_project(base);

    // A recorded pass, so the only difference between the two runs below is one
    // letter of the tag.
    mmz(base).args(["sh", "-c", "exit 0"]).assert().success();
    mmz(base)
        .args(["--is-fresh", "--tag", "gate"])
        .assert()
        .success();

    mmz(base)
        .args(["--is-fresh", "--tag", "gats"])
        .assert()
        .code(7)
        .stderr(
            predicate::str::contains("tag `gats`")
                .and(predicate::str::contains("declares `bench`, `gate`")),
        );
}

#[test]
fn tags_that_exist_separately_still_select_nothing_together() {
    let dir = tempfile::tempdir().expect("tempdir");
    let base = dir.path();
    write_tagged_project(base);
    mmz(base).args(["sh", "-c", "exit 0"]).assert().success();
    mmz(base).args(["env", "true"]).assert().success();

    // Both tags exist and both rules are fresh; no rule carries BOTH, and the
    // filter ANDs, so the selection is empty and the gate asserts nothing.
    mmz(base)
        .args(["--is-fresh", "--tag", "gate", "--tag", "bench"])
        .assert()
        .code(7)
        .stderr(predicate::str::contains("every tag of `gate`, `bench`"));
}

#[test]
fn a_manifest_declaring_no_rules_refuses_an_untagged_gate() {
    let dir = tempfile::tempdir().expect("tempdir");
    let base = dir.path();
    write_manifest(base, "scopes:\n  a: [\"a.txt\"]\n");

    mmz(base)
        .arg("--is-fresh")
        .assert()
        .code(7)
        .stderr(predicate::str::contains("declares no commands"));
}

#[test]
fn a_parametric_rule_whose_scope_resolves_to_nothing_is_not_a_pass() {
    let dir = tempfile::tempdir().expect("tempdir");
    let base = dir.path();
    // The rule is selected by the filter, and then fans over a scope with no
    // files in it: an empty gate reached through the fan rather than the tag.
    write_manifest(
        base,
        "gitignore: false\nscopes:\n  targets: [\"src/**/*.rs\"]\ncommands:\n  - name: \"check {targets}\"\n    inputs: [targets]\n    tags: [gate]\n",
    );

    mmz(base)
        .args(["--is-fresh", "--tag", "gate"])
        .assert()
        .code(7)
        .stderr(predicate::str::contains("resolved to no files"));
}

#[test]
fn status_reports_an_emptied_selection_instead_of_claiming_no_rules_exist() {
    let dir = tempfile::tempdir().expect("tempdir");
    let base = dir.path();
    write_tagged_project(base);

    // Rules ARE defined; none carries that tag. The old line said the opposite,
    // and a report is allowed to be empty but not to be wrong.
    mmz(base)
        .args(["--status", "--tag", "gats"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("no rules defined")
                .not()
                .and(predicate::str::contains("tag `gats`"))
                .and(predicate::str::contains("declares `bench`, `gate`")),
        );

    // The manifest that line was written for keeps it, verbatim.
    let empty = tempfile::tempdir().expect("tempdir");
    write_manifest(empty.path(), "scopes:\n  a: [\"a.txt\"]\n");
    mmz(empty.path())
        .arg("--status")
        .assert()
        .success()
        .stdout(predicate::str::contains("no rules defined"));
}

#[test]
fn status_json_answers_an_empty_selection_with_an_empty_rule_list() {
    let dir = tempfile::tempdir().expect("tempdir");
    let base = dir.path();
    write_tagged_project(base);

    let output = mmz(base)
        .args(["--status=json", "--tag", "gats"])
        .output()
        .expect("ran mmz");
    assert!(output.status.success(), "a report asserts nothing");
    let parsed: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("valid json report");
    assert_eq!(
        parsed
            .get("rules")
            .and_then(|rules| rules.as_array())
            .map(Vec::len),
        Some(0),
        "an empty array is already the whole answer for a consumer"
    );
}
