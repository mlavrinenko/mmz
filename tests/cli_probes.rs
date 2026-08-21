//! Command-driven inputs (`probes:`), end to end: a rule can take an input
//! whose bytes come from a command's stdout, and every way that command can
//! fail is a hard stop rather than a digest mmz cannot trust.

use std::fs;
use std::path::Path;
use std::time::Duration;

use predicates::prelude::{PredicateBooleanExt, predicate};

mod support;
use support::{mmz, write_manifest};

/// A rule keyed on `a.txt` plus a probe that prints the contents of
/// `pinned.txt` — so the probe's output moves without any input file moving.
const PINNED: &str = concat!(
    "scopes:\n  src: [\"a.txt\"]\n",
    "probes:\n  tool:\n    run: cat pinned.txt\n",
    "commands:\n  - name: sh\n    inputs: [src, tool]\n",
);

/// Longest a probe may take before the test calls it hung rather than slow.
const PATIENCE: Duration = Duration::from_secs(30);

fn write_project(dir: &Path, manifest: &str) {
    write_manifest(dir, manifest);
    fs::write(dir.join("a.txt"), b"one").expect("write input");
}

fn run_len(dir: &Path, name: &str) -> usize {
    fs::read(dir.join(name)).map_or(0, |bytes| bytes.len())
}

/// Wraps a command that logs one byte per execution.
fn build(dir: &Path) -> assert_cmd::assert::Assert {
    mmz(dir)
        .timeout(PATIENCE)
        .args(["sh", "-c", "printf x >> runs.log"])
        .assert()
}

/// Every `*.yaml` record under the default cache directory.
fn records(dir: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(dir.join(".mmz/cache")) else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".yaml"))
        .collect()
}

#[test]
fn a_changed_probe_busts_the_rule_while_a_stable_one_does_not() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_project(dir.path(), PINNED);
    fs::write(dir.path().join("pinned.txt"), b"v1").expect("write probe source");

    build(dir.path()).success();
    assert_eq!(run_len(dir.path(), "runs.log"), 1, "first run executes");
    build(dir.path()).success();
    assert_eq!(
        run_len(dir.path(), "runs.log"),
        1,
        "the probe printed the same bytes, so the rule is still fresh"
    );

    // Nothing the scope names changes — only what the probe prints.
    fs::write(dir.path().join("pinned.txt"), b"v2").expect("rewrite probe source");
    build(dir.path()).success();
    assert_eq!(
        run_len(dir.path(), "runs.log"),
        2,
        "the probe's output feeds the input digest, so the rule re-runs"
    );
}

#[test]
fn a_stale_gate_names_the_probe_that_moved() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_project(dir.path(), PINNED);
    fs::write(dir.path().join("pinned.txt"), b"v1").expect("write probe source");
    build(dir.path()).success();

    mmz(dir.path())
        .timeout(PATIENCE)
        .arg("--is-fresh")
        .assert()
        .success();

    fs::write(dir.path().join("pinned.txt"), b"v2").expect("rewrite probe source");
    mmz(dir.path())
        .timeout(PATIENCE)
        .arg("--is-fresh")
        .assert()
        .code(1)
        .stderr(
            predicate::str::contains("probe `tool` changed since it last passed")
                .and(predicate::str::contains("inputs changed").not()),
        );
}

#[test]
fn a_failing_probe_exits_six_and_records_nothing() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_project(
        dir.path(),
        concat!(
            "scopes:\n  src: [\"a.txt\"]\n",
            "probes:\n  broken:\n    run: printf usable; printf 'bad shape' >&2; exit 3\n",
            "commands:\n  - name: sh\n    inputs: [src, broken]\n",
        ),
    );

    build(dir.path()).code(6).stderr(
        predicate::str::contains("probe `broken`")
            .and(predicate::str::contains("exit 3"))
            .and(predicate::str::contains("bad shape"))
            .and(predicate::str::contains("wrote no cache record")),
    );
    assert_eq!(
        run_len(dir.path(), "runs.log"),
        0,
        "the wrapped command never ran: a failed probe stops before the spawn"
    );
    assert!(
        records(dir.path()).is_empty(),
        "and nothing was recorded, so the rule cannot read fresh off a digest mmz could not trust"
    );
}

