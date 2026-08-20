//! Unit tests for [`crate::compose`]'s merge: scopes, probes and commands
//! folding into one model, the duplicate-key and policy-key errors that guard
//! it, validation running once on the merged model rather than per file, the
//! provenance the merge records, and the byte-for-byte guarantee for a
//! manifest with no `imports:` key at all. Path resolution lives in
//! `compose_tests.rs`; cycle and diamond detection live in
//! `compose_cycles_tests.rs`.

use std::path::{Path, PathBuf};

use super::load;
use crate::error::Error;
use crate::manifest::Manifest;

/// Writes `body` to `relative` under `dir`, creating parent directories as
/// needed, and returns the written path.
fn write(dir: &Path, relative: &str, body: &str) -> PathBuf {
    let path = dir.join(relative);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("mkdir");
    }
    std::fs::write(&path, body).expect("write");
    path
}

/// The command names in `manifest`, in declaration order.
fn names(manifest: &Manifest) -> Vec<&str> {
    manifest.commands.iter().map(|c| c.name.as_str()).collect()
}

#[test]
fn fragment_scopes_probes_and_commands_reach_the_merged_model() {
    let dir = tempfile::tempdir().expect("tempdir");
    write(
        dir.path(),
        "frag.yaml",
        concat!(
            "scopes:\n  rust: [\"src/**/*.rs\"]\n",
            "probes:\n  toolchain:\n    run: \"echo pinned\"\n",
            "commands:\n  - name: cargo test\n    inputs: [rust, toolchain]\n",
        ),
    );
    let root = write(dir.path(), "root.yaml", "imports: [frag.yaml]\n");
    let (manifest, _) = load(&root, dir.path()).expect("loads");
    assert!(manifest.scopes.contains_key("rust"), "scope reached");
    assert!(manifest.probes.contains_key("toolchain"), "probe reached");
    assert_eq!(names(&manifest), vec!["cargo test"], "command reached");
}

#[test]
fn command_order_is_host_first_then_imports_depth_first() {
    let dir = tempfile::tempdir().expect("tempdir");
    write(dir.path(), "nested.yaml", "commands:\n  - name: nested\n");
    write(
        dir.path(),
        "a.yaml",
        "commands:\n  - name: a\nimports: [nested.yaml]\n",
    );
    write(dir.path(), "b.yaml", "commands:\n  - name: b\n");
    let root = write(
        dir.path(),
        "root.yaml",
        "commands:\n  - name: root\nimports: [a.yaml, b.yaml]\n",
    );
    let (manifest, _) = load(&root, dir.path()).expect("loads");
    assert_eq!(
        names(&manifest),
        vec!["root", "a", "nested", "b"],
        "host first, imports in listed order, nested import depth-first"
    );
}

#[test]
fn duplicate_scope_across_files_names_both() {
    let dir = tempfile::tempdir().expect("tempdir");
    write(dir.path(), "frag.yaml", "scopes:\n  rust: [\"a/**\"]\n");
    let root = write(
        dir.path(),
        "root.yaml",
        "scopes:\n  rust: [\"b/**\"]\nimports: [frag.yaml]\n",
    );
    let err = load(&root, dir.path()).expect_err("duplicate scope across files");
    let text = err.to_string();
    assert!(text.contains("root.yaml"), "names root: {text}");
    assert!(text.contains("frag.yaml"), "names fragment: {text}");
    assert!(text.contains("rust"), "names the key: {text}");
    assert!(matches!(err, Error::DuplicateScope { .. }));
}

#[test]
fn duplicate_probe_across_files_names_both() {
    let dir = tempfile::tempdir().expect("tempdir");
    write(
        dir.path(),
        "frag.yaml",
        "probes:\n  toolchain:\n    run: \"echo a\"\n",
    );
    let root = write(
        dir.path(),
        "root.yaml",
        "probes:\n  toolchain:\n    run: \"echo b\"\nimports: [frag.yaml]\n",
    );
    let err = load(&root, dir.path()).expect_err("duplicate probe across files");
    let text = err.to_string();
    assert!(text.contains("root.yaml"), "names root: {text}");
    assert!(text.contains("frag.yaml"), "names fragment: {text}");
    assert!(matches!(err, Error::DuplicateProbe { .. }));
}

#[test]
fn duplicate_command_across_files_names_both() {
    let dir = tempfile::tempdir().expect("tempdir");
    write(dir.path(), "frag.yaml", "commands:\n  - name: cargo test\n");
    let root = write(
        dir.path(),
        "root.yaml",
        "commands:\n  - name: cargo test\nimports: [frag.yaml]\n",
    );
    let err = load(&root, dir.path()).expect_err("duplicate command across files");
    let text = err.to_string();
    assert!(text.contains("root.yaml"), "names root: {text}");
    assert!(text.contains("frag.yaml"), "names fragment: {text}");
    assert!(matches!(err, Error::DuplicateCommandAcrossFiles { .. }));
}

