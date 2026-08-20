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
        let Some(reason) = self.shape_error() else {
            return Ok(());
        };
        Err(Error::ProbeSource {
            name: name.to_owned(),
            reason: reason.to_owned(),
        })
    }

    /// The first thing wrong with this probe's key set, or `None` when the
    /// combination is one mmz can read.
    ///
    /// Written as ordered guards rather than one match over five options
    /// because the rules are independent: each names one pair of keys, and a
    /// reader checking whether their probe is legal reads down the list rather
    /// than finding their case among thirty-two tuples.
    fn shape_error(&self) -> Option<&'static str> {
        if self.run.is_some() && self.file.is_some() {
            return Some(
                "declares both `run:` and `file:`; a probe has exactly one source of bytes, so \
                 drop whichever one it does not mean",
            );
        }
        if self.run.is_none() && self.file.is_none() {
            return Some(
                "declares neither `run:` nor `file:`; a probe needs a source of bytes — a command \
                 to run, or a file to read and select out of with `json:` or `ast:`",
            );
        }
        if self.json.is_some() && self.ast.is_some() {
            return Some(
                "declares both `json:` and `ast:`; a probe has one selector, and feeding a \
                 matched node back through jq would need a mapping from syntax to JSON that mmz \
                 does not define — drop whichever one it does not mean",
            );
        }
        if self.file.is_some() && self.json.is_none() && self.ast.is_none() {
            return Some(
                "declares `file:` with no selector; hashing a whole file is what a scope is for, \
                 and a scope keeps the gitignore filter and reports which file moved — declare \
                 the path under `scopes:`, or narrow it with `json:` or `ast:`",
            );
        }
        if self.lang.is_some() && self.ast.is_none() {
            return Some(
                "declares `lang:` without `ast:`; `lang:` only says how to parse the bytes an \
                 `ast:` pattern matches over, and a key that silently does nothing is a key \
                 whose author believes it did something — add the `ast:` pattern, or drop `lang:`",
            );
        }
        None
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
