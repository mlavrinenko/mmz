//! The run-anyway switch: `MMZ_DISABLE=1` makes a wrapped run execute its
//! command instead of skipping it.
//!
//! Every `just check` arm hardcodes `mmz just <gate>`, so the one handle that
//! reaches all of them at once is the environment. Under the switch a wrapped
//! run never takes the skip, but nothing else changes: the rule still matches,
//! `strict` still refuses, probes still run, `outputs` are still confirmed, and
//! the outcome is recorded exactly as a normal run records it. A forced gate
//! therefore leaves a record `mmz --is-fresh` accepts, which is what lets a
//! forced `just check` still close a task.
//!
//! `--is-fresh` and `--status` run nothing, so there is nothing to force and
//! they never read the variable; only [`crate::run`] resolves it.
//!
//! `1` turns it on. Unset, empty and `0` are off. Anything else is a hard error
//! naming the variable, as a malformed `MMZ_NOW` is: reading `yes` or `true` as
//! off would skip a run somebody asked to force, and reading it as on would
//! make a typo the way to burn a gate's whole runtime. Surrounding whitespace is
//! not forgiven either, since the value is a flag, not a captured number.

use crate::error::{Error, Result};

/// Environment variable that forces every wrapped run to execute.
pub const VAR: &str = "MMZ_DISABLE";

/// Whether this invocation runs its command even when the record says fresh,
/// resolved once and passed around like the [`crate::clock::Clock`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Disable {
    on: bool,
}

impl Disable {
    /// Resolves the switch for this process from `MMZ_DISABLE`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidDisable`] when `MMZ_DISABLE` is set to anything
    /// but `1`, `0` or the empty string, bytes that are not valid unicode
    /// included.
    pub fn resolve() -> Result<Self> {
        match std::env::var(VAR) {
            Ok(raw) => Self::from_raw(Some(&raw)),
            Err(std::env::VarError::NotPresent) => Self::from_raw(None),
            Err(std::env::VarError::NotUnicode(raw)) => Err(Error::InvalidDisable {
                value: raw.to_string_lossy().into_owned(),
            }),
        }
    }

    /// The switch off: every wrapped run memoizes as usual.
    #[must_use]
    pub const fn off() -> Self {
        Self { on: false }
    }

    /// The switch on, for tests and a caller that already decided.
    #[must_use]
    pub const fn on() -> Self {
        Self { on: true }
    }

    /// Whether the run must execute regardless of the record.
    #[must_use]
    pub const fn is_on(self) -> bool {
        self.on
    }

    /// Builds the switch from `MMZ_DISABLE`'s raw text, `None` when unset. Split
    /// out from [`Disable::resolve`] so the accepted values are unit-testable
    /// without setting an environment variable, which is `unsafe` in edition 2024
    /// (see [`crate::clock`]).
    fn from_raw(raw: Option<&str>) -> Result<Self> {
        match raw {
            None | Some("" | "0") => Ok(Self::off()),
            Some("1") => Ok(Self::on()),
            Some(other) => Err(Error::InvalidDisable {
                value: other.to_owned(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Disable, VAR};

    #[test]
    fn unset_empty_and_zero_are_off() {
        for raw in [None, Some(""), Some("0")] {
            assert!(
                !Disable::from_raw(raw).expect("off").is_on(),
                "{raw:?} leaves memoization alone"
            );
        }
    }

    #[test]
    fn one_is_on() {
        assert!(Disable::from_raw(Some("1")).expect("on").is_on());
    }

    #[test]
    fn anything_else_is_refused_naming_the_variable_and_the_value() {
        for raw in [
            "yes", "true", "TRUE", "2", "-1", "00", "01", " 1", "1\n", " ",
        ] {
            let err = Disable::from_raw(Some(raw)).expect_err("refused");
            let text = err.to_string();
            assert!(text.contains(VAR), "names the variable: {text}");
            assert!(text.contains(raw), "quotes what was set: {text}");
        }
    }
}
