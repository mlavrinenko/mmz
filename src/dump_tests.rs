//! Unit tests for `mmz --dump-config` and `--dump-config=json`: the merged
//! manifest, and the provenance attached to every scope, probe and command in
//! it. Precedent for the fixtures: `status_source_tests.rs`, which covers the
//! same import shapes for `--status`'s rule-level `source`.

use std::path::Path;

use super::{dump, dump_json};

fn write(dir: &Path, name: &str, body: &str) {
    let path = dir.join(name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("mkdir");
    }
    std::fs::write(path, body).expect("write");
}

fn manifest(root: &Path, body: &str) {
    let dir = root.join(".mmz");
    std::fs::create_dir_all(&dir).expect("mkdir .mmz");
    std::fs::write(dir.join("config.yaml"), body).expect("write manifest");
}

/// A project with no `imports:` at all has exactly one source — the root
/// manifest — and every scope and command reports it, matching
/// [`crate::provenance`]'s "no imports is the degenerate case, not a special
/// case" rule.
#[test]
fn a_single_file_project_dumps_with_itself_as_the_only_source() {
    let dir = tempfile::tempdir().expect("tempdir");
    let base = dir.path();
    write(base, "a.txt", "one");
    manifest(
        base,
        "scopes:\n  src: [\"*.txt\"]\ncommands:\n  - name: sh\n    inputs: [src]\n",
    );

    let text = dump(base).expect("dump");
    assert!(
        text.contains("sources:\n  1  .mmz/config.yaml\n"),
        "the root manifest is the sole, first-numbered source: {text}"
    );
    assert!(
        text.contains("src:  # .mmz/config.yaml"),
        "the scope is annotated with the root manifest: {text}"
    );
    assert!(
        text.contains("sh:  # .mmz/config.yaml"),
        "the command is annotated with the root manifest: {text}"
    );

    let json: serde_json::Value =
        serde_json::from_str(&dump_json(base).expect("json")).expect("valid json");
    assert_eq!(
        json.get("sources").and_then(serde_json::Value::as_array),
        Some(&vec![serde_json::Value::String(
            ".mmz/config.yaml".to_owned()
        )])
    );
    assert_eq!(
        json.pointer("/scopes/0/source")
            .and_then(serde_json::Value::as_str),
        Some(".mmz/config.yaml")
    );
    assert_eq!(
        json.pointer("/commands/0/source")
            .and_then(serde_json::Value::as_str),
        Some(".mmz/config.yaml")
    );
}

/// Writes the three-file composed fixture every test in this module but the
/// single-file one shares: a root that imports a `conf.d/` fragment, which in
/// turn imports a third file *outside* `conf.d/` — a genuine two-hop chain,
/// not a file the root's own directory scan would also reach, so attributing
/// its entries correctly proves the nested import was actually followed.
///
/// Load order: `.mmz/config.yaml`, then `.mmz/conf.d/10-rust.yaml` (the only
/// entry `conf.d/` expands to), then `.mmz/nested.yaml`, reached only via
/// `10-rust.yaml`'s own `imports: [../nested.yaml]` — relative to *its own*
/// directory, `.mmz/conf.d/`, which is what makes `../nested.yaml` resolve to
/// `.mmz/nested.yaml` rather than `.mmz/conf.d/nested.yaml`.
fn composed_project(base: &Path) {
    write(base, "a.txt", "one");
    write(base, "a.rs", "one");
    manifest(
        base,
        "imports: [conf.d/]\nscopes:\n  src: [\"*.txt\"]\ncommands:\n  - name: sh\n    inputs: [src]\n",
    );
    write(
        base,
        ".mmz/conf.d/10-rust.yaml",
        "imports: [../nested.yaml]\nscopes:\n  rust: [\"*.rs\"]\ncommands:\n  - name: lint\n    inputs: [rust]\n",
    );
    write(
        base,
        ".mmz/nested.yaml",
        "probes:\n  fmt-recipe:\n    run: echo hi\ncommands:\n  - name: nested-cmd\n    inputs: [fmt-recipe]\n",
    );
}

/// Every scope, probe and command reports the file that actually declared
/// it — the root, the fragment `conf.d/` names directly, and the fragment
/// reached only through that fragment's own `imports:` — not the file that
/// pulled it in transitively.
#[test]
fn a_composed_project_attributes_every_entry_to_its_declaring_file_through_a_nested_import() {
    let dir = tempfile::tempdir().expect("tempdir");
    let base = dir.path();
    composed_project(base);

    let json: serde_json::Value =
        serde_json::from_str(&dump_json(base).expect("json")).expect("valid json");

    let source = |pointer: &str| {
        json.pointer(pointer)
            .and_then(serde_json::Value::as_str)
            .unwrap_or_else(|| panic!("missing {pointer} in {json}"))
            .to_owned()
    };
    // `scopes` is keyed by name in a `BTreeMap`, so entries come out sorted
    // alphabetically ("rust" before "src") rather than in declaration order.
    assert_eq!(
        source("/scopes/0/source"),
        ".mmz/conf.d/10-rust.yaml",
        "rust"
    );
    assert_eq!(source("/scopes/1/source"), ".mmz/config.yaml", "src");
    assert_eq!(source("/probes/0/source"), ".mmz/nested.yaml", "fmt-recipe");
    assert_eq!(source("/commands/0/source"), ".mmz/config.yaml", "sh");
    assert_eq!(
        source("/commands/1/source"),
        ".mmz/conf.d/10-rust.yaml",
        "lint"
    );
    assert_eq!(
        source("/commands/2/source"),
        ".mmz/nested.yaml",
        "nested-cmd"
    );

    let text = dump(base).expect("dump");
    assert!(
        text.contains("rust:  # .mmz/conf.d/10-rust.yaml"),
        "the human form annotates the same way: {text}"
    );
    assert!(
        text.contains("nested-cmd:  # .mmz/nested.yaml"),
        "including the nested-only fragment: {text}"
    );
}

