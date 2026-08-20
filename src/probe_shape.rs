//! The rules about a probe's *set* of source keys, split out of `probe.rs`
//! once that file reached its line cap.
//!
//! These three methods are one concern: which combination of `run:`, `file:`
//! and `json:` is legal, what each illegal one should say, and how the legal
//! ones are read back afterwards. They stay `impl Probe` — the split moved the
//! code, not the type — so every call site is unchanged.

use crate::error::{Error, Result};

use super::{Probe, Source};

impl Probe {
    /// Refuses a probe whose source keys do not describe one readable thing.
    ///
    /// Three shapes are wrong, and each is refused with the edit that fixes it
    /// rather than resolved by a precedence rule a reader would have to
    /// memorise — precedence is how a manifest comes to mean something its
    /// author cannot see by reading it.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ProbeSource`] naming `name`.
    pub(super) fn check_shape(&self, name: &str) -> Result<()> {
        let reason = match (&self.run, &self.file, &self.json) {
            (Some(_), Some(_), _) => {
                "declares both `run:` and `file:`; a probe has exactly one source of bytes, so \
                 drop whichever one it does not mean"
            }
            (None, None, _) => {
                "declares neither `run:` nor `file:`; a probe needs a source of bytes — a command \
                 to run, or a file to read and select out of with `json:`"
            }
            (None, Some(_), None) => {
                "declares `file:` without `json:`; hashing a whole file is what a scope is for, \
                 and a scope keeps the gitignore filter and reports which file moved — declare \
                 the path under `scopes:`, or add a `json:` selector to narrow it"
            }
            _ => return Ok(()),
        };
        Err(Error::ProbeSource {
            name: name.to_owned(),
            reason: reason.to_owned(),
        })
    }

    /// The one source this probe reads. Total by construction after
    /// [`Probe::check_shape`]; a caller that skipped it gets the same load
    /// error rather than a panic.
    pub(super) fn source(&self, name: &str) -> Result<Source<'_>> {
        match (&self.run, &self.file) {
            (Some(run), None) => Ok(Source::Run(run)),
            (None, Some(file)) => Ok(Source::File(file)),
            _ => {
                self.check_shape(name)?;
                Err(Error::ProbeSource {
                    name: name.to_owned(),
                    reason: "declares no readable source".to_owned(),
                })
            }
        }
    }

    /// What this probe reads, phrased for the middle of an error message —
    /// a path as written, or the command line that produced the bytes.
    pub(super) fn origin(&self) -> String {
        match (&self.run, &self.file) {
            (_, Some(file)) => format!("`{}`", file.display()),
            (Some(run), None) => format!("the output of `{run}`"),
            (None, None) => "its source".to_owned(),
        }
    }
}
