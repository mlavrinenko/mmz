//! Why a probe produced no usable bytes: the runtime half of a probe's failure
//! surface, split out of `probe.rs` once `error.rs` reached its line cap.
//!
//! Every case here is one a probe reaches while *running* — a command that
//! failed, a file that would not read, a document that would not parse, a
//! selector that matched nothing — so every one of them shares an exit code
//! (`6`) and a consequence: mmz consumed no output and wrote no cache record.
//! The shape rules a probe is refused at load for are not here; they are
//! [`crate::error::Error::ProbeSource`], because a malformed probe is a
//! manifest defect (exit `4`) caught before anything runs.

use thiserror::Error;

use crate::ast::AstFailure;
use crate::error::Error;

/// Why a probe could not produce a digest, once it was asked to.
///
/// A sub-enum for the reason [`AstFailure`] is one: a family of related
/// refusals belongs beside the code that raises them, held in `Error` by the
/// single variant [`crate::error::Error::Probe`]. That variant supplies the
/// prefix naming the probe, so every message here is written to continue it
/// rather than to stand alone, and the rendered text is byte-for-byte what it
/// was when these were eight variants of `Error`: what moved is where they are
/// declared, not what anyone reads.
#[derive(Debug, Error)]
pub enum ProbeFailure {
    /// A probe command exited non-zero, so its stdout is not a usable input.
    #[error(
        "failed (exit {code}); mmz consumed no output and wrote no cache record\n  command: {run}\n  stderr: {stderr}"
    )]
    Failed {
        /// The `run` line, as the manifest spells it.
        run: String,
        /// The probe's exit code (1 for a signal death).
        code: i32,
        /// What the probe wrote to stderr, trimmed and capped.
        stderr: String,
    },

    /// A probe command could not be spawned at all — the same hard stop as a
    /// probe that ran and failed.
    #[error(
        "could not be run; mmz consumed no output and wrote no cache record\n  command: {run}\n  {source}"
    )]
    Spawn {
        /// The `run` line, as the manifest spells it.
        run: String,
        /// Underlying spawn error.
        source: std::io::Error,
    },

    /// A probe printed nothing and did not opt into that with `allow_empty`.
    #[error(
        "produced no output; that is almost always a selector that matched nothing — set `allow_empty: true` on the probe if empty really is a valid input\n  command: {run}"
    )]
    Empty {
        /// The `run` line, as the manifest spells it.
        run: String,
    },

    /// A probe's `file:` could not be read, so there are no bytes to select
    /// from. Named separately from any other unreadable file because a probe's
    /// error must name the probe *and* the path: a bare "no such file" leaves a
    /// reader hunting.
    #[error(
        "could not read `{path}`; mmz consumed no output and wrote no cache record\n  {source}"
    )]
    FileUnreadable {
        /// The path, as the manifest spells it.
        path: std::path::PathBuf,
        /// Underlying I/O error.
        source: std::io::Error,
    },

    /// The bytes a `json:` probe was pointed at are not one JSON value — an
    /// empty file, a tool that logged a line before its JSON, a truncated
    /// write.
    #[error(
        "read {origin}, which is not one JSON value ({reason}); mmz consumed no output and wrote no cache record"
    )]
    JsonInput {
        /// What was read, as the manifest points at it.
        origin: String,
        /// What the parser objected to.
        reason: String,
    },

    /// A `json:` program did not compile, or raised while running. One variant
    /// for both because the fix is the same edit — the program is wrong for
    /// the document it was pointed at — and the reason says which half broke.
    #[error(
        "could not select from {origin} ({reason}); mmz consumed no output and wrote no cache record\n  json: {program}"
    )]
    JsonFailed {
        /// The `json:` program, as the manifest spells it.
        program: String,
        /// What it was run against.
        origin: String,
        /// What jaq objected to.
        reason: String,
    },

    /// A `json:` selector yielded no value, or only `null`.
    ///
    /// The same refusal [`ProbeFailure::Empty`] makes for stdout, at the place
    /// the selection happens: a probe tracking `null` reports the same digest
    /// whatever the document does, so the rule is permanently fresh against an
    /// input nobody is measuring. `false` is a value and passes — jq's `-e`
    /// conflates the two only because a shell exit code cannot tell them
    /// apart, and mmz is under no such constraint.
    #[error(
        "selected nothing from {origin}; that digest would measure nothing, so the rule would be fresh forever — fix the selector, or set `allow_empty: true` if an absent value really is a valid input\n  json: {program}"
    )]
    JsonEmpty {
        /// The `json:` program, as the manifest spells it.
        program: String,
        /// What it was run against.
        origin: String,
    },

    /// A probe's `ast:` key could not produce a digest. The detail — and which
    /// refusal it is — lives one level further down again, with the matcher in
    /// [`AstFailure`], because one of those cases is answered by a cargo
    /// feature rather than a manifest edit and would not survive being
    /// flattened into a string here.
    #[error("{0}")]
    Ast(#[source] Box<AstFailure>),
}

