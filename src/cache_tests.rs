use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use super::{Outcome, is_fresh, prune, read, slug, write as write_record};
use crate::clock::Clock;

/// Every record below is stamped from a pinned clock, so `ran_at` is a fact
/// the test states rather than whatever second the suite happened to run in.
const RAN_AT: u64 = 1_700_000_000;

/// Records a run declaring no outputs and naming no probes — every case
/// here but the two that check those lists.
fn write(dir: &Path, command: &str, digest: &str, ok: bool) {
    write_record(
        dir,
        command,
        Clock::pinned(RAN_AT),
        &Outcome {
            digest,
            ok,
            ..Outcome::default()
        },
    );
}

#[test]
fn fresh_only_for_matching_successful_record() {
    let dir = tempfile::tempdir().expect("tempdir");
    let base = dir.path();
    assert!(!is_fresh(base, "sh", "digest-a"), "no record yet");

    write(base, "sh", "digest-a", true);
    assert!(
        is_fresh(base, "sh", "digest-a"),
        "matching ok record is fresh"
    );
    assert!(!is_fresh(base, "sh", "digest-b"), "different digest misses");

    write(base, "sh", "digest-a", false);
    assert!(
        !is_fresh(base, "sh", "digest-a"),
        "failed record is never fresh"
    );
}

#[test]
fn read_exposes_record_fields_for_macros() {
    let dir = tempfile::tempdir().expect("tempdir");
    write(dir.path(), "cargo test", "d1", true);
    let cached = read(dir.path(), "cargo test").expect("record");
    assert_eq!(
        cached.fields.get("command").map(String::as_str),
        Some("cargo test"),
        "string field exposed"
    );
    assert_eq!(
        cached.fields.get("status").map(String::as_str),
        Some("ok"),
        "enum field exposed in its serialized spelling"
    );
    assert_eq!(
        cached.fields.get("input_digest").map(String::as_str),
        Some("d1")
    );
    assert!(
        cached.fields.contains_key("ran_at"),
        "numeric field exposed for macros"
    );
}

#[test]
fn declared_outputs_are_stored_with_the_record() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_record(
        dir.path(),
        "just cover",
        Clock::pinned(RAN_AT),
        &Outcome {
            digest: "d1",
            ok: true,
            outputs: &[PathBuf::from("target/coverage/lcov.info")],
            ..Outcome::default()
        },
    );
    let cached = read(dir.path(), "just cover").expect("record");
    assert_eq!(
        cached.outputs,
        vec!["target/coverage/lcov.info".to_owned()],
        "the record remembers what the run promised to produce"
    );

    write(dir.path(), "cargo test", "d2", true);
    let bare = read(dir.path(), "cargo test").expect("record");
    assert!(
        bare.outputs.is_empty(),
        "a rule declaring no outputs records none"
    );
}

#[test]
fn probe_digests_are_stored_with_the_record() {
    let dir = tempfile::tempdir().expect("tempdir");
    let probes: BTreeMap<String, String> = [("fmt-recipe".to_owned(), "abc".to_owned())]
        .into_iter()
        .collect();
    write_record(
        dir.path(),
        "just fmt-check",
        Clock::pinned(RAN_AT),
        &Outcome {
            digest: "d1",
            ok: true,
            probes: probes.clone(),
            ..Outcome::default()
        },
    );
    let cached = read(dir.path(), "just fmt-check").expect("record");
    assert_eq!(
        cached.probes, probes,
        "the record remembers what each probe printed, so a later stale verdict can name the one that moved"
    );

    write(dir.path(), "cargo test", "d2", true);
    let bare = read(dir.path(), "cargo test").expect("record");
    assert!(
        bare.probes.is_empty(),
        "a rule naming no probes records none"
    );
}

#[test]
fn distinct_commands_get_distinct_records() {
    let dir = tempfile::tempdir().expect("tempdir");
    let base = dir.path();
    write(base, "cargo test", "d1", true);
    write(base, "cargo build", "d2", true);
    assert!(is_fresh(base, "cargo test", "d1"));
    assert!(is_fresh(base, "cargo build", "d2"));
}

#[test]
fn read_surfaces_the_clock_it_was_stamped_from() {
    let dir = tempfile::tempdir().expect("tempdir");
    write(dir.path(), "sh", "d", true);
    let cached = read(dir.path(), "sh").expect("record");
    assert_eq!(
        cached.ran_at, RAN_AT,
        "ran_at is the clock the writer was handed, not one read here"
    );
}

#[test]
fn slug_is_readable_distinct_capped_and_never_empty() {
    assert!(slug("cargo test").starts_with("cargo-test-"));
    assert_ne!(slug("cargo test"), slug("cargo build"));
    assert!(
        slug("+++").starts_with("cmd-"),
        "all-symbol name gets a stem"
    );
    let long = "x".repeat(500);
    let stem = slug(&long);
    // 64-char stem + '-' + 16-char hash.
    assert_eq!(stem.len(), super::SLUG_MAX + 1 + 16, "stem is capped");
}

#[test]
fn write_is_atomic_and_leaves_no_temp_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let base = dir.path();
    write(base, "cargo test", "digest-a", true);

    let temps: Vec<_> = std::fs::read_dir(base)
        .expect("cache dir")
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "tmp"))
        .collect();
    assert!(temps.is_empty(), "rename leaves no .tmp behind");
    assert!(
        is_fresh(base, "cargo test", "digest-a"),
        "record is readable"
    );
}

#[test]
fn prune_drops_only_orphan_records() {
    let dir = tempfile::tempdir().expect("tempdir");
    let base = dir.path();
    write(base, "cargo test", "d1", true);
    write(base, "cargo build", "d2", true);

    let live: BTreeSet<String> = ["cargo test".to_owned()].into_iter().collect();
    let pruned = prune(base, &live).expect("prune");
    assert_eq!(pruned, vec!["cargo build".to_owned()], "orphan removed");
    assert!(is_fresh(base, "cargo test", "d1"), "live record kept");
    assert!(read(base, "cargo build").is_none(), "orphan record gone");
}

#[test]
fn prune_on_missing_dir_is_empty() {
    let dir = tempfile::tempdir().expect("tempdir");
    let pruned = prune(&dir.path().join("absent"), &BTreeSet::new()).expect("prune");
    assert!(pruned.is_empty(), "missing cache dir prunes nothing");
}
