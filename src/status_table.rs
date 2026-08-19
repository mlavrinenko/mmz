//! Renders `mmz --status`'s human table: `RULE / [SOURCE] / STATE / AGE /
//! [MISSING OUTPUT]`. Split out of [`crate::status`] once a `SOURCE` column
//! needed a home — that module owns the report model (resolving every rule's
//! inputs and verdict); this one turns the resolved model into aligned text.
//!
//! Two columns are conditional, appearing only when a report actually needs
//! them, so an ordinary single-source report with nothing voided reads
//! exactly as it always has: SOURCE only when more than one file contributed
//! rules (a project with no `imports:` never pays for it — see
//! [`render_text`]), and MISSING OUTPUT only when some rule's record was
//! voided by a gone artifact.

use std::collections::BTreeSet;

use super::Report;

/// Renders the aligned status table. AGE is the time since the rule's record
/// was written, measured against the report's own resolved clock (so
/// `MMZ_NOW` pins it), and blank when the rule has no record.
///
/// SOURCE sits right after RULE — the file that declared a rule is identity,
/// the same kind of fact RULE itself is, not a detail about the verdict — and
/// it appears only when the report's rules name more than one file. A project
/// with no `imports:` has exactly one source for every rule, so the column
/// never appears and the table is byte-for-byte what it was before this
/// column existed. MISSING OUTPUT stays the trailing column, naming the gone
/// artifact, and appears only when some rule's record was voided by one.
pub(super) fn render_text(report: &Report) -> String {
    let now = report.now.now_secs();
    let ages: Vec<String> = report
        .rules
        .iter()
        .map(|rule| {
            rule.cached.as_ref().map_or_else(String::new, |record| {
                humanize_age(now.saturating_sub(record.ran_at))
            })
        })
        .collect();
    let voided = report
        .rules
        .iter()
        .any(|rule| rule.missing_output.is_some());
    let multi_source = report
        .rules
        .iter()
        .map(|rule| rule.source.as_str())
        .collect::<BTreeSet<_>>()
        .len()
        > 1;

    let rule_width = column_width(
        report.rules.iter().map(|rule| rule.name.chars().count()),
        "RULE",
    );
    let source_width = if multi_source {
        column_width(
            report.rules.iter().map(|rule| rule.source.chars().count()),
            "SOURCE",
        )
    } else {
        0
    };
    let state_width = column_width(
        report.rules.iter().map(|rule| rule.state.label().len()),
        "STATE",
    );
    let age_width = if voided {
        column_width(ages.iter().map(|age| age.chars().count()), "AGE")
    } else {
        0
    };

    let row = |rule: &str, source: &str, state: &str, age: &str, missing: &str| {
        let mut line = format!("{rule:<rule_width$}");
        if multi_source {
            line.push_str(&format!("  {source:<source_width$}"));
        }
        line.push_str(&format!(
            "  {state:<state_width$}  {age:<age_width$}  {missing}"
        ));
        format!("{}\n", line.trim_end())
    };
    let mut out = row(
        "RULE",
        "SOURCE",
        "STATE",
        "AGE",
        if voided { "MISSING OUTPUT" } else { "" },
    );
    for (rule, age) in report.rules.iter().zip(&ages) {
        out.push_str(&row(
            &rule.name,
            &rule.source,
            rule.state.label(),
            age,
            rule.missing_output.as_deref().unwrap_or(""),
        ));
    }
    out
}

/// The width of a table column: the widest cell, never narrower than `header`.
fn column_width(cells: impl Iterator<Item = usize>, header: &str) -> usize {
    cells.max().unwrap_or(0).max(header.len())
}

/// Renders a record's age as a coarse, human-readable span (`5s`, `3m`, `2h`,
/// `4d` ago).
fn humanize_age(secs: u64) -> String {
    const MINUTE: u64 = 60;
    const HOUR: u64 = 60 * MINUTE;
    const DAY: u64 = 24 * HOUR;
    if secs < MINUTE {
        format!("{secs}s ago")
    } else if secs < HOUR {
        format!("{}m ago", secs / MINUTE)
    } else if secs < DAY {
        format!("{}h ago", secs / HOUR)
    } else {
        format!("{}d ago", secs / DAY)
    }
}

#[cfg(test)]
#[path = "status_table_tests.rs"]
mod tests;
