//! Editing a rule's declaration and leaving its inputs alone: what a record
//! carries forward, and what it does not.
//!
//! A record stores an input digest, never the declaration in force when it was
//! written, which invites an obvious worry — a rule edited without touching a
//! file would inherit a record measured under the old declaration. It does not
//! hold. Every field that decides a verdict is read back off the CURRENT
//! manifest on every invocation: `outputs` are stat-ed live, `match` is applied
//! at match time, `tags` filter live, and only the digest and the exit status
//! come from the record. These tests pin that, so a refactor cannot quietly
//! make the worry true.

use std::fs;
use std::path::Path;

use predicates::prelude::{PredicateBooleanExt, predicate};

mod support;
use support::{mmz, write_manifest};

/// The rule before the edit: inputs and nothing else. Every case below appends
/// one field to it, so the edit under test is the only difference on disk.
const PLAIN: &str = "scopes:\n  src: [\"*.txt\"]\ncommands:\n  - name: sh\n    inputs: [src]\n";

/// Writes `manifest` and the single input file the `src` scope resolves to.
fn write_project(dir: &Path, manifest: &str) {
    write_manifest(dir, manifest);
    fs::write(dir.join("a.txt"), b"one").expect("write input");
}

/// Appends one indented rule field to [`PLAIN`] and rewrites the manifest with
/// it — the "declaration changed, inputs did not" edit itself.
fn redeclare(dir: &Path, field: &str) {
    write_manifest(dir, &format!("{PLAIN}    {field}\n"));
}

/// How many times the wrapped command has actually run, so a skip is measured
/// rather than inferred from an exit code.
fn run_len(dir: &Path) -> usize {
    fs::read(dir.join("runs.log")).map_or(0, |bytes| bytes.len())
}

/// The wrapped invocation every case memoizes: it logs a byte per real run.
fn wrap(dir: &Path) -> assert_cmd::assert::Assert {
    mmz(dir).args(["sh", "-c", "printf x >> runs.log"]).assert()
}

#[test]
fn an_output_added_after_the_fact_voids_a_record_that_never_promised_it() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_project(dir.path(), PLAIN);
    wrap(dir.path()).success();
    assert_eq!(run_len(dir.path()), 1, "the pass is recorded");

    // The edit: the same inputs, plus one promise the recorded run never made.
    redeclare(dir.path(), "outputs: [\"out/artifact.bin\"]");

    mmz(dir.path()).arg("--status").assert().success().stdout(
        predicate::str::contains("missing-output")
            .and(predicate::str::contains("out/artifact.bin")),
    );
    mmz(dir.path()).arg("--is-fresh").assert().code(1).stderr(
        predicate::str::contains("declared output `out/artifact.bin` is missing")
            .and(predicate::str::contains("inputs changed").not()),
    );
}

#[test]
fn an_output_added_after_the_fact_is_stat_ed_live_rather_than_read_off_the_record() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_project(dir.path(), PLAIN);
    wrap(dir.path()).success();

    // The other half of the same behaviour, and the honest limit of it: the
    // check is present-tense. An artifact already on disk satisfies the new
    // declaration, because a declared output is an existence check made at read
    // time — mmz never claims the artifact came from the run that was recorded,
    // and never hashes it (see the Concepts page's trusted/not-trusted table).
    fs::create_dir(dir.path().join("out")).expect("mkdir out");
    fs::write(dir.path().join("out/artifact.bin"), b"built").expect("write artifact");
    redeclare(dir.path(), "outputs: [\"out/artifact.bin\"]");

    mmz(dir.path())
        .arg("--status")
        .assert()
        .success()
        .stdout(predicate::str::contains("fresh").and(predicate::str::contains("stale").not()));
    mmz(dir.path()).arg("--is-fresh").assert().success();
}

#[test]
fn narrowing_match_to_exact_withdraws_the_rule_instead_of_lending_out_its_record() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_project(dir.path(), PLAIN);
    wrap(dir.path()).success();
    wrap(dir.path()).success();
    assert_eq!(run_len(dir.path()), 1, "prefix match skips the second run");

    redeclare(dir.path(), "match: exact");

    // `sh -c <script>` is no longer this rule's argv, so it stops reaching the
    // rule at all. A narrowed matcher withdraws the match rather than handing a
    // longer argv a record measured under the wider one, and the refusal is
    // loud: `strict`'s `no_match` case, exit 3, with nothing run behind it.
    wrap(dir.path())
        .code(3)
        .stderr(predicate::str::contains("no rule matches"));
    assert_eq!(run_len(dir.path()), 1, "the refusal ran nothing");

    // The record itself is untouched — it is keyed on the rule name, which the
    // edit did not change, and the bare identity still reads fresh.
    mmz(dir.path())
        .arg("--status")
        .assert()
        .success()
        .stdout(predicate::str::contains("fresh"));
}

#[test]
fn a_tag_added_after_the_fact_filters_the_rule_in_without_deciding_its_verdict() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_project(dir.path(), PLAIN);
    wrap(dir.path()).success();

    redeclare(dir.path(), "tags: [gate]");

    // The newly tagged rule enters the gate carrying the record it already had.
    // That is not a rule sneaking past unmeasured: the tag selects which rules
    // the gate consults, and the record it inherits is a pass of the same
    // command over the same inputs, which is the whole of what a record claims.
    mmz(dir.path())
        .args(["--is-fresh", "--tag", "gate"])
        .assert()
        .success();

    // And the tag confers nothing on its own — the digest still decides.
    fs::write(dir.path().join("a.txt"), b"two").expect("edit input");
    mmz(dir.path())
        .args(["--is-fresh", "--tag", "gate"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("inputs changed"));
}

#[test]
fn editing_on_hit_alone_leaves_the_record_standing() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_project(dir.path(), PLAIN);
    wrap(dir.path()).success();

    // `on_hit` is the one rule field that decides nothing, so it must not reach
    // the digest either: editing the notice re-runs no command.
    redeclare(dir.path(), "on_hit: \"skipped {cache:command}\"");

    wrap(dir.path())
        .success()
        .stderr(predicate::str::contains("skipped sh"));
    assert_eq!(run_len(dir.path()), 1, "a cosmetic field is not an input");
}
