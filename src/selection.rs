//! Refusing — and explaining — a rule selection that resolved to nothing.
//!
//! Both `mmz --is-fresh` and `mmz --status` start by choosing which rules the
//! invocation is about: every rule, or the ones a `--tag` filter keeps. That
//! choice can come out empty three ways — the manifest declares no rules at
//! all, no rule carries the filter's tags, or every kept rule fans over a
//! scope that resolved to no files — and the two actions want opposite things
//! from an empty one.
//!
//! A gate cannot survive it. `--is-fresh` asserts that work is done, and an
//! assertion over an empty set is vacuously true: a typo'd tag would exit 0
//! and read as a passing build. So [`ensure_gateable`] refuses it, in the
//! company of every other selector in mmz that resolves to nothing and says so
//! ([`Error::NoMatch`], [`Error::NoInputs`], [`Error::ProbeEmpty`]).
//!
//! A report survives it fine. `--status` claims nothing about the rules it
//! prints, so an empty report is honest — as long as the line explaining it is
//! true, which is what [`empty_note`] is for.

use std::path::Path;

use crate::error::{Error, Result};
use crate::manifest::Manifest;

/// Refuses a gate whose selection expanded to no cache identity at all.
///
/// `kept` names the rules that passed the filter and `expansions` counts what
/// they fanned into, because a kept rule is not yet something to gate: a
/// parametric rule over an empty scope keeps itself and expands to nothing.
///
/// # Errors
///
/// Returns [`Error::NoRules`] when the manifest declares no commands,
/// [`Error::NoTaggedRules`] when the `--tag` filter kept none of them, or
/// [`Error::NoExpansions`] when every kept rule fanned over an empty scope.
pub(crate) fn ensure_gateable(
    manifest: &Manifest,
    manifest_path: &Path,
    tags: &[String],
    kept: &[&str],
    expansions: usize,
) -> Result<()> {
    if expansions > 0 {
        return Ok(());
    }
    if manifest.commands.is_empty() {
        return Err(Error::NoRules {
            path: manifest_path.to_path_buf(),
        });
    }
    if kept.is_empty() {
        return Err(Error::NoTaggedRules {
            tags: tag_phrase(tags),
            declared: declared_tags(manifest),
        });
    }
    Err(Error::NoExpansions {
        rules: quoted(kept),
    })
}

/// The line `mmz --status` prints instead of a table when its selection came
/// out empty, naming which of the three emptinesses it is.
///
/// The old line — "no rules defined in …" — was written for a manifest with no
/// `commands:` at all, years of features before a `--tag` filter could empty a
/// report over a manifest full of them. It is still exactly right for the case
/// it was written for, and kept verbatim for it; the other two get their own.
pub(crate) fn empty_note(
    manifest: &Manifest,
    manifest_path: &str,
    tags: &[String],
    kept: &[&str],
) -> String {
    if manifest.commands.is_empty() {
        return format!("no rules defined in {manifest_path}");
    }
    if kept.is_empty() {
        return format!(
            "no rule in {manifest_path} carries {}; {}",
            tag_phrase(tags),
            declared_tags(manifest)
        );
    }
    format!(
        "every rule this selection kept ({}) fans over a scope that resolved to no files",
        quoted(kept)
    )
}

/// The `--tag` filter as a phrase: one tag reads ``tag `gate` ``, several read
/// the AND the filter actually applies rather than a bare list.
fn tag_phrase(tags: &[String]) -> String {
    match tags {
        [] => "any tag".to_owned(),
        [one] => format!("tag `{one}`"),
        many => format!("every tag of {}", quoted(many)),
    }
}

/// Every tag the manifest declares, deduplicated and sorted — the list a
/// misspelled tag is visible against, printed where the mistake was made
/// rather than left for a second command to find.
fn declared_tags(manifest: &Manifest) -> String {
    let mut declared: Vec<&str> = manifest
        .commands
        .iter()
        .flat_map(|rule| rule.tags.iter().map(String::as_str))
        .collect();
    declared.sort_unstable();
    declared.dedup();
    if declared.is_empty() {
        return "no rule in the manifest declares any tags".to_owned();
    }
    format!("the manifest declares {}", quoted(&declared))
}

