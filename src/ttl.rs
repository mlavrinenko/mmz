//! A rule's `ttl`: how long a successful record stays fresh once its inputs
//! stop being the whole story.
//!
//! Some verdicts depend on the world rather than on any file — `npm audit`
//! reads an advisory database that changes daily, a registry check asks a
//! server. Memoized on input digests alone, one pass of either is fresh
//! forever. A `ttl` bounds that: a record whose age has reached it is
//! `expired`, and the rule re-runs exactly as a stale one would.
//!
//! The grammar is deliberately minimal and strict: one positive whole number
//! and one unit — `s`, `m`, `h` or `d` — so `30m`, `12h`, `7d`. No compounds
//! (`1h30m`), signs, leading zeros, fractions, whitespace or upper case: a
//! duration a reader has to think about is one two readers can disagree on.
//! Zero is refused too, since a rule that is never fresh is a rule that does
//! not want mmz. A rejected value fails the manifest load (exit 4), naming it.
//!
//! Age is measured against the process [`Clock`], so `MMZ_NOW` pins it, and
//! against the record's `ran_at` — stamped on every write since the first
//! record format, so a successful record's `ran_at` is when it succeeded.

use std::fmt;

use serde::Deserialize;

use crate::clock::Clock;
use crate::manifest::Command;

/// Seconds per unit, in the order the error message lists them.
const UNITS: [(char, u64); 4] = [('s', 1), ('m', 60), ('h', 3_600), ('d', 86_400)];

/// What every rejection says, so the message always teaches the grammar.
const GRAMMAR: &str =
    "expected a positive whole number and one unit — s, m, h or d (e.g. 30m, 12h, 7d)";

/// A parsed `ttl`: its length in seconds, and the spelling the manifest used,
/// kept so every surface that echoes it (`--status`, `--dump-config`, a gate's
/// reason) shows the value the reader wrote rather than a normalisation of it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(try_from = "String")]
pub struct Ttl {
    secs: u64,
    text: String,
}

impl Ttl {
    /// Parses a duration in the grammar the module doc describes.
    ///
    /// # Errors
    ///
    /// Returns a message naming `text` and the grammar when it is malformed,
    /// zero, or more seconds than a `u64` holds.
    pub fn parse(text: &str) -> Result<Self, String> {
        let refuse = |why: &str| format!("invalid ttl `{text}`: {why}");
        let unit = text.chars().last().ok_or_else(|| refuse(GRAMMAR))?;
        let digits = text.strip_suffix(unit).unwrap_or_default();
        let scale = UNITS
            .iter()
            .find(|(name, _)| *name == unit)
            .map(|(_, scale)| *scale)
            .ok_or_else(|| refuse(GRAMMAR))?;
        let malformed = digits.is_empty()
            || !digits.bytes().all(|byte| byte.is_ascii_digit())
            || (digits.starts_with('0') && digits != "0");
        if malformed {
            return Err(refuse(GRAMMAR));
        }
        let count: u64 = digits.parse().map_err(|_| refuse("too large"))?;
        if count == 0 {
            return Err(refuse(
                "a zero ttl is never fresh; drop the rule instead of memoizing it",
            ));
        }
        let secs = count
            .checked_mul(scale)
            .ok_or_else(|| refuse("too large"))?;
        Ok(Self {
            secs,
            text: text.to_owned(),
        })
    }

    /// The duration in seconds.
    #[must_use]
    pub const fn secs(&self) -> u64 {
        self.secs
    }

    /// The value as the manifest spelled it.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// The first instant, in Unix seconds, at which a record written at
    /// `ran_at` is no longer fresh.
    #[must_use]
    pub const fn expires_at(&self, ran_at: u64) -> u64 {
        ran_at.saturating_add(self.secs)
    }

    /// True when a record written at `ran_at` has reached this ttl as of
    /// `now`. A record stamped after `now` (a rewound clock) has age zero, so
    /// it is not expired.
    #[must_use]
    pub const fn is_expired(&self, ran_at: u64, now: Clock) -> bool {
        now.now_secs() >= self.expires_at(ran_at)
    }
}

impl TryFrom<String> for Ttl {
    type Error = String;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        Self::parse(&text)
    }
}

impl fmt::Display for Ttl {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(&self.text)
    }
}

/// True for a rule whose only input is time: a `ttl` and no declared
/// `inputs`. It digests an empty input set instead of being `no-inputs`. A ttl
/// rule whose declared inputs resolve to nothing is still `no-inputs`: that is
/// a scope that stopped matching, which a ttl does not make any less wrong.
#[must_use]
pub fn time_only(rule: &Command) -> bool {
    rule.ttl.is_some() && rule.inputs.is_empty()
}

