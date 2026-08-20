//! Unit tests for [`crate::probe`], ejected to keep `probe.rs` under the
//! file-size limit.

use std::collections::BTreeMap;
use std::path::Path;

use super::{Probe, Resolver, first_changed, validate};
use crate::manifest::Manifest;

/// Parses a manifest body, skipping validation so a test can build a shape
/// `validate` would reject on purpose.
fn parse(body: &str) -> Manifest {
    serde_yaml_ng::from_str(body).expect("manifest parses")
}

/// The rule named `name` in `manifest`.
fn rule<'a>(manifest: &'a Manifest, name: &str) -> &'a crate::manifest::Command {
    manifest
        .commands
        .iter()
        .find(|rule| rule.name == name)
        .expect("rule declared")
}

fn probes(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(name, hash)| ((*name).to_owned(), (*hash).to_owned()))
        .collect()
}

#[test]
fn a_probe_hashes_its_stdout() {
    let dir = tempfile::tempdir().expect("tempdir");
    let manifest = parse(
        "probes:\n  fingerprint:\n    run: printf hello\ncommands:\n  - name: sh\n    inputs: [fingerprint]\n",
    );
    let hashed = Resolver::new(&manifest, dir.path())
        .for_rule(rule(&manifest, "sh"))
        .expect("probe resolves");
    assert_eq!(
        hashed.get("fingerprint").map(String::as_str),
        Some(crate::hashing::hash_bytes(b"hello").as_str()),
        "the digest is the blake3 of the bytes the command printed"
    );
}

#[test]
fn a_probe_runs_from_the_project_root() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("pinned.txt"), b"root").expect("write");
    let manifest = parse(
        "probes:\n  here:\n    run: cat pinned.txt\ncommands:\n  - name: sh\n    inputs: [here]\n",
    );
    let hashed = Resolver::new(&manifest, dir.path())
        .for_rule(rule(&manifest, "sh"))
        .expect("probe resolves");
    assert_eq!(
        hashed.get("here").map(String::as_str),
        Some(crate::hashing::hash_bytes(b"root").as_str()),
        "a relative path in a probe resolves against the project root, as globs do"
    );
}

#[test]
fn a_failing_probe_is_a_hard_error_naming_code_and_stderr() {
    let dir = tempfile::tempdir().expect("tempdir");
    let manifest = parse(concat!(
        "probes:\n  broken:\n    run: >-\n      printf usable; printf 'bad shape' >&2; exit 3\n",
        "commands:\n  - name: sh\n    inputs: [broken]\n",
    ));
    let err = Resolver::new(&manifest, dir.path())
        .for_rule(rule(&manifest, "sh"))
        .expect_err("a non-zero probe never reaches the hasher");
    let message = err.to_string();
    assert!(message.contains("probe `broken`"), "names it: {message}");
    assert!(message.contains("exit 3"), "names the code: {message}");
    assert!(message.contains("bad shape"), "quotes stderr: {message}");
    assert!(
        message.contains("wrote no cache record"),
        "spells out the consequence: {message}"
    );
}

#[test]
fn a_probe_that_cannot_be_spawned_is_the_same_error() {
    let manifest =
        parse("probes:\n  gone:\n    run: printf x\ncommands:\n  - name: sh\n    inputs: [gone]\n");
    // A working directory that does not exist fails the spawn itself, which is
    // the one failure `sh -c` cannot turn into an exit code.
    let err = Resolver::new(&manifest, Path::new("/nonexistent-mmz-probe-root"))
        .for_rule(rule(&manifest, "sh"))
        .expect_err("an unspawnable probe is refused");
    let message = err.to_string();
    assert!(message.contains("probe `gone`"), "names it: {message}");
    assert!(
        message.contains("wrote no cache record"),
        "same hard stop as a failing probe: {message}"
    );
}