/// Renders names as a comma-separated list of backticked items.
fn quoted<S: AsRef<str>>(names: &[S]) -> String {
    names
        .iter()
        .map(|name| format!("`{}`", name.as_ref()))
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{empty_note, ensure_gateable};
    use crate::error::Error;
    use crate::manifest::Manifest;

    fn manifest(body: &str) -> Manifest {
        serde_yaml_ng::from_str(body).expect("parse manifest")
    }

    fn tagged() -> Manifest {
        manifest(
            "scopes:\n  a: [\"a.txt\"]\ncommands:\n  - name: sh\n    inputs: [a]\n    tags: [gate]\n  - name: env\n    inputs: [a]\n    tags: [bench]\n",
        )
    }

    #[test]
    fn a_populated_selection_passes() {
        assert!(ensure_gateable(&tagged(), Path::new("/x"), &[], &["sh", "env"], 2).is_ok());
    }

    #[test]
    fn an_unmatched_tag_names_the_tag_and_the_ones_that_exist() {
        let err = ensure_gateable(
            &tagged(),
            Path::new("/x/.mmz/config.yaml"),
            &["gats".to_owned()],
            &[],
            0,
        )
        .expect_err("an empty tag selection is refused");
        assert!(matches!(err, Error::NoTaggedRules { .. }));
        let text = err.to_string();
        assert!(text.contains("tag `gats`"), "names the filter: {text}");
        assert!(
            text.contains("`bench`, `gate`"),
            "lists the declared tags, sorted: {text}"
        );
    }

    #[test]
    fn several_tags_read_as_the_and_they_are() {
        let err = ensure_gateable(
            &tagged(),
            Path::new("/x"),
            &["gate".to_owned(), "bench".to_owned()],
            &[],
            0,
        )
        .expect_err("refused");
        let text = err.to_string();
        assert!(
            text.contains("every tag of `gate`, `bench`"),
            "the AND is spelled out: {text}"
        );
    }

    #[test]
    fn an_untagged_manifest_says_so_rather_than_listing_nothing() {
        let bare =
            manifest("scopes:\n  a: [\"a.txt\"]\ncommands:\n  - name: sh\n    inputs: [a]\n");
        let err = ensure_gateable(&bare, Path::new("/x"), &["gate".to_owned()], &[], 0)
            .expect_err("refused");
        assert!(
            err.to_string().contains("declares any tags"),
            "an empty list would read as a rendering bug: {err}"
        );
    }

    #[test]
    fn a_manifest_with_no_rules_is_refused_by_its_path() {
        let empty = manifest("scopes: {}\n");
        let err = ensure_gateable(&empty, Path::new("/x/.mmz/config.yaml"), &[], &[], 0)
            .expect_err("refused");
        assert!(matches!(err, Error::NoRules { .. }));
        assert!(
            err.to_string().contains("/x/.mmz/config.yaml"),
            "names the manifest: {err}"
        );
    }

    #[test]
    fn kept_rules_that_fan_to_nothing_are_refused_by_name() {
        let err =
            ensure_gateable(&tagged(), Path::new("/x"), &[], &["sh"], 0).expect_err("refused");
        assert!(matches!(err, Error::NoExpansions { .. }));
        assert!(err.to_string().contains("`sh`"), "names the rule: {err}");
    }

    #[test]
    fn the_report_line_stops_claiming_no_rules_when_a_filter_emptied_it() {
        let filtered = empty_note(&tagged(), "/x/.mmz/config.yaml", &["gats".to_owned()], &[]);
        assert!(
            !filtered.contains("no rules defined"),
            "rules are defined; none carries the tag: {filtered}"
        );
        assert!(filtered.contains("tag `gats`"), "{filtered}");
        assert!(filtered.contains("`bench`, `gate`"), "{filtered}");

        let none = manifest("scopes: {}\n");
        assert_eq!(
            empty_note(&none, "/x/.mmz/config.yaml", &[], &[]),
            "no rules defined in /x/.mmz/config.yaml",
            "the case the old line was written for keeps it"
        );
    }
}
