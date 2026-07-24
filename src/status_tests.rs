use super::{SCHEMA, humanize_age, report, report_json};

fn write(dir: &std::path::Path, name: &str, body: &str) {
    std::fs::write(dir.join(name), body).expect("write");
}

fn manifest(root: &std::path::Path, body: &str) {
    let dir = root.join(".mmz");
    std::fs::create_dir_all(&dir).expect("mkdir .mmz");
    std::fs::write(dir.join("config.yaml"), body).expect("write manifest");
}

#[test]
fn humanize_age_scales_by_unit() {
    assert_eq!(humanize_age(5), "5s ago");
    assert_eq!(humanize_age(90), "1m ago");
    assert_eq!(humanize_age(3 * 3600), "3h ago");
    assert_eq!(humanize_age(2 * 86_400), "2d ago");
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
        "inputs",
        "no-inputs",
        "ran_at",
    ] {
        assert!(SCHEMA.contains(key), "schema mentions `{key}`");
    }
}
