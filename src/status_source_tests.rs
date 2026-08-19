//! Unit tests for `source` — each rule's declaring file, surfaced in
//! `--status=json` and, once more than one file contributes rules, as the
//! table's `SOURCE` column. Split out of `status_tests.rs`, which the general
//! freshness/report tests already filled to its own cap; this file is the
//! seam for composition-specific coverage as it grows.

use super::{report, report_json};

fn write(dir: &std::path::Path, name: &str, body: &str) {
    std::fs::write(dir.join(name), body).expect("write");
}

fn manifest(root: &std::path::Path, body: &str) {
    let dir = root.join(".mmz");
    std::fs::create_dir_all(&dir).expect("mkdir .mmz");
    std::fs::write(dir.join("config.yaml"), body).expect("write manifest");
}

/// A rule declared directly in the root manifest reports the root manifest as
/// its `source` — the degenerate case of composition, not a special case, so
/// this holds even in a single-file project with no `imports:` at all.
#[test]
fn a_rule_from_the_root_manifest_reports_the_root_manifest_as_its_source() {
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
    assert_eq!(
        rule.get("source").and_then(serde_json::Value::as_str),
        Some(".mmz/config.yaml"),
        "the root manifest is the source of its own rule even with nothing imported"
    );
}

/// A rule pulled in through `imports:` reports the fragment that declared it,
/// not the root manifest that imported it — exactly the fact someone
/// debugging a surprising skip needs.
#[test]
fn a_rule_from_an_imported_fragment_reports_the_fragment_as_its_source() {
    let dir = tempfile::tempdir().expect("tempdir");
    let base = dir.path();
    write(base, "a.rs", "one");
    let mmz_dir = base.join(".mmz");
    std::fs::create_dir_all(mmz_dir.join("conf.d")).expect("mkdir conf.d");
    std::fs::write(
        mmz_dir.join("conf.d/generated.yaml"),
        "scopes:\n  rust: [\"*.rs\"]\ncommands:\n  - name: lint\n    inputs: [rust]\n",
    )
    .expect("write fragment");
    manifest(base, "imports: [conf.d/]\n");

    let json: serde_json::Value =
        serde_json::from_str(&report_json(base, &[]).expect("json")).expect("valid json");
    let rule = json.pointer("/rules/0").expect("first rule");
    assert_eq!(
        rule.get("source").and_then(serde_json::Value::as_str),
        Some(".mmz/conf.d/generated.yaml"),
        "the fragment that declared the rule, not the root that imported it"
    );
}

/// An import outside the project root — a Nix store path, in the wild — has
/// nothing to render root-relative against, so `source` stays absolute rather
/// than collapsing into a long `../../..` climb.
#[test]
fn a_store_path_fragments_rule_reports_the_absolute_path() {
    let project = tempfile::tempdir().expect("project tempdir");
    let base = project.path();
    write(base, "a.txt", "one");

    let store = tempfile::tempdir().expect("store tempdir");
    let fragment = store.path().join("rules.yaml");
    std::fs::write(
        &fragment,
        "scopes:\n  src: [\"*.txt\"]\ncommands:\n  - name: sh\n    inputs: [src]\n",
    )
    .expect("write fragment");
    manifest(base, &format!("imports: [{}]\n", fragment.display()));

    let json: serde_json::Value =
        serde_json::from_str(&report_json(base, &[]).expect("json")).expect("valid json");
    let rule = json.pointer("/rules/0").expect("first rule");
    let expected = std::fs::canonicalize(&fragment)
        .expect("canonicalize")
        .display()
        .to_string();
    assert_eq!(
        rule.get("source").and_then(serde_json::Value::as_str),
        Some(expected.as_str()),
        "outside the project root, source stays an absolute path"
    );
}

/// End to end through the real merge and the real table renderer: a project
/// whose rules come from two files grows the `SOURCE` column, naming each
/// rule's file; a project with everything in one file never does.
#[test]
fn the_table_grows_a_source_column_only_once_two_files_contribute_rules() {
    let dir = tempfile::tempdir().expect("tempdir");
    let base = dir.path();
    write(base, "a.txt", "one");
    write(base, "a.rs", "one");
    let mmz_dir = base.join(".mmz");
    std::fs::create_dir_all(mmz_dir.join("conf.d")).expect("mkdir conf.d");
    std::fs::write(
        mmz_dir.join("conf.d/generated.yaml"),
        "scopes:\n  rust: [\"*.rs\"]\ncommands:\n  - name: lint\n    inputs: [rust]\n",
    )
    .expect("write fragment");
    manifest(
        base,
        concat!(
            "imports: [conf.d/]\n",
            "scopes:\n  src: [\"*.txt\"]\n",
            "commands:\n  - name: sh\n    inputs: [src]\n    tags: [only-root]\n",
        ),
    );

    let composed = report(base, &[]).expect("report");
    assert!(
        composed.contains("SOURCE"),
        "two files contributed rules: {composed}"
    );
    assert!(
        composed.contains(".mmz/config.yaml") && composed.contains(".mmz/conf.d/generated.yaml"),
        "each rule's own file is named: {composed}"
    );

    // Same project, but filtered down to the root's own tagged rule — the
    // fragment's untagged rule drops out, only one file contributed to what
    // remains, and the column disappears again.
    let filtered = report(base, &["only-root".to_owned()]).expect("report");
    assert!(
        !filtered.contains("SOURCE"),
        "filtered back down to one source: {filtered}"
    );
}
