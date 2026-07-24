//! Parametric (`{scope}`-fanned) rule behaviour across the CLI surface: how
//! `--status` and `--is-fresh` each expand a fanned rule into its per-file
//! records, end to end through the built binary.

use std::fs;

use predicates::prelude::{PredicateBooleanExt, predicate};

mod support;
use support::{mmz, write_manifest};

#[test]
fn parametric_rule_fans_over_a_scope_end_to_end() {
    let dir = tempfile::tempdir().expect("tempdir");
    let base = dir.path();
    fs::create_dir(base.join("src")).expect("mkdir src");
    fs::write(base.join("src/a.rs"), b"a").expect("a");
    fs::write(base.join("src/b.rs"), b"b").expect("b");
    // One declaration, fanned over src/**/*.rs; the command touches <file>.done.
    write_manifest(
        base,
        "scopes:\n  targets: [\"src/**/*.rs\"]\ncommands:\n  - name: 'sh -c echo>>\"$1\".done sh {targets}'\n",
    );

    // --status enumerates one row per matched file.
    mmz(base).arg("--status").assert().success().stdout(
        predicate::str::contains("sh -c echo")
            .and(predicate::str::contains("src/a.rs"))
            .and(predicate::str::contains("src/b.rs")),
    );

    // Running one file records only that file; a re-run is a hit.
    let run_a = ["sh", "-c", "echo>>\"$1\".done", "sh", "src/a.rs"];
    mmz(base).args(run_a).assert().success();
    mmz(base).args(run_a).assert().success();
    assert_eq!(
        fs::read(base.join("src/a.rs.done")).expect("done").len(),
        1,
        "second run is a cache hit"
    );
    assert!(
        !base.join("src/b.rs.done").exists(),
        "b was never invoked, so its record is independent"
    );
}

#[test]
fn is_fresh_reflects_per_file_state_for_a_parametric_rule() {
    let dir = tempfile::tempdir().expect("tempdir");
    let base = dir.path();
    fs::create_dir(base.join("src")).expect("mkdir src");
    fs::write(base.join("src/a.rs"), b"a").expect("write a");
    fs::write(base.join("src/b.rs"), b"b").expect("write b");
    write_manifest(
        base,
        "scopes:\n  targets: [\"src/*.rs\"]\ncommands:\n  - name: \"sh -c true sh {targets}\"\n",
    );

    // Record only src/a.rs.
    let run_a = ["sh", "-c", "true", "sh", "src/a.rs"];
    mmz(base).args(run_a).assert().success();

    // Bug: an untargeted gate used to key the whole rule on its literal
    // `{targets}` template, which never has a record, so it always reported
    // `never`. It must instead report the real per-file expansions: fresh
    // for the recorded file, never for the unrecorded sibling.
    mmz(base).arg("--is-fresh").assert().code(1).stderr(
        predicate::str::contains("{targets}")
            .not()
            .and(predicate::str::contains("src/b.rs"))
            .and(predicate::str::contains("never")),
    );

    // Bug: a targeted gate used static matching, so `{targets}` never equalled
    // a real file and every targeted invocation returned "no rule matches".
    // It must instead gate the one expansion the command resolves to: passes
    // for the recorded file, fails for the never-invoked sibling.
    mmz(base)
        .args(["--is-fresh", "--", "sh", "-c", "true", "sh", "src/a.rs"])
        .assert()
        .success();
    mmz(base)
        .args(["--is-fresh", "--", "sh", "-c", "true", "sh", "src/b.rs"])
        .assert()
        .code(1);
}
