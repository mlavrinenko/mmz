use super::{SCHEMA, report, report_json};

fn write(dir: &std::path::Path, name: &str, body: &str) {
    std::fs::write(dir.join(name), body).expect("write");
}

fn manifest(root: &std::path::Path, body: &str) {
    let dir = root.join(".mmz");
    std::fs::create_dir_all(&dir).expect("mkdir .mmz");
    std::fs::write(dir.join("config.yaml"), body).expect("write manifest");
}

#[test]
fn reports_never_then_fresh_then_stale() {
    let dir = tempfile::tempdir().expect("tempdir");
    let base = dir.path();
    write(base, "a.txt", "one");
    manifest(
        base,
        "scopes:\n  src: [\"*.txt\"]\ncommands:\n  - name: sh\n    inputs: [src]\n",
    );

    let never = report(base, &[]).expect("report");
    assert!(never.contains("sh") && never.contains("never"));

    let argv = ["sh".to_owned(), "-c".to_owned(), "exit 0".to_owned()];
    crate::run(&argv, base).expect("recorded run");
    assert!(
        report(base, &[]).expect("report").contains("fresh"),
        "fresh after a recorded run"
    );

    write(base, "a.txt", "two");
    assert!(
        report(base, &[]).expect("report").contains("stale"),
        "stale after an input changes"
    );
}

#[test]
fn text_shows_age_after_a_run_and_json_reports_ran_at() {
    let dir = tempfile::tempdir().expect("tempdir");
    let base = dir.path();
    write(base, "a.txt", "one");
    manifest(
        base,
        "scopes:\n  src: [\"*.txt\"]\ncommands:\n  - name: sh\n    inputs: [src]\n",
    );
    let argv = ["sh".to_owned(), "-c".to_owned(), "exit 0".to_owned()];
    crate::run(&argv, base).expect("recorded run");

    assert!(
        report(base, &[]).expect("report").contains("ago"),
        "table shows a record age once a run is recorded"
    );
    let json: serde_json::Value =
        serde_json::from_str(&report_json(base, &[]).expect("json")).expect("valid json");
    let ran_at = json
        .pointer("/rules/0/cached/ran_at")
        .expect("ran_at present");
    assert!(
        ran_at.as_u64().is_some_and(|secs| secs > 0),
        "ran_at is a unix timestamp"
    );
    assert!(
        json.get("now").is_none(),
        "the report's own clock stays out of the payload; the schema forbids an undeclared key"
    );
}

/// A rule keyed on `*.txt` that declares (and its run writes) the artifact
/// `out/artifact.bin`.
fn producing_project(base: &std::path::Path) {
    write(base, "a.txt", "one");
    std::fs::create_dir_all(base.join("out")).expect("mkdir out");
    manifest(
        base,
        concat!(
            "scopes:\n  src: [\"*.txt\"]\n",
            "commands:\n  - name: sh\n    inputs: [src]\n",
            "    outputs:\n      - out/artifact.bin\n",
        ),
    );
    let argv = [
        "sh".to_owned(),
        "-c".to_owned(),
        "printf built > out/artifact.bin".to_owned(),
    ];
    crate::run(&argv, base).expect("recorded run");
}

#[test]
fn a_voided_record_names_its_missing_output_in_the_table() {
    let dir = tempfile::tempdir().expect("tempdir");
    let base = dir.path();
    producing_project(base);

    let fresh = report(base, &[]).expect("report");
    assert!(fresh.contains("fresh"), "artifact present: {fresh}");
    assert!(
        !fresh.contains("MISSING OUTPUT"),
        "the extra column stays out of an ordinary table: {fresh}"
    );

    std::fs::remove_file(base.join("out/artifact.bin")).expect("delete artifact");
    let voided = report(base, &[]).expect("report");
    assert!(
        voided.contains("missing-output"),
        "the state says what happened: {voided}"
    );
    assert!(
        voided.contains("MISSING OUTPUT") && voided.contains("out/artifact.bin"),
        "and the column names the artifact: {voided}"
    );
}

#[test]
fn json_reports_the_missing_output_and_what_the_run_promised() {
    let dir = tempfile::tempdir().expect("tempdir");
    let base = dir.path();
    producing_project(base);
    std::fs::remove_file(base.join("out/artifact.bin")).expect("delete artifact");

    let json: serde_json::Value =
        serde_json::from_str(&report_json(base, &[]).expect("json")).expect("valid json");
    let rule = json.pointer("/rules/0").expect("first rule");
    assert_eq!(
        rule.get("state").and_then(serde_json::Value::as_str),
        Some("missing-output")
    );
    assert_eq!(
        rule.get("missing_output")
            .and_then(serde_json::Value::as_str),
        Some("out/artifact.bin"),
        "the gone artifact is machine-readable, not just in the table"
    );
    assert_eq!(
        rule.pointer("/cached/outputs/0")
            .and_then(serde_json::Value::as_str),
        Some("out/artifact.bin"),
        "reported against the outputs the recorded run itself promised"
    );
}

