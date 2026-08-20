//! Unit tests for a probe's `json:` and `file:` keys, split from
//! `probe_tests.rs` so neither file reaches the line cap.
//!
//! What is tested here is the half of [`crate::probe`] that never spawns
//! anything: reading a file, selecting out of it, refusing a selection that
//! measures nothing, and refusing a probe whose source keys do not describe
//! one readable thing.

use super::{Resolver, validate};
use crate::manifest::Manifest;

/// Parses a manifest body, skipping validation so a test can build a shape
/// `validate` would reject on purpose.
fn parse(body: &str) -> Manifest {
    serde_yaml_ng::from_str(body).expect("manifest parses")
}

/// The rule named `sh` in `manifest` — every fixture below declares exactly
/// one rule, under that name.
fn rule(manifest: &Manifest) -> &crate::manifest::Command {
    manifest
        .commands
        .iter()
        .find(|rule| rule.name == "sh")
        .expect("rule declared")
}
/// A project root holding `data.json` with the given body, plus the manifest
/// that reads it. The file-sourced shape needs a real file on disk, which is
/// the whole point: no process is spawned to reach it.
fn json_project(body: &str, manifest: &str) -> (tempfile::TempDir, Manifest) {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("data.json"), body).expect("write json");
    let manifest = parse(manifest);
    (dir, manifest)
}

/// The digest of the single-rule manifest's probe, resolved from `dir`.
fn digest_of(dir: &tempfile::TempDir, manifest: &Manifest, probe: &str) -> String {
    Resolver::new(manifest, dir.path())
        .for_rule(rule(manifest))
        .expect("probe resolves")
        .get(probe)
        .cloned()
        .unwrap_or_else(|| panic!("probe `{probe}` resolved"))
}

/// A manifest whose one rule reads `data.json` through `program`.
fn file_manifest(program: &str) -> String {
    format!(
        "probes:\n  pick:\n    file: data.json\n    json: '{program}'\ncommands:\n  - name: sh\n    inputs: [pick]\n"
    )
}

