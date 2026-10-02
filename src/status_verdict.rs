//! The verdict vocabulary `mmz --status` and `mmz --is-fresh` share: the
//! [`State`] a rule is in, the [`Assessment`] carrying the detail behind it,
//! and [`verdict`], the one function that decides between them. Split out of
//! [`crate::status`] when `expired` arrived and that module hit its line cap;
//! it owns resolving and measuring a rule, this owns what the measurement
//! means. Everything here is re-exported there, so no path outside moved.

use std::collections::BTreeMap;

use serde::Serialize;

use super::CachedInfo;
use crate::probe;
use crate::ttl::Ttl;

/// A rule's freshness verdict.
#[derive(Serialize, Clone, Copy)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum State {
    Fresh,
    Stale,
    Never,
    Failed,
    NoInputs,
    MissingOutput,
    Expired,
}

/// A verdict plus the detail behind it: the declared output that voided the
/// record, the probe whose output moved, or the ttl a pass outlived, when there
/// is one. The state alone cannot carry any of them — a reader told only
/// "stale" goes looking at the input files, which in every case is the wrong
/// place.
pub(crate) struct Assessment {
    pub(crate) state: State,
    pub(crate) missing_output: Option<String>,
    pub(crate) changed_probe: Option<String>,
    pub(crate) expired: Option<String>,
}

impl State {
    /// The label used in the human table; matches the JSON enum spelling.
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Fresh => "fresh",
            Self::Stale => "stale",
            Self::Never => "never",
            Self::Failed => "failed",
            Self::NoInputs => "no-inputs",
            Self::MissingOutput => "missing-output",
            Self::Expired => "expired",
        }
    }

    /// True only for [`State::Fresh`] — the sole state `mmz --is-fresh` passes.
    pub(crate) const fn is_fresh(self) -> bool {
        matches!(self, Self::Fresh)
    }

    /// True for the non-fresh states a recorded pass can clear: `Stale`,
    /// `Never`, `Failed`, `MissingOutput` and `Expired` — re-running the
    /// command under mmz regenerates the artifact (or asks the world again) and
    /// records the pass. `NoInputs` never has inputs to digest, so a wrapped
    /// run records nothing (or is refused under
    /// the `no_inputs` strictness) and the rule stays `NoInputs` — its remedy
    /// is fixing the manifest, not re-running under mmz.
    pub(crate) const fn is_remediable(self) -> bool {
        matches!(
            self,
            Self::Stale | Self::Never | Self::Failed | Self::MissingOutput | Self::Expired
        )
    }

    /// Why a non-fresh rule would re-run, for the `--is-fresh` gate's message.
    /// `None` when the rule is fresh. The `MissingOutput` wording here is the
    /// fallback; the caller names the path (see [`Assessment`]).
    pub(crate) const fn reason(self) -> Option<&'static str> {
        match self {
            Self::Fresh => None,
            Self::Stale => Some("inputs changed since it last passed"),
            Self::Never => Some("never run"),
            Self::Failed => Some("last run failed"),
            Self::NoInputs => Some("resolved no input files"),
            Self::MissingOutput => Some("a declared output is missing"),
            Self::Expired => Some("last pass is older than its ttl"),
        }
    }
}

impl Assessment {
    /// The verdict's one-line explanation, naming the missing artifact when
    /// that is what voided the record, the probe when that is what moved, or
    /// the ttl when the pass is merely too old.
    /// `None` when the rule is fresh.
    pub(crate) fn reason(&self) -> Option<String> {
        if let Some(path) = self.missing_output.as_deref() {
            return Some(format!("declared output `{path}` is missing"));
        }
        if let Some(name) = self.changed_probe.as_deref() {
            return Some(format!("probe `{name}` changed since it last passed"));
        }
        if let Some(ttl) = self.expired.as_deref() {
            return Some(format!("last pass is older than its ttl `{ttl}`"));
        }
        self.state.reason().map(str::to_owned)
    }
}