#[test]
fn empty_output_errors_unless_opted_in() {
    let dir = tempfile::tempdir().expect("tempdir");
    let manifest = parse(concat!(
        "probes:\n  silent:\n    run: 'true'\n",
        "  blank:\n    run: printf '\\n'\n",
        "  opted:\n    run: 'true'\n    allow_empty: true\n",
        "commands:\n  - name: a\n    inputs: [silent]\n",
        "  - name: b\n    inputs: [blank]\n",
        "  - name: c\n    inputs: [opted]\n",
    ));
    let err = Resolver::new(&manifest, dir.path())
        .for_rule(rule(&manifest, "a"))
        .expect_err("empty stdout is refused by default");
    assert!(
        err.to_string().contains("matched nothing"),
        "the message points at the likely cause: {err}"
    );
    assert!(
        Resolver::new(&manifest, dir.path())
            .for_rule(rule(&manifest, "b"))
            .is_err(),
        "whitespace-only output is empty too — the same matched-nothing shape"
    );
    let opted = Resolver::new(&manifest, dir.path())
        .for_rule(rule(&manifest, "c"))
        .expect("allow_empty opts in");
    assert_eq!(
        opted.get("opted").map(String::as_str),
        Some(crate::hashing::hash_bytes(b"").as_str()),
        "the opted-in probe still contributes a stable digest"
    );
}

#[test]
fn a_probe_shared_by_two_rules_runs_once() {
    let dir = tempfile::tempdir().expect("tempdir");
    let log = dir.path().join("runs.log");
    let manifest = parse(&format!(
        "probes:\n  shared:\n    run: printf x >> {}; printf stable\ncommands:\n  - name: a\n    inputs: [shared]\n  - name: b\n    inputs: [shared]\n",
        log.display()
    ));
    let mut resolver = Resolver::new(&manifest, dir.path());
    let first = resolver.for_rule(rule(&manifest, "a")).expect("first rule");
    let second = resolver
        .for_rule(rule(&manifest, "b"))
        .expect("second rule");

    assert_eq!(first, second, "both rules see the same digest");
    assert_eq!(
        std::fs::read(&log).map_or(0, |bytes| bytes.len()),
        1,
        "the shared probe was executed once, not once per referencing rule"
    );
    assert_eq!(
        resolver.resolved().len(),
        1,
        "the memo holds one entry for the one probe both rules name"
    );
}

#[test]
fn a_scope_name_in_inputs_is_not_a_probe() {
    let dir = tempfile::tempdir().expect("tempdir");
    let manifest = parse(concat!(
        "scopes:\n  src: [\"*.rs\"]\n",
        "probes:\n  tool:\n    run: printf v1\n",
        "commands:\n  - name: sh\n    inputs: [src, tool]\n",
    ));
    let hashed = Resolver::new(&manifest, dir.path())
        .for_rule(rule(&manifest, "sh"))
        .expect("resolves");
    assert_eq!(
        hashed.keys().collect::<Vec<_>>(),
        vec!["tool"],
        "only the probe half of `inputs` lands here; the scope is the glob walk's"
    );
}

#[test]
fn a_probe_shadowing_a_scope_is_rejected() {
    let manifest = parse("scopes:\n  rust: [\"*.rs\"]\nprobes:\n  rust:\n    run: printf x\n");
    let err = validate(&manifest.probes, &manifest.scopes, &manifest.probe_shell)
        .expect_err("collision refused");
    let message = err.to_string();
    assert!(message.contains("`rust`"), "names the clash: {message}");
    assert!(
        message.contains("one namespace"),
        "says why it cannot stand: {message}"
    );

    let clean = parse("scopes:\n  rust: [\"*.rs\"]\nprobes:\n  tool:\n    run: printf x\n");
    validate(&clean.probes, &clean.scopes, &clean.probe_shell).expect("distinct names are fine");
}

#[test]
fn an_unknown_probe_field_is_rejected() {
    let parsed: std::result::Result<Probe, _> =
        serde_yaml_ng::from_str("run: printf x\nallow_emty: true\n");
    assert!(
        parsed.is_err(),
        "a misspelled `allow_empty` must fail loudly, not silently disable the check"
    );
}

#[test]
fn first_changed_names_the_moved_probe_only() {
    let recorded = probes(&[("alpha", "a1"), ("beta", "b1")]);
    assert_eq!(
        first_changed(&recorded, &recorded),
        None,
        "unchanged probes name nobody"
    );
    assert_eq!(
        first_changed(&recorded, &probes(&[("alpha", "a1"), ("beta", "b2")])),
        Some("beta".to_owned()),
        "the moved one is named, not its unchanged sibling"
    );
    assert_eq!(
        first_changed(&recorded, &probes(&[("alpha", "a1"), ("gamma", "g1")])),
        Some("gamma".to_owned()),
        "a probe the record never saw counts as changed"
    );
    assert_eq!(
        first_changed(&BTreeMap::new(), &BTreeMap::new()),
        None,
        "a rule with no probes never blames one"
    );
}
