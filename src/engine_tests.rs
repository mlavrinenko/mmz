use super::run;

fn write_manifest(dir: &std::path::Path, body: &str) {
    let cfg = dir.join(".mmz");
    std::fs::create_dir_all(&cfg).expect("mkdir .mmz");
    std::fs::write(cfg.join("config.yaml"), body).expect("write manifest");
}

#[test]
fn skips_second_run_when_inputs_unchanged() {
    let dir = tempfile::tempdir().expect("tempdir");
    let base = dir.path();
    std::fs::write(base.join("a.txt"), b"one").expect("input");
    write_manifest(
        base,
        "scopes:\n  src: [\"*.txt\"]\ncommands:\n  - name: sh\n    inputs: [src]\n",
    );

    let argv = [
        "sh".to_owned(),
        "-c".to_owned(),
        "printf x >> runs.log".to_owned(),
    ];
    assert_eq!(run(&argv, base).expect("run"), 0);
    assert_eq!(run(&argv, base).expect("run"), 0);
    assert_eq!(
        std::fs::read(base.join("runs.log")).expect("log").len(),
        1,
        "skipped once"
    );

    std::fs::write(base.join("a.txt"), b"two").expect("rewrite");
    assert_eq!(run(&argv, base).expect("run"), 0);
    assert_eq!(
        std::fs::read(base.join("runs.log")).expect("log").len(),
        2,
        "input change re-runs"
    );
}

fn run_twice(base: &std::path::Path) {
    let argv = ["sh".to_owned(), "-c".to_owned(), "exit 0".to_owned()];
    assert_eq!(run(&argv, base).expect("first run records"), 0);
    assert_eq!(run(&argv, base).expect("second run is a hit"), 0);
}

#[test]
fn on_hit_paths_are_exercised_on_a_cache_hit() {
    // Per-command on_hit (overrides absent global) with a macro: the hit path
    // resolves and prints the notice.
    let rule = tempfile::tempdir().expect("tempdir");
    std::fs::write(rule.path().join("a.txt"), b"one").expect("input");
    write_manifest(
        rule.path(),
        "scopes:\n  src: [\"*.txt\"]\ncommands:\n  - name: sh\n    inputs: [src]\n    on_hit: \"skip {cache:command}\"\n",
    );
    run_twice(rule.path());

    // An empty global on_hit suppresses the line without erroring.
    let blank = tempfile::tempdir().expect("tempdir");
    std::fs::write(blank.path().join("a.txt"), b"one").expect("input");
    write_manifest(
        blank.path(),
        "scopes:\n  src: [\"*.txt\"]\non_hit: \"\"\ncommands:\n  - name: sh\n    inputs: [src]\n",
    );
    run_twice(blank.path());
}

#[test]
fn propagates_exit_code_and_reruns_after_failure() {
    let dir = tempfile::tempdir().expect("tempdir");
    let base = dir.path();
    std::fs::write(base.join("a.txt"), b"one").expect("input");
    write_manifest(
        base,
        "scopes:\n  src: [\"*.txt\"]\ncommands:\n  - name: sh\n    inputs: [src]\n",
    );

    let argv = ["sh".to_owned(), "-c".to_owned(), "exit 3".to_owned()];
    assert_eq!(run(&argv, base).expect("run"), 3, "exit code propagates");
    assert_eq!(
        run(&argv, base).expect("run"),
        3,
        "failure was not cached as fresh"
    );
}

/// A rule keyed on `*.txt` that declares the artifact `out/artifact.bin`.
fn write_producer(dir: &std::path::Path) {
    std::fs::write(dir.join("a.txt"), b"one").expect("input");
    std::fs::create_dir_all(dir.join("out")).expect("mkdir out");
    write_manifest(
        dir,
        concat!(
            "scopes:\n  src: [\"*.txt\"]\n",
            "commands:\n  - name: sh\n    inputs: [src]\n",
            "    outputs:\n      - out/artifact.bin\n",
        ),
    );
}

fn build_argv() -> [String; 3] {
    [
        "sh".to_owned(),
        "-c".to_owned(),
        "printf x >> runs.log; printf built > out/artifact.bin".to_owned(),
    ]
}

fn runs(base: &std::path::Path) -> usize {
    std::fs::read(base.join("runs.log")).map_or(0, |bytes| bytes.len())
}

