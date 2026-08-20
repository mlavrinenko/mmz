use std::collections::BTreeMap;

use super::{humanize_age, render_text};
use crate::clock::Clock;
use crate::status::{CachedInfo, Report, RuleStatus, State};

/// A `RuleStatus` with only the fields these tests care about; every other
/// field takes the value an ordinary resolved-but-never-run rule would have.
fn rule(name: &str, source: &str) -> RuleStatus {
    RuleStatus {
        name: name.to_owned(),
        source: source.to_owned(),
        state: State::Never,
        digest: None,
        missing_output: None,
        cached: None,
        inputs: Vec::new(),
    }
}

fn report(rules: Vec<RuleStatus>) -> Report {
    Report {
        manifest: ".mmz/config.yaml".to_owned(),
        now: Clock::pinned(0),
        empty_note: String::new(),
        probes: BTreeMap::new(),
        rules,
    }
}

#[test]
fn humanize_age_scales_by_unit() {
    assert_eq!(humanize_age(5), "5s ago");
    assert_eq!(humanize_age(90), "1m ago");
    assert_eq!(humanize_age(3 * 3600), "3h ago");
    assert_eq!(humanize_age(2 * 86_400), "2d ago");
}

/// The `AGE` column reads the clock the report resolved, not one the renderer
/// looks up for itself — which is what lets `MMZ_NOW` produce a genuinely aged
/// row instead of the `0s ago` a just-recorded run always shows.
#[test]
fn age_is_measured_against_the_reports_own_clock() {
    const RAN_AT: u64 = 1_700_000_000;
    let report = Report {
        manifest: ".mmz/config.yaml".to_owned(),
        now: Clock::pinned(RAN_AT + 2 * 3600),
        empty_note: String::new(),
        probes: BTreeMap::new(),
        rules: vec![RuleStatus {
            name: "just check".to_owned(),
            source: ".mmz/config.yaml".to_owned(),
            state: State::Fresh,
            digest: Some("d1".to_owned()),
            missing_output: None,
            cached: Some(CachedInfo {
                digest: "d1".to_owned(),
                ok: true,
                ran_at: RAN_AT,
                outputs: Vec::new(),
                probes: BTreeMap::new(),
            }),
            inputs: Vec::new(),
        }],
    };
    assert!(
        render_text(&report).contains("2h ago"),
        "a record two hours older than the report's clock reads as two hours old"
    );
}

/// Pinned against real bytes captured from the binary built just before the
/// `SOURCE` column existed (`mmz sh -c "exit 0"` with `MMZ_NOW=1700000000`,
/// then `mmz --status` with `MMZ_NOW=1700007200`) — not a hand-derived
/// literal, so a padding mistake in the split can't hide behind arithmetic
/// that happens to agree with itself.
#[test]
fn single_source_table_is_byte_identical_to_before_the_source_column() {
    const RAN_AT: u64 = 1_700_000_000;
    let report = Report {
        manifest: ".mmz/config.yaml".to_owned(),
        now: Clock::pinned(RAN_AT + 2 * 3600),
        empty_note: String::new(),
        probes: BTreeMap::new(),
        rules: vec![RuleStatus {
            name: "sh".to_owned(),
            source: ".mmz/config.yaml".to_owned(),
            state: State::Fresh,
            digest: Some("d1".to_owned()),
            missing_output: None,
            cached: Some(CachedInfo {
                digest: "d1".to_owned(),
                ok: true,
                ran_at: RAN_AT,
                outputs: Vec::new(),
                probes: BTreeMap::new(),
            }),
            inputs: Vec::new(),
        }],
    };
    assert_eq!(
        render_text(&report),
        "RULE  STATE  AGE\nsh    fresh  2h ago\n"
    );
}

#[test]
fn source_column_appears_only_when_rules_declare_more_than_one_file() {
    let one_file = report(vec![
        rule("a", ".mmz/config.yaml"),
        rule("b", ".mmz/config.yaml"),
    ]);
    let text = render_text(&one_file);
    assert!(
        !text.contains("SOURCE"),
        "two rules sharing one file still cost nothing: {text}"
    );

    let two_files = report(vec![
        rule("a", ".mmz/config.yaml"),
        rule("b", ".mmz/conf.d/frag.yaml"),
    ]);
    let text = render_text(&two_files);
    assert!(text.contains("SOURCE"), "two distinct sources: {text}");
    assert!(
        text.contains(".mmz/config.yaml") && text.contains(".mmz/conf.d/frag.yaml"),
        "each row names its own source: {text}"
    );
}