#[test]
fn a_rule_without_outputs_reports_no_missing_output_field() {
    let dir = tempfile::tempdir().expect("tempdir");
    let base = dir.path();
    write(base, "a.txt", "one");
    manifest(
        base,
        "scopes:\n  src: [\"*.txt\"]\ncommands:\n  - name: sh\n    inputs: [src]\n",
    );
    let json: serde_json::Value =
        serde_json::from_str(&report_json(base, &[]).expect("json")).expect("valid json");
    let rule = json.pointer("/rules/0").expect("first rule");
    assert!(
        rule.get("missing_output").is_none(),
        "no outputs declared, so nothing to report"
    );
}

#[test]
fn reports_no_inputs_for_empty_scopes() {
    let dir = tempfile::tempdir().expect("tempdir");
    let base = dir.path();
    manifest(
        base,
        "scopes:\n  none: [\"*.none\"]\ncommands:\n  - name: sh\n    inputs: [none]\n",
    );
    let report = report(base, &[]).expect("report");
    assert!(report.contains("sh") && report.contains("no-inputs"));
}

#[test]
fn missing_manifest_is_an_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    assert!(report(dir.path(), &[]).is_err());
}

#[test]
fn parametric_rule_enumerates_one_row_per_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let base = dir.path();
    std::fs::create_dir(base.join("src")).expect("mkdir");
    write(&base.join("src"), "a.rs", "a");
    write(&base.join("src"), "b.rs", "b");
    manifest(
        base,
        "scopes:\n  targets: [\"src/**/*.rs\"]\ncommands:\n  - name: \"lint {targets}\"\n",
    );
    let report = report(base, &[]).expect("report");
    assert!(report.contains("lint src/a.rs"), "row for a: {report}");
    assert!(report.contains("lint src/b.rs"), "row for b: {report}");
    assert!(report.contains("never"), "each expansion has a verdict");
}

#[test]
fn parametric_expansion_inputs_are_shared_pin_union_bound_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let base = dir.path();
    write(base, "shared.txt", "pin");
    std::fs::create_dir(base.join("src")).expect("mkdir");
    write(&base.join("src"), "a.rs", "a");
    write(&base.join("src"), "b.rs", "b");
    manifest(
        base,
        "scopes:\n  pin: [\"shared.txt\"]\n  targets: [\"src/**/*.rs\"]\ncommands:\n  - name: \"lint {targets}\"\n    inputs: [pin]\n",
    );

    let json: serde_json::Value =
        serde_json::from_str(&report_json(base, &[]).expect("json")).expect("valid json");
    let rules = json
        .pointer("/rules")
        .and_then(serde_json::Value::as_array)
        .expect("rules array");
    let row = rules
        .iter()
        .find(|rule| rule.get("name").and_then(serde_json::Value::as_str) == Some("lint src/a.rs"))
        .expect("row for the a.rs expansion");

    let input_paths: Vec<&str> = row
        .pointer("/inputs")
        .and_then(serde_json::Value::as_array)
        .expect("inputs array")
        .iter()
        .map(|input| {
            input
                .get("path")
                .and_then(serde_json::Value::as_str)
                .expect("path")
        })
        .collect();
    assert_eq!(
        input_paths,
        vec!["shared.txt", "src/a.rs"],
        "the shared pin and the bound file, sorted, and nothing from the \
         sibling expansion's bound file"
    );

    let expected =
        crate::hashing::digest_files(base, &["shared.txt".to_owned(), "src/a.rs".to_owned()])
            .expect("expected digest");
    assert_eq!(
        row.get("digest").and_then(serde_json::Value::as_str),
        Some(expected.as_str()),
        "digest is over the shared pin plus the bound file, in sorted order"
    );
}

#[test]
fn colliding_expansions_are_an_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let base = dir.path();
    write(base, "a.rs", "x");
    manifest(
        base,
        "scopes:\n  wide: [\"*.rs\"]\n  narrow: [\"a.rs\"]\ncommands:\n  - name: \"do {wide}\"\n  - name: \"do {narrow}\"\n",
    );
    assert!(
        report(base, &[]).is_err(),
        "status surfaces a colliding-identity config proactively"
    );
}

