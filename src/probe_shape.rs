//! The rules about a probe's *set* of source keys, split out of `probe.rs`
//! once that file reached its line cap.
//!
//! These methods are one concern: which combination of `run:`, `file:` and
//! `json:` is legal, what each illegal one should say, and how the legal ones
//! are read back afterwards. They stay `impl Probe` — the split moved the
//! code, not the type — so every call site is unchanged.
//!
//! Everything here is decided from the manifest alone, which is why it runs at
//! load rather than when a probe is first reached. That is also the seam that
//! splits the `capture:` rules in two: whether a name *could* be a
//! metavariable is a fact about the string, and lives here; whether the
//! pattern actually *defines* it needs the compiled pattern, and lives in
//! [`crate::ast`] beside the matcher that knows.

use crate::error::{Error, Result};

use super::{Probe, Source};

impl Probe {
    /// Refuses a probe whose source keys do not describe one readable thing.
    ///
    /// Each wrong shape is refused with the edit that fixes it rather than
    /// resolved by a precedence rule a reader would have to memorise —
    /// precedence is how a manifest comes to mean something its author cannot
    /// see by reading it. The key-set rules come first and the `capture:` list
    /// second, so a probe wrong in both ways is told about the missing `ast:`
    /// rather than about a list that cannot mean anything without it.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ProbeSource`] naming `name`.
    pub(super) fn check_shape(&self, name: &str) -> Result<()> {
        let found = self
            .shape_error()
            .map(str::to_owned)
            .or_else(|| self.capture_error());
        let Some(reason) = found else {
            return Ok(());
        };
        Err(Error::ProbeSource {
            name: name.to_owned(),
            reason,
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
        if self.capture.is_some() && self.ast.is_none() {
            return Some(
                "declares `capture:` without `ast:`; `capture:` names metavariables of an `ast:` \
                 pattern, and with no pattern to name them in it is a key whose author believes \
                 it did something — add the `ast:` pattern, or drop `capture:`",
            );
        }
        None
    }

    /// The first thing wrong with this probe's `capture:` list, judged from the
    /// manifest alone.
    ///
    /// An empty list is refused for the reason a `json:` selection that yielded
    /// nothing is: it asks mmz to hash the empty string once per match, so the
    /// probe reports one digest whatever the source says and every rule naming
    /// it reads fresh forever. Dropping the key means the whole node, which is
    /// what an author who wanted no narrowing meant.
    ///
    /// The name rule is ast-grep's own: a capture is an uppercase letter
    /// followed by uppercase letters, digits or underscores. `$NAME` copied
    /// straight out of the pattern, a lowercase name, and `$_X` (which
    /// ast-grep *drops* rather than captures) can none of them ever be
    /// defined, so they are answered here rather than left to come back as
    /// "the pattern does not define it", which sends a reader to edit the
    /// pattern instead of the list.
    fn capture_error(&self) -> Option<String> {
        let names = self.capture.as_deref()?;
        if names.is_empty() {
            return Some(
                "declares an empty `capture:` list; hashing no part of a match reports the same \
                 digest whatever the source says, leaving every rule that names it fresh forever \
                 — name the metavariables that matter, or drop `capture:` to hash the whole \
                 matched node"
                    .to_owned(),
            );
        }
        for (index, name) in names.iter().enumerate() {
            if !is_metavariable(name) {
                return Some(format!(
                    "declares the capture `{name}`, which is not a metavariable name; write it \
                     as the pattern does but without the `$` — an uppercase letter followed by \
                     uppercase letters, digits or underscores, as in `NAME` for `$NAME` or \
                     `ARGS` for `$$$ARGS`"
                ));
            }
            if names.iter().take(index).any(|earlier| earlier == name) {
                return Some(format!(
                    "names the capture `{name}` twice; `capture:` is the set of parts that \
                     matter and a repeat measures nothing the first mention did not — drop the \
                     duplicate"
                ));
            }
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

/// Whether `name` could name a captured metavariable at all.
///
/// Mirrors `ast_grep_core::meta_var::extract_meta_var`: the first character
/// must be `A`–`Z` and the rest `A`–`Z`, `0`–`9` or `_`. A leading underscore
/// is deliberately excluded even though ast-grep accepts it in a pattern,
/// because `$_X` is a *dropped* variable there rather than a captured one — it
/// never reaches an env, so naming it could only ever hash nothing.
fn is_metavariable(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|first| first.is_ascii_uppercase())
        && chars.all(|rest| rest.is_ascii_uppercase() || rest.is_ascii_digit() || rest == '_')
}