#[test]
fn an_empty_probe_errors_unless_it_opts_in() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_project(
        dir.path(),
        concat!(
            "scopes:\n  src: [\"a.txt\"]\n",
            "probes:\n  selector:\n    run: 'true'\n",
            "commands:\n  - name: sh\n    inputs: [src, selector]\n",
        ),
    );

    build(dir.path()).code(6).stderr(
        predicate::str::contains("probe `selector` produced no output")
            .and(predicate::str::contains("allow_empty: true")),
    );
    assert_eq!(run_len(dir.path(), "runs.log"), 0, "nothing ran");
    assert!(records(dir.path()).is_empty(), "nothing recorded");

    write_manifest(
        dir.path(),
        concat!(
            "scopes:\n  src: [\"a.txt\"]\n",
            "probes:\n  selector:\n    run: 'true'\n    allow_empty: true\n",
            "commands:\n  - name: sh\n    inputs: [src, selector]\n",
        ),
    );
    build(dir.path()).success();
    assert_eq!(
        run_len(dir.path(), "runs.log"),
        1,
        "opting in makes empty output a valid input"
    );
}

#[test]
fn a_probe_shared_by_two_rules_runs_once() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_project(
        dir.path(),
        concat!(
            "scopes:\n  src: [\"a.txt\"]\n",
            "probes:\n  shared:\n    run: printf x >> probe.log; printf stable\n",
            "commands:\n  - name: alpha\n    inputs: [src, shared]\n",
            "  - name: beta\n    inputs: [src, shared]\n",
            "  - name: gamma\n    inputs: [src, shared]\n",
        ),
    );

    // A bare `--is-fresh` gates every rule — the shape a pre-commit hook runs.
    mmz(dir.path())
        .timeout(PATIENCE)
        .arg("--is-fresh")
        .assert()
        .code(1);
    assert_eq!(
        run_len(dir.path(), "probe.log"),
        1,
        "three rules named one probe and it was executed once, not once per rule"
    );

    mmz(dir.path())
        .timeout(PATIENCE)
        .arg("--status")
        .assert()
        .success();
    assert_eq!(
        run_len(dir.path(), "probe.log"),
        2,
        "a second invocation resolves it again — the memo is per-invocation, not a stale process cache"
    );
}

#[test]
fn a_probe_named_like_a_scope_is_rejected_at_parse() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_project(
        dir.path(),
        concat!(
            "scopes:\n  rust: [\"a.txt\"]\n",
            "probes:\n  rust:\n    run: printf x\n",
            "commands:\n  - name: sh\n    inputs: [rust]\n",
        ),
    );

    build(dir.path()).code(4).stderr(
        predicate::str::contains("`rust` is declared as both a scope and a probe")
            .and(predicate::str::contains("one namespace")),
    );
    assert_eq!(
        run_len(dir.path(), "runs.log"),
        0,
        "an ambiguous manifest never reaches a run"
    );
}

#[test]
fn an_inputs_entry_that_is_neither_names_both_places() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_project(
        dir.path(),
        "scopes:\n  src: [\"a.txt\"]\ncommands:\n  - name: sh\n    inputs: [src, ghost]\n",
    );

    build(dir.path()).code(4).stderr(
        predicate::str::contains("unknown input `ghost`")
            .and(predicate::str::contains("`scopes:` or `probes:`")),
    );
}

#[test]
fn a_probe_reading_stdin_fails_instead_of_hanging() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_project(
        dir.path(),
        concat!(
            "scopes:\n  src: [\"a.txt\"]\n",
            "probes:\n  reader:\n    run: cat\n",
            "commands:\n  - name: sh\n    inputs: [src, reader]\n",
        ),
    );

    // stdin is closed for a probe, so `cat` sees EOF at once and prints
    // nothing. Without that it would block forever and wedge every gate that
    // resolves this rule; the timeout here is what catches a regression.
    build(dir.path()).code(6).stderr(predicate::str::contains(
        "probe `reader` produced no output",
    ));
}