#[test]
fn a_deleted_output_re_runs_a_rule_whose_inputs_never_moved() {
    let dir = tempfile::tempdir().expect("tempdir");
    let base = dir.path();
    write_producer(base);

    assert_eq!(run(&build_argv(), base).expect("run"), 0);
    assert_eq!(runs(base), 1, "first run executes");
    assert_eq!(run(&build_argv(), base).expect("run"), 0);
    assert_eq!(runs(base), 1, "artifact present, inputs unchanged: skipped");

    // The `cargo clean` case: the artifact goes, every input stays.
    std::fs::remove_file(base.join("out/artifact.bin")).expect("delete artifact");
    assert_eq!(run(&build_argv(), base).expect("run"), 0);
    assert_eq!(runs(base), 2, "the voided record re-runs the command");
    assert_eq!(run(&build_argv(), base).expect("run"), 0);
    assert_eq!(runs(base), 2, "and the fresh artifact restores the skip");
}

#[test]
fn a_success_without_the_declared_output_errors_and_records_nothing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let base = dir.path();
    write_producer(base);

    let liar = ["sh".to_owned(), "-c".to_owned(), "exit 0".to_owned()];
    let err = run(&liar, base).expect_err("a success without its output is refused");
    assert!(
        matches!(&err, crate::Error::MissingOutput { path, .. } if path == "out/artifact.bin"),
        "the error names the missing artifact: {err}"
    );

    let records: Vec<_> = std::fs::read_dir(base.join(".mmz/cache"))
        .map(|entries| entries.filter_map(Result::ok).collect())
        .unwrap_or_default();
    assert!(
        records.is_empty(),
        "no record is written, so the rule cannot skip on a claim it did not honour"
    );

    // The next honest run still executes and records normally.
    assert_eq!(run(&build_argv(), base).expect("run"), 0);
    assert_eq!(runs(base), 1);
    assert_eq!(run(&build_argv(), base).expect("run"), 0);
    assert_eq!(runs(base), 1, "recorded once the artifact really landed");
}

#[test]
fn a_failing_run_is_recorded_as_a_failure_not_an_output_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let base = dir.path();
    write_producer(base);

    let argv = ["sh".to_owned(), "-c".to_owned(), "exit 3".to_owned()];
    assert_eq!(
        run(&argv, base).expect("a failing run keeps its own exit code"),
        3,
        "the command's failure is the story, not its absent output"
    );
}

#[test]
fn a_glob_in_outputs_is_a_manifest_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let base = dir.path();
    std::fs::write(base.join("a.txt"), b"one").expect("input");
    write_manifest(
        base,
        concat!(
            "scopes:\n  src: [\"*.txt\"]\n",
            "commands:\n  - name: sh\n    inputs: [src]\n",
            "    outputs:\n      - \"out/*.bin\"\n",
        ),
    );
    let argv = ["sh".to_owned(), "-c".to_owned(), "exit 0".to_owned()];
    assert!(
        matches!(run(&argv, base), Err(crate::Error::InvalidOutput { .. })),
        "a pattern is refused at load, not left to never match"
    );
}

#[test]
fn missing_manifest_is_an_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let argv = ["sh".to_owned(), "-c".to_owned(), "exit 0".to_owned()];
    assert!(
        matches!(run(&argv, dir.path()), Err(crate::Error::NoManifest { .. })),
        "no manifest is fatal, not passthrough"
    );
}

#[test]
fn invalid_manifest_is_an_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let base = dir.path();
    write_manifest(base, "commands:\n  - name: sh\n    inputs: [ghost]\n");
    let argv = ["sh".to_owned(), "-c".to_owned(), "exit 0".to_owned()];
    assert!(run(&argv, base).is_err(), "invalid manifest is fatal");
}

#[test]
fn no_match_errors_under_strict_but_passes_through_when_relaxed() {
    let dir = tempfile::tempdir().expect("tempdir");
    let base = dir.path();
    let argv = ["sh".to_owned(), "-c".to_owned(), "exit 5".to_owned()];

    write_manifest(base, "commands:\n  - name: cargo\n");
    assert!(
        matches!(run(&argv, base), Err(crate::Error::NoMatch { .. })),
        "strict default errors on no match"
    );

    write_manifest(base, "commands:\n  - name: cargo\nstrict: []\n");
    assert_eq!(run(&argv, base).expect("run"), 5, "relaxed runs unmemoized");
}

#[test]
fn empty_input_set_errors_under_strict() {
    let dir = tempfile::tempdir().expect("tempdir");
    let base = dir.path();
    write_manifest(
        base,
        "scopes:\n  none: [\"*.none\"]\ncommands:\n  - name: sh\n    inputs: [none]\n",
    );
    let argv = ["sh".to_owned(), "-c".to_owned(), "exit 0".to_owned()];
    assert!(
        matches!(run(&argv, base), Err(crate::Error::NoInputs { .. })),
        "strict default errors on empty input set"
    );
}

