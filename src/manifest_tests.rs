use super::{Manifest, MatchMode, StrictCase};

fn parse(text: &str) -> Manifest {
    serde_yaml_ng::from_str(text).expect("parse")
}

#[test]
fn cache_dir_defaults_and_overrides() {
    assert_eq!(parse("commands: []\n").cache_dir, ".mmz/cache");
    assert_eq!(
        parse("commands: []\ncache_dir: .cache/mmz\n").cache_dir,
        ".cache/mmz"
    );
}

#[test]
fn match_mode_defaults_to_prefix_and_parses_exact() {
    let manifest =
        parse("commands:\n  - name: cargo test\n  - name: cargo build\n    match: exact\n");
    let prefix = manifest.commands.first().expect("first rule");
    let exact = manifest.commands.get(1).expect("second rule");
    assert_eq!(prefix.match_mode, MatchMode::Prefix, "default");
    assert_eq!(exact.match_mode, MatchMode::Exact, "explicit");
}

#[test]
fn strict_defaults_to_all_cases() {
    let manifest = parse("commands: []\n");
    assert!(manifest.strict.enforces(StrictCase::NoMatch));
    assert!(manifest.strict.enforces(StrictCase::NoInputs));
}

#[test]
fn strict_list_selects_a_subset() {
    let manifest = parse("commands: []\nstrict: [no_match]\n");
    assert!(manifest.strict.enforces(StrictCase::NoMatch));
    assert!(
        !manifest.strict.enforces(StrictCase::NoInputs),
        "unlisted case relaxed"
    );

    let none = parse("commands: []\nstrict: []\n");
    assert!(!none.strict.enforces(StrictCase::NoMatch));
    assert!(!none.strict.enforces(StrictCase::NoInputs));
}

#[test]
fn strict_rejects_unknown_case() {
    let parsed: Result<Manifest, _> = serde_yaml_ng::from_str("strict: [bogus]\n");
    assert!(parsed.is_err(), "unknown strict case is rejected");
}

#[test]
fn on_hit_parses_global_and_per_command_and_defaults_none() {
    let manifest = parse(
        "on_hit: \"global note\"\ncommands:\n  - name: cargo test\n    on_hit: \"rule note\"\n  - name: cargo build\n",
    );
    assert_eq!(manifest.on_hit.as_deref(), Some("global note"));
    let overridden = manifest.commands.first().expect("first rule");
    let inherits = manifest.commands.get(1).expect("second rule");
    assert_eq!(
        overridden.on_hit.as_deref(),
        Some("rule note"),
        "per-command override"
    );
    assert_eq!(inherits.on_hit, None, "absent per-command on_hit is None");
    assert_eq!(
        parse("commands: []\n").on_hit,
        None,
        "absent global on_hit is None"
    );
}

#[test]
fn parses_scopes_and_commands() {
    let manifest = parse(
        "scopes:\n  rust: [\"**/*.rs\"]\ncommands:\n  - name: cargo test\n    inputs: [rust]\n",
    );
    assert_eq!(manifest.commands.len(), 1);
    let command = manifest.commands.first().expect("command");
    assert_eq!(command.name, "cargo test");
    assert_eq!(
        manifest.globs_for(command).expect("globs"),
        vec!["**/*.rs".to_owned()]
    );
    assert!(manifest.gitignore, "gitignore defaults on");
}

#[test]
fn rejects_unknown_fields() {
    let parsed: Result<Manifest, _> =
        serde_yaml_ng::from_str("scopes: {}\ncommands: []\nbogus: 1\n");
    assert!(parsed.is_err(), "unknown top-level fields are rejected");
}

#[test]
fn validate_rejects_blank_and_duplicate_names() {
    let blank = parse("commands:\n  - name: \"  \"\n");
    assert!(blank.validate().is_err(), "blank name rejected");

    let dup = parse("commands:\n  - name: sh\n  - name: sh\n");
    assert!(dup.validate().is_err(), "duplicate name rejected");
}

#[test]
fn validate_rejects_unknown_scope() {
    let manifest = parse("commands:\n  - name: sh\n    inputs: [ghost]\n");
    assert!(manifest.validate().is_err(), "missing scope rejected");
}

#[test]
fn load_validates_from_disk() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("config.yaml");
    std::fs::write(&path, "commands:\n  - name: sh\n    inputs: [ghost]\n").expect("write");
    assert!(Manifest::load(&path).is_err(), "load runs validation");
}

fn write_config(root: &std::path::Path, body: &str) -> std::path::PathBuf {
    let dir = root.join(".mmz");
    std::fs::create_dir_all(&dir).expect("mkdir .mmz");
    let path = dir.join("config.yaml");
    std::fs::write(&path, body).expect("write config");
    path
}

#[test]
fn discovers_walking_upwards() {
    let dir = tempfile::tempdir().expect("tempdir");
    let nested = dir.path().join("a/b");
    std::fs::create_dir_all(&nested).expect("mkdir");
    let path = write_config(dir.path(), "commands: []\n");
    assert_eq!(Manifest::discover(&nested), Some(path));
}

#[test]
fn locate_roots_at_the_parent_of_dot_mmz() {
    let dir = tempfile::tempdir().expect("tempdir");
    let nested = dir.path().join("a/b");
    std::fs::create_dir_all(&nested).expect("mkdir");
    write_config(dir.path(), "commands: []\n");
    let located = Manifest::locate(&nested).expect("locate");
    assert_eq!(located.root, dir.path(), "root is the parent of .mmz");
    assert_eq!(
        located.root.join(&located.manifest.cache_dir),
        dir.path().join(".mmz/cache"),
        "cache_dir resolves under the project root, not inside .mmz",
    );
}

#[test]
fn tags_are_trimmed_and_empties_dropped() {
    let manifest =
        parse("commands:\n  - name: cargo test\n    tags: [\" gate \", \"\", \"  \", \"Bench\"]\n");
    let command = manifest.commands.first().expect("command");
    assert_eq!(
        command.tags,
        vec!["gate".to_owned(), "Bench".to_owned()],
        "trimmed, blanks dropped, case preserved"
    );
}

#[test]
fn tags_default_to_empty() {
    let manifest = parse("commands:\n  - name: cargo test\n");
    let command = manifest.commands.first().expect("command");
    assert!(command.tags.is_empty(), "no tags: field means no tags");
}

#[test]
fn validate_rejects_duplicate_tags_on_one_command() {
    let manifest = parse("commands:\n  - name: sh\n    tags: [gate, gate]\n");
    assert!(
        manifest.validate().is_err(),
        "the same tag twice on one rule is rejected"
    );
}

#[test]
fn validate_allows_the_same_tag_on_different_commands() {
    let manifest =
        parse("commands:\n  - name: sh\n    tags: [gate]\n  - name: cat\n    tags: [gate]\n");
    assert!(
        manifest.validate().is_ok(),
        "the same tag on two different rules is fine"
    );
}

#[test]
fn rejects_unknown_fields_still_holds_with_tags_present() {
    let parsed: Result<Manifest, _> =
        serde_yaml_ng::from_str("commands:\n  - name: sh\n    tags: [gate]\n    bogus: 1\n");
    assert!(
        parsed.is_err(),
        "deny_unknown_fields still rejects a stray field alongside tags"
    );
}