#[test]
fn a_file_probe_hashes_the_selection_without_spawning_anything() {
    let (dir, manifest) = json_project(
        r#"{"nodes": {"qahq": {"locked": {"narHash": "sha256-abc"}}}}"#,
        &file_manifest(r#".nodes["qahq"]["locked"]["narHash"]"#),
    );
    assert_eq!(
        digest_of(&dir, &manifest, "pick"),
        crate::hashing::hash_bytes(b"\"sha256-abc\"\n"),
        "the digest is the blake3 of the selected value, canonically rendered"
    );
}

#[test]
fn only_the_selected_part_of_the_file_moves_the_digest() {
    let program = file_manifest(".tracked");
    let (first, manifest) = json_project(r#"{"tracked": 1, "ignored": "a"}"#, &program);
    let (second, _) = json_project(r#"{"tracked": 1, "ignored": "b"}"#, &program);
    let (third, _) = json_project(r#"{"tracked": 2, "ignored": "a"}"#, &program);

    assert_eq!(
        digest_of(&first, &manifest, "pick"),
        digest_of(&second, &manifest, "pick"),
        "a sibling key the selector does not name is not an input"
    );
    assert_ne!(
        digest_of(&first, &manifest, "pick"),
        digest_of(&third, &manifest, "pick"),
        "the selected value is"
    );
}

#[test]
fn reordering_the_source_objects_keys_does_not_move_the_digest() {
    let program = file_manifest(".");
    let (first, manifest) = json_project(r#"{"b": 1, "a": {"d": 2, "c": 3}}"#, &program);
    let (second, _) = json_project(r#"{"a": {"c": 3, "d": 2}, "b": 1}"#, &program);
    assert_eq!(
        digest_of(&first, &manifest, "pick"),
        digest_of(&second, &manifest, "pick"),
        "mmz renders the parsed value with keys sorted, so key order is structurally \
         not an input — nobody has to remember `jq -S`"
    );

    let ordered = file_manifest(".list");
    let (one, ordered_manifest) = json_project(r#"{"list": [1, 2]}"#, &ordered);
    let (two, _) = json_project(r#"{"list": [2, 1]}"#, &ordered);
    assert_ne!(
        digest_of(&one, &ordered_manifest, "pick"),
        digest_of(&two, &ordered_manifest, "pick"),
        "array order is content, not presentation, so it stays an input"
    );
}

#[test]
fn a_selector_that_matches_nothing_is_a_hard_error() {
    let (dir, manifest) = json_project(r#"{"a": 1}"#, &file_manifest(".missing"));
    let err = Resolver::new(&manifest, dir.path())
        .for_rule(rule(&manifest))
        .expect_err("a null selection never reaches the hasher");
    let message = err.to_string();
    assert!(message.contains("probe `pick`"), "names it: {message}");
    assert!(
        message.contains("selected nothing"),
        "says what happened: {message}"
    );
    assert!(
        message.contains("fresh forever"),
        "says why that cannot stand: {message}"
    );

    let (empty_dir, empty) = json_project(r#"{"a": 1}"#, &file_manifest(".[] | select(. > 9)"));
    assert!(
        Resolver::new(&empty, empty_dir.path())
            .for_rule(rule(&empty))
            .is_err(),
        "zero outputs is the same refusal as a lone null"
    );
}

#[test]
fn false_is_a_value_but_null_is_not() {
    let (dir, manifest) = json_project(r#"{"on": false}"#, &file_manifest(".on"));
    assert_eq!(
        digest_of(&dir, &manifest, "pick"),
        crate::hashing::hash_bytes(b"false\n"),
        "`false` is a value a rule can legitimately track; jq's -e conflates it with \
         null only because a shell exit code cannot tell them apart"
    );

    let (null_dir, null) = json_project(r#"{"on": null}"#, &file_manifest(".on"));
    assert!(
        Resolver::new(&null, null_dir.path())
            .for_rule(rule(&null))
            .is_err(),
        "an explicit null is still a digest that measures nothing"
    );
}

#[test]
fn allow_empty_opts_into_a_selection_that_matched_nothing() {
    let (dir, manifest) = json_project(
        r#"{"a": 1}"#,
        "probes:\n  pick:\n    file: data.json\n    json: '.missing'\n    allow_empty: true\ncommands:\n  - name: sh\n    inputs: [pick]\n",
    );
    assert_eq!(
        digest_of(&dir, &manifest, "pick"),
        crate::hashing::hash_bytes(b"null\n"),
        "the same key that accepts empty stdout accepts an empty selection, and the \
         opted-in probe still contributes a stable digest"
    );
}

#[test]
fn a_selector_with_two_outputs_has_a_stable_digest() {
    let (dir, manifest) = json_project(r#"{"a": 1, "b": 2}"#, &file_manifest(".a, .b"));
    assert_eq!(
        digest_of(&dir, &manifest, "pick"),
        crate::hashing::hash_bytes(b"1\n2\n"),
        "multiple outputs join one per line, the shape `jq -c` itself prints"
    );

    let (one_dir, one) = json_project(r#"{"a": 1, "b": 2}"#, &file_manifest(".a"));
    assert_ne!(
        digest_of(&dir, &manifest, "pick"),
        digest_of(&one_dir, &one, "pick"),
        "a selector that grew a second output is a different input"
    );
}

#[test]
fn a_run_line_can_be_selected_out_of_instead_of_a_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let manifest = parse(concat!(
        "probes:\n  recipe:\n    run: printf '{\"b\":1,\"a\":2}'\n    json: '.'\n",
        "commands:\n  - name: sh\n    inputs: [recipe]\n",
    ));
    assert_eq!(
        digest_of(&dir, &manifest, "recipe"),
        crate::hashing::hash_bytes(b"{\"a\":2,\"b\":1}\n"),
        "stdout is parsed and re-rendered canonically, so the tool's key order \
         stops being an input without a `jq -S` in the pipeline"
    );
}

#[test]
fn a_missing_or_malformed_file_names_the_probe_and_the_path() {
    let dir = tempfile::tempdir().expect("tempdir");
    let manifest = parse(&file_manifest(".a"));
    let err = Resolver::new(&manifest, dir.path())
        .for_rule(rule(&manifest))
        .expect_err("a file that is not there is refused");
    let message = err.to_string();
    assert!(message.contains("probe `pick`"), "names it: {message}");
    assert!(message.contains("data.json"), "names the path: {message}");

    let (bad_dir, bad) = json_project("not json at all", &file_manifest(".a"));
    let err = Resolver::new(&bad, bad_dir.path())
        .for_rule(rule(&bad))
        .expect_err("a file that does not parse is refused");
    let message = err.to_string();
    assert!(message.contains("probe `pick`"), "names it: {message}");
    assert!(
        message.contains("not one JSON value"),
        "says what is wrong with it: {message}"
    );
}

#[test]
fn a_json_program_that_cannot_run_names_the_probe_and_quotes_the_program() {
    let (dir, manifest) = json_project(r#"{"a": 1}"#, &file_manifest(".a | nosuchfilter"));
    let err = Resolver::new(&manifest, dir.path())
        .for_rule(rule(&manifest))
        .expect_err("an undefined filter is refused");
    let message = err.to_string();
    assert!(message.contains("probe `pick`"), "names it: {message}");
    assert!(
        message.contains("nosuchfilter"),
        "quotes the program: {message}"
    );

    let (typed_dir, typed) = json_project(r#"{"a": 1}"#, &file_manifest(".a.b"));
    assert!(
        Resolver::new(&typed, typed_dir.path())
            .for_rule(rule(&typed))
            .is_err(),
        "a program that raises against this document is the same hard stop"
    );
}

#[test]
fn a_probes_source_keys_must_describe_exactly_one_readable_thing() {
    let both = parse("probes:\n  x:\n    run: cat f\n    file: f\n    json: '.'\n");
    let message = validate(&both.probes, &both.scopes, &both.probe_shell)
        .expect_err("both sources is refused")
        .to_string();
    assert!(message.contains("probe `x`"), "names it: {message}");
    assert!(
        message.contains("exactly one source"),
        "says the rule rather than picking a winner: {message}"
    );

    let neither = parse("probes:\n  x:\n    json: '.'\n");
    let message = validate(&neither.probes, &neither.scopes, &neither.probe_shell)
        .expect_err("no source is refused")
        .to_string();
    assert!(
        message.contains("needs a source of bytes"),
        "says what is missing: {message}"
    );

    let bare = parse("probes:\n  x:\n    file: flake.lock\n");
    let message = validate(&bare.probes, &bare.scopes, &bare.probe_shell)
        .expect_err("a whole-file probe is refused")
        .to_string();
    assert!(
        message.contains("`scopes:`"),
        "points at the key that already does this: {message}"
    );

    for good in [
        "probes:\n  x:\n    run: cat f\n",
        "probes:\n  x:\n    run: cat f\n    json: '.'\n",
        "probes:\n  x:\n    file: f\n    json: '.'\n",
    ] {
        let manifest = parse(good);
        validate(&manifest.probes, &manifest.scopes, &manifest.probe_shell)
            .unwrap_or_else(|err| panic!("`{good}` is a legal shape: {err}"));
    }
}