/// The ttl a successful record at `ran_at` has outlived, if any — the one
/// question every freshness surface asks of a rule's `ttl`. `None` for a rule
/// without one, a failed record (already not fresh, for a better reason), or a
/// record still inside it.
#[must_use]
pub fn outlived(ttl: Option<&Ttl>, ok: bool, ran_at: u64, now: Clock) -> Option<&Ttl> {
    ttl.filter(|ttl| ok && ttl.is_expired(ran_at, now))
}

#[cfg(test)]
mod tests {
    use super::{Ttl, outlived};
    use crate::clock::Clock;

    fn secs(text: &str) -> u64 {
        Ttl::parse(text).expect("valid ttl").secs()
    }

    #[test]
    fn each_unit_scales_its_count() {
        assert_eq!(secs("45s"), 45);
        assert_eq!(secs("30m"), 30 * 60);
        assert_eq!(secs("12h"), 12 * 3_600);
        assert_eq!(secs("7d"), 7 * 86_400);
        assert_eq!(secs("1s"), 1, "the smallest legal ttl");
    }

    #[test]
    fn the_spelling_is_kept_for_display() {
        let ttl = Ttl::parse("24h").expect("valid");
        assert_eq!(ttl.as_str(), "24h");
        assert_eq!(ttl.to_string(), "24h");
    }

    #[test]
    fn anything_outside_the_grammar_is_refused_naming_it() {
        for bad in [
            "", "h", "10", "1h30m", "1.5h", "-1h", "+1h", "01h", "00h", "1H", "1 h", " 1h", "1h ",
            "1w", "1ms", "1y", "1ч", "١h",
        ] {
            let err = Ttl::parse(bad).expect_err("malformed ttl is refused");
            assert!(err.contains(&format!("`{bad}`")), "names the value: {err}");
            assert!(err.contains("s, m, h or d"), "teaches the grammar: {err}");
        }
    }

    #[test]
    fn zero_is_refused_with_its_own_reason() {
        for zero in ["0s", "0m", "0h", "0d"] {
            let err = Ttl::parse(zero).expect_err("zero ttl is refused");
            assert!(err.contains("zero"), "{err}");
        }
    }

    #[test]
    fn a_ttl_past_a_timestamp_is_refused_rather_than_wrapped() {
        for huge in [
            "99999999999999999999d",
            "999999999999999999d",
            "18446744073709551616s",
        ] {
            let err = Ttl::parse(huge).expect_err("overflowing ttl is refused");
            assert!(err.contains("too large"), "{err}");
        }
        assert_eq!(
            secs("18446744073709551615s"),
            u64::MAX,
            "the largest legal ttl"
        );
    }

    #[test]
    fn deserializes_through_the_same_parser() {
        let ttl: Ttl = serde_yaml_ng::from_str("12h").expect("valid");
        assert_eq!(ttl.secs(), 12 * 3_600);
        let err = serde_yaml_ng::from_str::<Ttl>("0h").expect_err("zero refused");
        assert!(err.to_string().contains("invalid ttl `0h`"), "{err}");
    }

    #[test]
    fn a_record_expires_once_its_age_reaches_the_ttl() {
        let ttl = Ttl::parse("1h").expect("valid");
        let ran_at = 1_000;
        assert_eq!(ttl.expires_at(ran_at), 4_600);
        assert!(
            !ttl.is_expired(ran_at, Clock::pinned(ran_at)),
            "just written"
        );
        assert!(
            !ttl.is_expired(ran_at, Clock::pinned(4_599)),
            "one second inside"
        );
        assert!(ttl.is_expired(ran_at, Clock::pinned(4_600)), "at the ttl");
        assert!(ttl.is_expired(ran_at, Clock::pinned(90_000)), "long past");
        assert!(
            !ttl.is_expired(ran_at, Clock::pinned(10)),
            "a record from the future has age zero"
        );
        assert_eq!(
            ttl.expires_at(u64::MAX),
            u64::MAX,
            "a far-future stamp saturates"
        );
    }

    #[test]
    fn outlived_names_the_ttl_only_for_an_expired_pass() {
        let ttl = Ttl::parse("1h").expect("valid");
        let late = Clock::pinned(10_000);
        assert_eq!(outlived(Some(&ttl), true, 0, late), Some(&ttl));
        assert_eq!(outlived(Some(&ttl), true, 9_000, late), None, "inside");
        assert_eq!(outlived(Some(&ttl), false, 0, late), None, "a failure");
        assert_eq!(outlived(None, true, 0, late), None, "no ttl");
    }
}
