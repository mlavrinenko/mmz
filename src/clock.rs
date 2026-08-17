//! The process clock: one resolved "now", pinnable through `MMZ_NOW`.
//!
//! Two surfaces carry a time — the `ran_at` a cache record stamps, and the `AGE`
//! column `mmz --status` renders — and both are output something else captures.
//! Reading the system clock at each use site makes that output differ run to
//! run, and lets two stamps inside ONE invocation disagree by however long the
//! work between them took. So the clock is resolved once, at the entry point,
//! and threaded down as a value.
//!
//! `MMZ_NOW` pins it to a Unix epoch in seconds. Its own variable, deliberately
//! never `SOURCE_DATE_EPOCH`: dev shells and CI routinely export that one at the
//! 1980-01-01 zip-epoch floor, and honouring it here would silently rewrite
//! every stamp in every project that has it set.
//!
//! A malformed pin is a hard error naming the variable, never a fall-back to the
//! system clock. A fall-back would hide the misconfiguration and hand back the
//! non-determinism the pin exists to remove — and it would surface as a diff in
//! somebody else's build rather than as a message here.
//!
//! Freshness is untouched by any of this. mmz compares digests, not times, so a
//! pinned clock changes what output SAYS, never which rules are fresh.

use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::{Error, Result};

/// Environment variable that pins the clock to a Unix epoch in seconds.
pub const VAR: &str = "MMZ_NOW";

/// One instant, in Unix seconds, resolved once and passed around.
///
/// A value rather than a function, so a caller cannot accidentally re-read the
/// clock halfway through: everything downstream of [`Clock::resolve`] sees the
/// same second.
#[derive(Debug, Clone, Copy)]
pub struct Clock {
    now: u64,
}

impl Clock {
    /// Resolves the clock for this process: the epoch `MMZ_NOW` pins when it is
    /// set, otherwise the system clock.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidNow`] when `MMZ_NOW` is set to anything but a
    /// Unix epoch in seconds — an empty value, a negative or fractional number,
    /// a date string, or bytes that are not valid unicode.
    pub fn resolve() -> Result<Self> {
        Self::from_pin(pin()?.as_deref())
    }

    /// A clock pinned to `secs`, for tests and for a caller that already has its
    /// own resolved time source.
    #[must_use]
    pub const fn pinned(secs: u64) -> Self {
        Self { now: secs }
    }

    /// The resolved instant, in Unix seconds.
    #[must_use]
    pub const fn now_secs(self) -> u64 {
        self.now
    }

    /// Builds a clock from `MMZ_NOW`'s raw text, or from the system clock when
    /// it is unset. Split out from [`Clock::resolve`] so the parsing rules are
    /// unit-testable: setting an environment variable is `unsafe` in edition
    /// 2024 and this crate denies `unsafe_code`, so a test cannot drive the
    /// variable in-process — only the CLI suite can, through a child process.
    ///
    /// Surrounding whitespace is trimmed, because a pin is usually a captured
    /// command substitution. Nothing else is forgiven.
    fn from_pin(raw: Option<&str>) -> Result<Self> {
        let Some(raw) = raw else {
            return Ok(Self { now: system_now() });
        };
        raw.trim()
            .parse::<u64>()
            .map(|now| Self { now })
            .map_err(|_| Error::InvalidNow {
                value: raw.to_owned(),
            })
    }
}

/// Reads `MMZ_NOW` as text, or `None` when it is unset. Bytes that are not valid
/// unicode cannot be an epoch, so they are refused here rather than carried into
/// the parse.
fn pin() -> Result<Option<String>> {
    match std::env::var(VAR) {
        Ok(raw) => Ok(Some(raw)),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(std::env::VarError::NotUnicode(raw)) => Err(Error::InvalidNow {
            value: raw.to_string_lossy().into_owned(),
        }),
    }
}

/// The system clock in Unix seconds. A clock somehow set before 1970 reads as
/// `0`, which is what a record stamped with it would have said anyway.
fn system_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

#[cfg(test)]
mod tests {
    use super::{Clock, VAR};

    #[test]
    fn an_unset_pin_reads_the_system_clock() {
        let clock = Clock::from_pin(None).expect("system clock");
        assert!(
            clock.now_secs() > 1_600_000_000,
            "an unset MMZ_NOW leaves mmz on the real clock"
        );
    }

    #[test]
    fn a_pin_is_taken_verbatim_modulo_whitespace() {
        assert_eq!(
            Clock::from_pin(Some("1700000000"))
                .expect("pinned")
                .now_secs(),
            1_700_000_000
        );
        assert_eq!(
            Clock::from_pin(Some(" 1700000000\n"))
                .expect("pinned")
                .now_secs(),
            1_700_000_000,
            "a captured command substitution keeps its whitespace"
        );
        assert_eq!(
            Clock::from_pin(Some("0")).expect("pinned").now_secs(),
            0,
            "the epoch itself is a legal pin"
        );
    }

    #[test]
    fn a_malformed_pin_is_refused_naming_the_variable() {
        for raw in ["", "   ", "abc", "-1", "1.5", "1_700_000_000", "2026-08-17"] {
            let err = Clock::from_pin(Some(raw)).expect_err("malformed pin is refused");
            let text = err.to_string();
            assert!(
                text.contains(VAR),
                "the message names the variable so the misconfiguration is findable: {text}"
            );
            assert!(
                text.contains(raw) || raw.trim().is_empty(),
                "and quotes what was actually set: {text}"
            );
        }
    }

    #[test]
    fn a_pin_past_u64_is_malformed_rather_than_wrapped() {
        assert!(
            Clock::from_pin(Some("18446744073709551616")).is_err(),
            "an overflowing epoch is refused, never silently truncated"
        );
    }
}
