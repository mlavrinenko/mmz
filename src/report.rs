//! The run report: `MMZ_REPORT=<path>` makes a wrapped run write `skipped` or
//! `ran` to that path once it decides which.
//!
//! A skip prints nothing unless `on_hit` says so, and a quiet gate runner sets
//! it empty, so the run's output cannot tell a caller which happened. Asking
//! `--is-fresh` first answers it at the price of hashing every input twice,
//! and the answer can go stale before the run that follows it. The report
//! comes from the run itself, on a channel the command's own output never
//! shares.
//!
//! The word is written before the command spawns, so a path mmz cannot write
//! stops the invocation before anything runs or is recorded. `--is-fresh` and
//! `--status` run nothing and never read the variable; only [`crate::run`]
//! resolves it. Unset and empty are off.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

/// Environment variable naming the file a wrapped run reports to.
pub const VAR: &str = "MMZ_REPORT";

/// What a wrapped run did with its command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// The record was fresh, so the command did not run.
    Skipped,
    /// The command ran, memoized or not, forced or not.
    Ran,
}

impl Decision {
    /// The word the report file holds.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Skipped => "skipped",
            Self::Ran => "ran",
        }
    }
}

/// Where this invocation reports its decision, if anywhere, resolved once
/// and passed around like the [`crate::disable::Disable`] switch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    path: Option<PathBuf>,
}

impl Report {
    /// Resolves the report path for this process from `MMZ_REPORT`.
    #[must_use]
    pub fn resolve() -> Self {
        Self::from_raw(std::env::var_os(VAR))
    }

    /// No report: the run writes nothing.
    #[must_use]
    pub const fn off() -> Self {
        Self { path: None }
    }

    /// Builds the report from `MMZ_REPORT`'s raw value, `None` when unset.
    /// Split out from [`Report::resolve`] so it is unit-testable without
    /// setting an environment variable (see [`crate::clock`]).
    fn from_raw(raw: Option<OsString>) -> Self {
        Self {
            path: raw.filter(|value| !value.is_empty()).map(PathBuf::from),
        }
    }

    /// Writes `decision` to the report file, resolving a relative path
    /// against `cwd`. Does nothing when the report is off.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ReportWrite`] naming the path as it was given when
    /// the file cannot be written.
    pub fn write(&self, cwd: &Path, decision: Decision) -> Result<()> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        std::fs::write(cwd.join(path), format!("{}\n", decision.word())).map_err(|source| {
            Error::ReportWrite {
                path: path.clone(),
                source,
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use super::{Decision, Report};

    #[test]
    fn unset_and_empty_are_off() {
        assert_eq!(Report::from_raw(None), Report::off());
        assert_eq!(Report::from_raw(Some(OsString::new())), Report::off());
    }

    #[test]
    fn off_writes_nothing() {
        let dir = tempfile::tempdir().expect("tempdir");
        Report::off()
            .write(dir.path(), Decision::Ran)
            .expect("nothing to write");
        assert_eq!(std::fs::read_dir(dir.path()).expect("read dir").count(), 0);
    }

    #[test]
    fn a_relative_path_resolves_against_cwd() {
        let dir = tempfile::tempdir().expect("tempdir");
        Report::from_raw(Some("out".into()))
            .write(dir.path(), Decision::Skipped)
            .expect("write");
        assert_eq!(
            std::fs::read_to_string(dir.path().join("out")).expect("read"),
            "skipped\n"
        );
    }
}