#[test]
fn a_policy_key_in_a_fragment_errors_naming_the_key_and_file() {
    let cases = [
        ("gitignore", "gitignore: false\n"),
        ("cache_dir", "cache_dir: .cache\n"),
        ("strict", "strict: []\n"),
        ("on_hit", "on_hit: note\n"),
    ];
    for (key, line) in cases {
        let dir = tempfile::tempdir().expect("tempdir");
        write(dir.path(), "frag.yaml", line);
        let root = write(dir.path(), "root.yaml", "imports: [frag.yaml]\n");
        let err = load(&root, dir.path()).expect_err("policy key in a fragment is rejected");
        let text = err.to_string();
        assert!(text.contains(key), "names the key `{key}`: {text}");
        assert!(text.contains("frag.yaml"), "names the fragment: {text}");
        assert!(matches!(err, Error::FragmentPolicyKey { .. }));
    }
}

#[test]
fn the_same_policy_keys_in_the_root_do_not_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = write(
        dir.path(),
        "root.yaml",
        "gitignore: false\ncache_dir: .cache\nstrict: []\non_hit: note\ncommands: []\n",
    );
    let (manifest, _) = load(&root, dir.path()).expect("the root may set every policy key");
    assert!(!manifest.gitignore);
    assert_eq!(manifest.cache_dir, ".cache");
    assert_eq!(manifest.on_hit.as_deref(), Some("note"));
}

/// A present-but-`null` policy key is still *setting* the key, not omitting
/// it: `Option<T>` alone cannot tell "absent" from "explicit null" apart, and
/// the fragment check must not let a bare `gitignore:` slip through where
/// `gitignore: false` would have been caught.
#[test]
fn an_explicit_null_policy_key_in_a_fragment_is_still_treated_as_set() {
    for key in ["gitignore", "cache_dir", "strict", "on_hit"] {
        let dir = tempfile::tempdir().expect("tempdir");
        write(dir.path(), "frag.yaml", &format!("{key}:\n"));
        let root = write(dir.path(), "root.yaml", "imports: [frag.yaml]\n");
        let err = load(&root, dir.path()).expect_err("an explicit null still sets the key");
        let text = err.to_string();
        assert!(text.contains(key), "names the key `{key}`: {text}");
        assert!(text.contains("frag.yaml"), "names the fragment: {text}");
        assert!(
            matches!(err, Error::FragmentPolicyKey { .. }),
            "for `{key}`: {err}"
        );
    }
}

/// `gitignore`, `cache_dir` and `strict` are not nullable manifest fields —
/// before composition, `null` on any of them was a hard parse error. Falling
/// through to the default instead would be a silent behaviour change: a
/// `gitignore:` written to mean `false` (the artifact-scope escape hatch)
/// must not quietly resolve to `true` and start filtering inputs nobody
/// hashed. The exact error variant is not the point here — only that the
/// manifest is rejected outright, never resolved to any particular value.
#[test]
fn an_explicit_null_gitignore_cache_dir_or_strict_in_the_root_is_rejected() {
    for key in ["gitignore", "cache_dir", "strict"] {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = write(dir.path(), "root.yaml", &format!("{key}:\ncommands: []\n"));
        load(&root, dir.path()).expect_err(&format!(
            "an explicit null `{key}` in the root must fail closed, not default silently"
        ));
    }
}

/// `on_hit` is the one policy key that was already `Option<String>` on
/// [`Manifest`] before composition existed, so `on_hit:` (null) in the root
/// has always meant "no `on_hit`" — exactly like omitting the key. Composition
/// must not regress this into an error just because the other three keys now
/// reject an explicit null.
#[test]
fn an_explicit_null_on_hit_in_the_root_stays_valid_and_means_none() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = write(dir.path(), "root.yaml", "on_hit:\ncommands: []\n");
    let (manifest, _) =
        load(&root, dir.path()).expect("on_hit: null has always been legal in the root");
    assert_eq!(manifest.on_hit, None);
}

#[test]
fn a_fragment_invalid_alone_but_valid_merged_is_accepted() {
    let dir = tempfile::tempdir().expect("tempdir");
    let uses = write(
        dir.path(),
        "uses.yaml",
        "commands:\n  - name: cargo test\n    inputs: [rust]\n",
    );
    write(
        dir.path(),
        "defines.yaml",
        "scopes:\n  rust: [\"src/**\"]\n",
    );
    let root = write(
        dir.path(),
        "root.yaml",
        "imports: [uses.yaml, defines.yaml]\n",
    );

    let alone: Manifest = serde_yaml_ng::from_str(&std::fs::read_to_string(&uses).expect("read"))
        .expect("parses alone");
    assert!(
        alone.validate().is_err(),
        "uses.yaml alone references an undeclared scope"
    );

    let (manifest, _) =
        load(&root, dir.path()).expect("valid once merged with the sibling that defines it");
    assert_eq!(names(&manifest), vec!["cargo test"]);
}