#[test]
fn json_lists_inputs_with_hashes_and_state() {
    let dir = tempfile::tempdir().expect("tempdir");
    let base = dir.path();
    write(base, "a.txt", "one");
    manifest(
        base,
        "scopes:\n  src: [\"*.txt\"]\ncommands:\n  - name: sh\n    inputs: [src]\n",
    );

    let json: serde_json::Value =
        serde_json::from_str(&report_json(base, &[]).expect("json")).expect("valid json");
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
fn tag_filter_narrows_the_table_and_excludes_untagged_rules() {
    let dir = tempfile::tempdir().expect("tempdir");
    let base = dir.path();
    write(base, "a.txt", "one");
    manifest(
        base,
        "scopes:\n  src: [\"*.txt\"]\ncommands:\n  - name: sh\n    inputs: [src]\n    tags: [gate]\n  - name: cat\n    inputs: [src]\n",
    );

    let tagged = report(base, &["gate".to_owned()]).expect("report");
    assert!(tagged.contains("sh"), "tagged rule reported: {tagged}");
    assert!(
        !tagged.contains("cat"),
        "untagged rule excluded under a --tag filter: {tagged}"
    );

    let json: serde_json::Value =
        serde_json::from_str(&report_json(base, &["gate".to_owned()]).expect("json"))
            .expect("valid json");
    let rules = json.pointer("/rules").and_then(serde_json::Value::as_array);
    assert_eq!(
        rules.map(Vec::len),
        Some(1),
        "json report is filtered the same way"
    );
}

#[test]
fn json_exposes_each_resolved_probe_digest() {
    let dir = tempfile::tempdir().expect("tempdir");
    let base = dir.path();
    write(base, "a.txt", "one");
    write(base, "pinned.txt", "v1");
    manifest(
        base,
        concat!(
            "scopes:\n  src: [\"a.txt\"]\n",
            "probes:\n  tool:\n    run: cat pinned.txt\n",
            "commands:\n  - name: sh\n    inputs: [src, tool]\n",
        ),
    );

    let json: serde_json::Value =
        serde_json::from_str(&report_json(base, &[]).expect("json")).expect("valid json");
    let seen = json
        .pointer("/probes/tool")
        .and_then(serde_json::Value::as_str)
        .expect("the report exposes what mmz saw the probe print")
        .to_owned();
    assert_eq!(
        seen,
        crate::hashing::hash_bytes(b"v1"),
        "the exposed digest is the hash of the probe's stdout"
    );

    write(base, "pinned.txt", "v2");
    let moved: serde_json::Value =
        serde_json::from_str(&report_json(base, &[]).expect("json")).expect("valid json");
    assert_ne!(
        moved
            .pointer("/probes/tool")
            .and_then(serde_json::Value::as_str),
        Some(seen.as_str()),
        "a consumer can diff runs because the digest tracks the output"
    );
}

#[test]
fn a_manifest_without_probes_reports_the_shape_it_always_did() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path();
    write(root, "a.txt", "one");
    manifest(
        root,
        "scopes:\n  src: [\"*.txt\"]\ncommands:\n  - name: sh\n    inputs: [src]\n",
    );

    let json: serde_json::Value =
        serde_json::from_str(&report_json(root, &[]).expect("json")).expect("valid json");
    assert!(
        json.get("probes").is_none(),
        "the key is absent, not an empty object, so existing consumers see no change"
    );
}

#[test]
fn a_probe_only_rule_is_not_no_inputs() {
    let dir = tempfile::tempdir().expect("tempdir");
    let base = dir.path();
    manifest(
        base,
        "probes:\n  tool:\n    run: printf v1\ncommands:\n  - name: sh\n    inputs: [tool]\n",
    );
    let text = report(base, &[]).expect("report");
    assert!(
        text.contains("never") && !text.contains("no-inputs"),
        "a rule whose only input is a probe has inputs: {text}"
    );
}

#[test]
fn schema_is_valid_json_describing_the_output() {
    let schema: serde_json::Value = serde_json::from_str(SCHEMA).expect("schema is json");
    assert_eq!(
        schema.get("$schema").and_then(serde_json::Value::as_str),
        Some("https://json-schema.org/draft/2020-12/schema")
    );
    for key in [
        "manifest",
        "rules",
        "state",
        "source",
        "inputs",
        "no-inputs",
        "ran_at",
        "missing-output",
        "missing_output",
        "outputs",
        "probes",
    ] {
        assert!(SCHEMA.contains(key), "schema mentions `{key}`");
    }
}