#[test]
fn empty_input_set_runs_every_time_when_relaxed() {
    let dir = tempfile::tempdir().expect("tempdir");
    let base = dir.path();
    write_manifest(
        base,
        "scopes:\n  none: [\"*.none\"]\ncommands:\n  - name: sh\n    inputs: [none]\nstrict: [no_match]\n",
    );
    let argv = [
        "sh".to_owned(),
        "-c".to_owned(),
        "printf x >> runs.log".to_owned(),
    ];
    assert_eq!(run(&argv, base).expect("run"), 0);
    assert_eq!(run(&argv, base).expect("run"), 0);
    assert_eq!(
        std::fs::read(base.join("runs.log")).expect("log").len(),
        2,
        "relaxed no-inputs never memoizes"
    );
}

/// A parametric rule fanned over `src/**/*.rs`. The executed command appends
/// a byte to `<file>.ran` via sh's `$1`, so a run is observable while a
/// cache hit leaves the counter untouched.
fn write_fan(base: &std::path::Path) {
    std::fs::create_dir_all(base.join("src")).expect("mkdir src");
    std::fs::write(base.join("src/a.rs"), b"a").expect("a");
    std::fs::write(base.join("src/b.rs"), b"b").expect("b");
    write_manifest(
        base,
        "scopes:\n  targets: [\"src/**/*.rs\"]\ncommands:\n  - name: 'sh -c echo>>\"$1\".ran sh {targets}'\n",
    );
}

fn fan_argv(file: &str) -> [String; 5] {
    [
        "sh".to_owned(),
        "-c".to_owned(),
        "echo>>\"$1\".ran".to_owned(),
        "sh".to_owned(),
        file.to_owned(),
    ]
}

fn ran_count(base: &std::path::Path, file: &str) -> usize {
    std::fs::read(base.join(format!("{file}.ran"))).map_or(0, |bytes| bytes.len())
}

#[test]
fn parametric_rule_busts_only_its_own_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let base = dir.path();
    write_fan(base);

    assert_eq!(run(&fan_argv("src/a.rs"), base).expect("run a"), 0);
    assert_eq!(ran_count(base, "src/a.rs"), 1, "a ran once");
    assert_eq!(run(&fan_argv("src/a.rs"), base).expect("run a"), 0);
    assert_eq!(ran_count(base, "src/a.rs"), 1, "a's second run is a hit");

    // b has its own record, unaffected by a.
    assert_eq!(run(&fan_argv("src/b.rs"), base).expect("run b"), 0);
    assert_eq!(ran_count(base, "src/b.rs"), 1, "b ran once");
    assert_eq!(ran_count(base, "src/a.rs"), 1, "a untouched by b");

    // Editing b busts only b; a stays fresh (tight per-file scoping).
    std::fs::write(base.join("src/b.rs"), b"edited").expect("edit b");
    assert_eq!(run(&fan_argv("src/a.rs"), base).expect("run a"), 0);
    assert_eq!(
        ran_count(base, "src/a.rs"),
        1,
        "sibling edit did not bust a"
    );
    assert_eq!(run(&fan_argv("src/b.rs"), base).expect("run b"), 0);
    assert_eq!(ran_count(base, "src/b.rs"), 2, "b's own edit re-ran it");
}

#[test]
fn off_domain_file_falls_through_to_no_match() {
    let dir = tempfile::tempdir().expect("tempdir");
    let base = dir.path();
    write_fan(base);
    // A .txt path is not in the `src/**/*.rs` domain, so no rule matches.
    let argv = fan_argv("src/c.txt");
    assert!(
        matches!(run(&argv, base), Err(crate::Error::NoMatch { .. })),
        "an off-domain file does not fan a record"
    );
}

#[test]
fn colliding_parametric_expansions_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let base = dir.path();
    std::fs::write(base.join("a.rs"), b"x").expect("a");
    write_manifest(
        base,
        "scopes:\n  wide: [\"*.rs\"]\n  narrow: [\"a.rs\"]\ncommands:\n  - name: \"do {wide}\"\n  - name: \"do {narrow}\"\n",
    );
    let argv = ["do".to_owned(), "a.rs".to_owned()];
    assert!(
        matches!(
            run(&argv, base),
            Err(crate::Error::CollidingIdentity { .. })
        ),
        "two rules claiming `do a.rs` is a loud error, not a silent winner"
    );
}

#[test]
fn malformed_macro_is_a_config_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let base = dir.path();
    write_manifest(base, "commands:\n  - name: \"do {a} {b}\"\n");
    let argv = ["do".to_owned(), "x".to_owned()];
    assert!(
        matches!(run(&argv, base), Err(crate::Error::MacroSyntax { .. })),
        "a two-macro name is rejected at load"
    );
}