#[test]
fn status_json_exposes_each_probe_digest() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_project(dir.path(), PINNED);
    fs::write(dir.path().join("pinned.txt"), b"v1").expect("write probe source");

    mmz(dir.path())
        .timeout(PATIENCE)
        .arg("--status=json")
        .assert()
        .success()
        .stdout(
            predicate::str::contains("\"probes\": {").and(predicate::str::contains("\"tool\"")),
        );
}

#[test]
fn a_rule_whose_only_input_is_a_probe_is_memoized() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_project(
        dir.path(),
        concat!(
            "probes:\n  tool:\n    run: cat pinned.txt\n",
            "commands:\n  - name: sh\n    inputs: [tool]\n",
        ),
    );
    fs::write(dir.path().join("pinned.txt"), b"v1").expect("write probe source");

    build(dir.path()).success();
    build(dir.path()).success();
    assert_eq!(
        run_len(dir.path(), "runs.log"),
        1,
        "a probe is an input, so a probe-only rule is memoized rather than `no-inputs`"
    );

    fs::write(dir.path().join("pinned.txt"), b"v2").expect("rewrite probe source");
    build(dir.path()).success();
    assert_eq!(run_len(dir.path(), "runs.log"), 2, "and it busts on change");
}

#[test]
fn probe_shell_pins_the_argv_a_run_line_is_handed_to() {
    let dir = tempfile::tempdir().expect("tempdir");
    // `env -i` clears the environment, so a probe reading $MARKER sees the
    // value this shell sets and not the caller's — proof the wrapper really
    // interposed rather than the line running under a plain `sh -c`.
    write_project(
        dir.path(),
        concat!(
            "probe_shell: [\"env\", \"MARKER=pinned\", \"sh\", \"-c\"]\n",
            "probes:\n  tool:\n    run: printf '%s' \"$MARKER\"\n",
            "commands:\n  - name: sh\n    inputs: [tool]\n",
        ),
    );

    mmz(dir.path())
        .timeout(PATIENCE)
        .arg("--dump-config")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "probe_shell: [\"env\", \"MARKER=pinned\", \"sh\", \"-c\"]",
        ));

    // The probe resolves at all, which it could not if `env` had swallowed the
    // run line or the wrapper had been dropped.
    build(dir.path()).success();
    assert_eq!(run_len(dir.path(), "runs.log"), 1);
    build(dir.path()).success();
    assert_eq!(
        run_len(dir.path(), "runs.log"),
        1,
        "the pinned shell is stable, so the digest is too"
    );
}

#[test]
fn a_probe_shell_that_names_no_program_is_refused() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_project(
        dir.path(),
        concat!(
            "probe_shell: []\n",
            "probes:\n  tool:\n    run: echo hi\n",
            "commands:\n  - name: sh\n    inputs: [tool]\n",
        ),
    );

    // Exit 4, the manifest-error code: an empty list is caught at load, before
    // any probe is spawned, rather than panicking in the spawn path.
    build(dir.path())
        .code(4)
        .stderr(predicate::str::contains("`probe_shell` is empty"));
}

#[test]
fn probe_shell_is_rejected_in_an_imported_fragment() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_project(
        dir.path(),
        concat!(
            "imports: [fragment.yaml]\n",
            "commands:\n  - name: sh\n    inputs: [src]\n",
        ),
    );
    fs::write(
        dir.path().join(".mmz/fragment.yaml"),
        "probe_shell: [\"sh\", \"-c\"]\nscopes:\n  src: [\"a.txt\"]\n",
    )
    .expect("write fragment");

    // Root-only, like the other four policy keys: a fragment setting it would
    // leave which one governs a probe declared elsewhere undecidable.
    build(dir.path())
        .code(4)
        .stderr(predicate::str::contains("probe_shell"));
}
