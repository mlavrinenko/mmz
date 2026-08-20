//! Unit tests for [`crate::json`] — the engine, tested at its own boundary.
//!
//! [`crate::probe`] owns the policy (what an empty selection means, which mmz
//! error a failure becomes) and is tested for it separately. What is left here
//! is the contract this module actually promises: bytes in, canonical bytes
//! out, and a [`Failure`] that says which of the three things was wrong.

use super::{Failure, select};

/// The selection, as a list of strings — the shape assertions read best in.
fn outputs(program: &str, input: &str) -> Vec<String> {
    select(program, input.as_bytes())
        .expect("selects")
        .into_iter()
        .map(|bytes| String::from_utf8(bytes).expect("rendered output is utf-8"))
        .collect()
}

#[test]
fn objects_render_with_their_keys_sorted_at_every_depth() {
    assert_eq!(
        outputs(".", r#"{"b": 1, "a": {"d": [2, 1], "c": 3}}"#),
        vec![r#"{"a":{"c":3,"d":[2,1]},"b":1}"#],
        "keys sort, arrays do not: one is presentation, the other is content"
    );
}

#[test]
fn a_program_may_yield_no_values_one_or_many() {
    assert!(outputs(".[] | select(. > 9)", "[1, 2]").is_empty());
    assert_eq!(outputs(".a", r#"{"a": "x"}"#), vec![r#""x""#]);
    assert_eq!(outputs(".a, .b", r#"{"a": 1, "b": 2}"#), vec!["1", "2"]);
}

#[test]
fn a_selector_that_names_nothing_yields_null_rather_than_nothing() {
    assert_eq!(
        outputs(".missing", "{}"),
        vec!["null"],
        "jq's own semantics, and exactly why the caller cannot treat \
         `outputs.is_empty()` as the whole matched-nothing test"
    );
}

#[test]
fn jq_syntax_this_repos_own_probes_already_use_still_works() {
    assert_eq!(
        outputs(
            r#"with_entries(select(.key | test("_PATHS$")))"#,
            r#"{"A_PATHS": 1, "B": 2}"#
        ),
        vec![r#"{"A_PATHS":1}"#],
        "the reason the key is jq and not a path syntax: these programs exist \
         already, so a narrower spelling would have to change meaning later"
    );
}

#[test]
fn each_kind_of_failure_is_reported_as_its_own_kind() {
    assert!(
        matches!(select(".a", b"not json"), Err(Failure::Input(_))),
        "bytes that are not one JSON value"
    );
    assert!(
        matches!(select(".a", b""), Err(Failure::Input(_))),
        "and no bytes at all"
    );
    assert!(
        matches!(select(".a |", b"{}"), Err(Failure::Program(_))),
        "a program that does not parse"
    );
    assert!(
        matches!(select("nosuchfilter", b"{}"), Err(Failure::Program(_))),
        "a program naming a filter jaq does not define"
    );
    assert!(
        matches!(select(".a", b"3"), Err(Failure::Run(_))),
        "a program that compiled and then raised against this document"
    );
}

#[test]
fn a_stream_of_values_is_refused_rather_than_folded() {
    assert!(
        matches!(select(".", b"{} {}"), Err(Failure::Input(_))),
        "mmz would have to invent a rule for how the filter maps over the \
         stream, so it declines to have one"
    );
}