impl ProbeFailure {
    /// Wraps this failure in the error that names the probe it came from.
    ///
    /// The one way a `ProbeFailure` becomes an [`Error`], so the two halves of
    /// a probe's message — whose it was, and what went wrong — can never be
    /// assembled two different ways. This module knows the second half;
    /// [`crate::probe`] and its digest step know the first.
    pub(crate) fn named(self, name: &str) -> Error {
        Error::Probe {
            name: name.to_owned(),
            source: Box::new(self),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ProbeFailure;
    use crate::ast::AstFailure;

    /// One of every variant, so a new one added without a message reviewed here
    /// is a compile error rather than a silent gap.
    fn every_failure() -> Vec<ProbeFailure> {
        vec![
            ProbeFailure::Failed {
                run: "just --dump".to_owned(),
                code: 5,
                stderr: "boom".to_owned(),
            },
            ProbeFailure::Spawn {
                run: "rustc -vV".to_owned(),
                source: std::io::Error::other("no such file"),
            },
            ProbeFailure::Empty {
                run: "jq -c .missing".to_owned(),
            },
            ProbeFailure::FileUnreadable {
                path: std::path::PathBuf::from("flake.lock"),
                source: std::io::Error::other("permission denied"),
            },
            ProbeFailure::JsonInput {
                origin: "flake.lock".to_owned(),
                reason: "trailing characters".to_owned(),
            },
            ProbeFailure::JsonFailed {
                program: ".nodes".to_owned(),
                origin: "flake.lock".to_owned(),
                reason: "cannot index".to_owned(),
            },
            ProbeFailure::JsonEmpty {
                program: ".nodes".to_owned(),
                origin: "flake.lock".to_owned(),
            },
            ProbeFailure::Ast(Box::new(AstFailure::NotText {
                origin: "src/lib.rs".to_owned(),
                lang: "rust".to_owned(),
            })),
        ]
    }

    /// The wrapper supplies the prefix, so a message here must continue it. A
    /// sub-message that restated it would render `probe `x` probe `x` …`, which
    /// nothing else in the build would catch — the enum is not the thing a user
    /// reads, and the CLI tests match on substrings.
    #[test]
    fn no_message_restates_the_prefix_its_wrapper_supplies() {
        for failure in every_failure() {
            let text = failure.to_string();
            assert!(
                !text.starts_with("probe "),
                "the wrapper already named the probe: {text}"
            );
        }
    }

    /// And the wrapped form reads as one sentence about a named probe, which is
    /// what these messages read as before the family moved out of `Error`.
    #[test]
    fn wrapping_names_the_probe_once() {
        for failure in every_failure() {
            let text = failure.named("fmt-recipe").to_string();
            assert!(
                text.starts_with("probe `fmt-recipe` "),
                "names the probe first: {text}"
            );
            assert_eq!(
                text.matches("probe `fmt-recipe`").count(),
                1,
                "and names it once: {text}"
            );
        }
    }
}