/// The JSON form is valid JSON carrying `sources` in load order — root first,
/// then each import depth-first — the same order the human form numbers.
#[test]
fn the_json_form_round_trips_and_carries_sources_in_load_order() {
    let dir = tempfile::tempdir().expect("tempdir");
    let base = dir.path();
    composed_project(base);

    let rendered = dump_json(base).expect("json");
    let json: serde_json::Value = serde_json::from_str(&rendered).expect("valid json");
    let sources: Vec<&str> = json
        .get("sources")
        .and_then(serde_json::Value::as_array)
        .expect("sources array")
        .iter()
        .map(|value| value.as_str().expect("string source"))
        .collect();
    assert_eq!(
        sources,
        vec![
            ".mmz/config.yaml",
            ".mmz/conf.d/10-rust.yaml",
            ".mmz/nested.yaml",
        ],
        "root first, then each import depth-first"
    );
    assert_eq!(
        json.get("manifest").and_then(serde_json::Value::as_str),
        Some(".mmz/config.yaml"),
        "manifest names the same file sources[0] does"
    );

    // Round-trips: re-serializing the parsed value reproduces the same bytes
    // modulo whitespace, i.e. nothing was lost decoding it back through serde.
    let reparsed: serde_json::Value =
        serde_json::from_str(&serde_json::to_string(&json).expect("re-serialize"))
            .expect("re-parse");
    assert_eq!(json, reparsed);
}

/// An import outside the project root — a Nix store path, in the wild — has
/// nothing to render root-relative against, so every `source` naming it stays
/// absolute; a fragment under the root still renders root-relative. Precedent:
/// `status_source_tests::a_store_path_fragments_rule_reports_the_absolute_path`.
#[test]
fn a_store_path_fragment_shows_its_absolute_path_and_an_in_root_one_shows_a_relative_path() {
    let project = tempfile::tempdir().expect("project tempdir");
    let base = project.path();
    write(base, "a.txt", "one");

    let store = tempfile::tempdir().expect("store tempdir");
    let fragment = store.path().join("rules.yaml");
    std::fs::write(
        &fragment,
        "scopes:\n  ext: [\"*.txt\"]\ncommands:\n  - name: ext-cmd\n    inputs: [ext]\n",
    )
    .expect("write store fragment");
    manifest(
        base,
        &format!(
            "imports: [{}]\nscopes:\n  src: [\"*.txt\"]\ncommands:\n  - name: sh\n    inputs: [src]\n",
            fragment.display()
        ),
    );
    let expected_store_path = std::fs::canonicalize(&fragment)
        .expect("canonicalize")
        .display()
        .to_string();

    let json: serde_json::Value =
        serde_json::from_str(&dump_json(base).expect("json")).expect("valid json");
    // `scopes` sorts alphabetically by name ("ext" before "src"), not by
    // which file declared it.
    assert_eq!(
        json.pointer("/scopes/0/source")
            .and_then(serde_json::Value::as_str),
        Some(expected_store_path.as_str()),
        "the store-path fragment's scope renders absolute"
    );
    assert_eq!(
        json.pointer("/scopes/1/source")
            .and_then(serde_json::Value::as_str),
        Some(".mmz/config.yaml"),
        "the in-root scope renders root-relative"
    );
    let sources: Vec<&str> = json
        .get("sources")
        .and_then(serde_json::Value::as_array)
        .expect("sources array")
        .iter()
        .map(|value| value.as_str().expect("string source"))
        .collect();
    assert_eq!(
        sources,
        vec![".mmz/config.yaml", expected_store_path.as_str()]
    );
}

/// A manifest that fails to merge is [`Err`] from both renderings, with the
/// merge error naming the offending key and (as part of the underlying
/// `PathBuf` `Display`) both source files — the exact failure `--dump-config`
/// is deliberately not a debugging aid for, per the task's `Surface` section.
/// Because [`dump`] and [`dump_json`] only ever build a [`String`] from a
/// fully-collected model, there is no code path that could print a partial
/// one: an `Err` here is structurally the only thing either function can
/// return before touching stdout.
#[test]
fn an_invalid_manifest_errors_from_both_forms_instead_of_dumping_partially() {
    let dir = tempfile::tempdir().expect("tempdir");
    let base = dir.path();
    write(base, ".mmz/conf.d/a.yaml", "scopes:\n  rust: [\"*.rs\"]\n");
    manifest(base, "imports: [conf.d/]\nscopes:\n  rust: [\"*.txt\"]\n");

    let err = dump(base).expect_err("a scope declared in two files fails to merge");
    let text = err.to_string();
    assert!(
        text.contains("rust") && text.contains("declared in both"),
        "the merge error names the offending key: {text}"
    );

    let json_err = dump_json(base).expect_err("the json form fails identically");
    assert_eq!(
        err.to_string(),
        json_err.to_string(),
        "both renderings surface the same merge error"
    );
}