/// The freshness verdict for `digest` against a rule's stored record: fresh
/// only when the record is present, succeeded, its digest matches, every
/// output the rule declares is still on disk (`missing` is the first one that
/// is not), and it has not outlived the rule's `ttl` (`expired` is that ttl).
///
/// A missing artifact outranks a digest mismatch. Both would re-run the rule,
/// but only one of them is a fact a reader would otherwise never guess — an
/// input change is the assumption they already hold — so the verdict names the
/// gone artifact rather than sending them to diff the inputs. A stale verdict
/// gets the same treatment for probes: when the record's stored probe digests
/// show one moved, `probes` is compared against them and the culprit is named,
/// because no file changed and diffing the files would find nothing. Age ranks
/// last: `expired` means exactly "nothing moved, the pass is just too old", so
/// a record that is both old and stale reports the change.
pub(super) fn verdict(
    cached: Option<&CachedInfo>,
    digest: &str,
    missing: Option<String>,
    expired: Option<&Ttl>,
    probes: &BTreeMap<String, String>,
) -> Assessment {
    let state = match cached {
        None => State::Never,
        Some(record) if !record.ok => State::Failed,
        Some(_) if missing.is_some() => State::MissingOutput,
        Some(record) if record.digest != digest => State::Stale,
        Some(_) if expired.is_some() => State::Expired,
        Some(_) => State::Fresh,
    };
    let missing_output = match state {
        State::MissingOutput => missing,
        _ => None,
    };
    let changed_probe = match state {
        State::Stale => cached.and_then(|record| probe::first_changed(&record.probes, probes)),
        _ => None,
    };
    let expired = match state {
        State::Expired => expired.map(|ttl| ttl.as_str().to_owned()),
        _ => None,
    };
    Assessment {
        state,
        missing_output,
        changed_probe,
        expired,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{CachedInfo, State, verdict};
    use crate::ttl::Ttl;

    fn record(ok: bool, digest: &str) -> CachedInfo {
        CachedInfo {
            digest: digest.to_owned(),
            ok,
            ran_at: 0,
            outputs: Vec::new(),
            probes: BTreeMap::new(),
        }
    }

    fn label(
        cached: Option<&CachedInfo>,
        missing: Option<&str>,
        expired: Option<&Ttl>,
    ) -> &'static str {
        let none = BTreeMap::new();
        verdict(cached, "d1", missing.map(str::to_owned), expired, &none)
            .state
            .label()
    }

    #[test]
    fn an_outlived_pass_with_unchanged_inputs_is_expired() {
        let ttl = Ttl::parse("1h").expect("valid");
        let ok = record(true, "d1");
        assert_eq!(label(Some(&ok), None, None), "fresh");
        assert_eq!(label(Some(&ok), None, Some(&ttl)), "expired");

        let none = BTreeMap::new();
        let assessed = verdict(Some(&ok), "d1", None, Some(&ttl), &none);
        assert!(matches!(assessed.state, State::Expired));
        assert!(
            assessed.state.is_remediable(),
            "a re-run records a new pass"
        );
        assert_eq!(
            assessed.reason().as_deref(),
            Some("last pass is older than its ttl `1h`")
        );
    }

    #[test]
    fn every_other_reason_to_rerun_outranks_age() {
        let ttl = Ttl::parse("1h").expect("valid");
        assert_eq!(label(None, None, Some(&ttl)), "never");
        assert_eq!(
            label(Some(&record(false, "d1")), None, Some(&ttl)),
            "failed"
        );
        assert_eq!(
            label(Some(&record(true, "d1")), Some("out.txt"), Some(&ttl)),
            "missing-output"
        );
        assert_eq!(
            label(Some(&record(true, "d0")), None, Some(&ttl)),
            "stale",
            "an input change is what re-runs it; age would only hide where to look"
        );
    }

    #[test]
    fn the_bare_state_reason_covers_expired_too() {
        assert_eq!(
            State::Expired.reason(),
            Some("last pass is older than its ttl")
        );
    }
}