#[test]
fn a_merge_invalid_even_when_every_fragment_is_valid_alone_is_rejected() {
    let dir = tempfile::tempdir().expect("tempdir");
    write(
        dir.path(),
        "scope.yaml",
        "scopes:\n  shared: [\"src/**\"]\n",
    );
    write(
        dir.path(),
        "probe.yaml",
        "probes:\n  shared:\n    run: \"echo x\"\n",
    );
    let root = write(
        dir.path(),
        "root.yaml",
        "imports: [scope.yaml, probe.yaml]\n",
    );

    let scope_only: Manifest =
        serde_yaml_ng::from_str("scopes:\n  shared: [\"src/**\"]\n").expect("parses");
    scope_only.validate().expect("valid alone");
    let probe_only: Manifest =
        serde_yaml_ng::from_str("probes:\n  shared:\n    run: \"echo x\"\n").expect("parses");
    probe_only.validate().expect("valid alone");

    let err = load(&root, dir.path())
        .expect_err("a scope and a probe named `shared` collide once merged");
    assert!(matches!(err, Error::NameCollision { .. }));
}

#[test]
fn provenance_records_the_root_as_source_of_its_own_entries() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = write(
        dir.path(),
        "root.yaml",
        "scopes:\n  rust: [\"src/**\"]\ncommands: []\n",
    );
    let (_, provenance) = load(&root, dir.path()).expect("loads");
    assert_eq!(
        provenance.scopes.get("rust"),
        Some(&std::fs::canonicalize(&root).expect("canonicalize")),
        "a single-file project is the degenerate case, not a special case"
    );
}

#[test]
fn provenance_records_a_fragment_as_source_of_its_own_entries() {
    let dir = tempfile::tempdir().expect("tempdir");
    let fragment = write(dir.path(), "frag.yaml", "scopes:\n  rust: [\"src/**\"]\n");
    let root = write(dir.path(), "root.yaml", "imports: [frag.yaml]\n");
    let (_, provenance) = load(&root, dir.path()).expect("loads");
    assert_eq!(
        provenance.scopes.get("rust"),
        Some(&std::fs::canonicalize(&fragment).expect("canonicalize"))
    );
}

#[test]
fn no_imports_key_produces_the_same_model_as_a_direct_parse() {
    let body = concat!(
        "scopes:\n  rust: [\"src/**\"]\n",
        "probes:\n  toolchain:\n    run: \"echo hi\"\n",
        "commands:\n  - name: cargo test\n    inputs: [rust, toolchain]\n",
        "gitignore: false\n",
        "cache_dir: .cache/mmz\n",
        "on_hit: note\n",
    );
    let dir = tempfile::tempdir().expect("tempdir");
    let root = write(dir.path(), "root.yaml", body);
    let (composed, _) = load(&root, dir.path()).expect("loads");
    let direct: Manifest = serde_yaml_ng::from_str(body).expect("parses");
    direct.validate().expect("validates");

    assert_eq!(
        composed.scopes.keys().collect::<Vec<_>>(),
        direct.scopes.keys().collect::<Vec<_>>()
    );
    assert_eq!(
        composed.probes.keys().collect::<Vec<_>>(),
        direct.probes.keys().collect::<Vec<_>>()
    );
    assert_eq!(names(&composed), names(&direct));
    assert_eq!(composed.gitignore, direct.gitignore);
    assert_eq!(composed.cache_dir, direct.cache_dir);
    assert_eq!(composed.on_hit, direct.on_hit);
}

#[test]
fn no_imports_key_reproduces_every_existing_validation_error_message() {
    let cases = [
        "commands:\n  - name: \"  \"\n",
        "commands:\n  - name: sh\n  - name: sh\n",
        "commands:\n  - name: sh\n    inputs: [ghost]\n",
        "commands:\n  - name: sh\n    tags: [gate, gate]\n",
        "commands:\n  - name: sh\n    outputs: [\"target/*.info\"]\n",
    ];
    for body in cases {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = write(dir.path(), "root.yaml", body);
        let composed_err = load(&root, dir.path())
            .expect_err("still invalid once merged")
            .to_string();

        let direct: Manifest = serde_yaml_ng::from_str(body).expect("parses");
        let direct_err = direct
            .validate()
            .expect_err("still invalid directly")
            .to_string();

        assert_eq!(composed_err, direct_err, "identical message for: {body}");
    }
}

#[test]
fn no_imports_key_keeps_deny_unknown_fields_as_a_parse_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = write(dir.path(), "root.yaml", "commands: []\nbogus: 1\n");
    let err = load(&root, dir.path()).expect_err("unknown top-level field is rejected");
    assert!(matches!(err, Error::ManifestParse { .. }));
}
