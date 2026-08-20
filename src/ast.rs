//! In-process AST matching: the engine behind a probe's `ast:` key.
//!
//! A probe with `ast:` reaches a *structural* slice of a source file — a
//! function, a type, an impl block — with no process at all. mmz reads the
//! bytes, parses them with a bundled tree-sitter grammar, matches an ast-grep
//! pattern against the tree, and hands the canonical rendering of every match
//! to the hasher. Nothing is spawned, nothing has to be on `PATH`, and no regex
//! is asked to pretend it is a parser.
//!
//! ```yaml
//! probes:
//!   wire-types:
//!     file: src/types.rs
//!     ast: 'pub struct $NAME { $$$FIELDS }'
//! ```
//!
//! That is an input nothing else here can express. A scope naming
//! `src/types.rs` hashes the file, so a comment reflow re-runs the rule; the
//! probe above moves only when a public struct definition does.
//!
//! # A match is a whole node
//!
//! Which decides what a pattern costs: the input is every node the pattern
//! matched, entire. `pub fn $N($$$A) -> $R { $$$B }` depends on the bodies as
//! well as the signatures, and there is no spelling that keeps one and drops
//! the other, because a signature stops being a node of its own once a body
//! follows it. Narrowing a match to part of itself is a question about the
//! captured metavariables, and a separate feature from this one.
//!
//! What it does buy over a scope regardless: everything the pattern did not
//! match is free to move — comments, imports, private items, a whole second
//! module — and none of it is an input.
//!
//! # What is hashed
//!
//! One canonical rendering per match, joined one per line — the same shape
//! [`crate::probe`] joins `json:` outputs in. See [`crate::ast_render`] for what
//! a rendering keeps (every token, exactly) and what it drops (the whitespace
//! between them), and for the grammar-version cost that choice carries.
//!
//! Matches stay in the order the tree walk found them, which is document order,
//! pre-order, and deterministic for a given grammar. They are deliberately not
//! sorted: this is the same line [`crate::json`] draws when it sorts object keys
//! and leaves array order alone. Sorting would be a *narrowing* — two files
//! differing only in declaration order would report one digest — and a probe
//! that cannot see a real edit is the failure this tool refuses. Order that
//! comes from the document is content; only order a renderer chose is
//! presentation.
//!
//! # A source that does not parse cleanly is still matched
//!
//! Unlike [`crate::json`], which refuses bytes that are not one whole value,
//! this module hashes whatever tree-sitter recovered. Two reasons. A file using
//! syntax newer than the bundled grammar parses with `ERROR` nodes and is a
//! perfectly ordinary state, not a corrupt one — refusing it would break a
//! probe on a language's next release rather than on anything the project did.
//! And the case worth catching is caught anyway: a half-written file usually
//! stops matching, and a pattern that matched nothing is already a hard error
//! in [`crate::probe`].

use ast_grep_core::tree_sitter::StrDoc;
use ast_grep_core::{AstGrep, Pattern};
use ast_grep_language::SupportLang;
use thiserror::Error;

use crate::ast_lang;
use crate::ast_render::{self, Doc};

/// Why an `ast:` probe could not produce a digest.
///
/// Every case is a manifest defect or a build mmz was not made with — never a
/// state of the world — so every message carries the edit or the flag that
/// fixes it. It is a public error type rather than a private reason string
/// (which is what [`crate::json`] uses) because a missing grammar is answered
/// by a build command, and losing that in a `String` would make the one failure
/// a user cannot guess their way out of the one mmz explains least.
#[derive(Debug, Error)]
pub enum AstFailure {
    /// A `run:` probe, or a `file:` whose extension maps to no bundled
    /// grammar, with no `lang:` to say what to parse it as.
    #[error(
        "cannot tell what language {origin} is, so `ast:` has nothing to parse it with; name it with `lang:` (this build parses: {available})"
    )]
    LanguageUnknown {
        /// What the probe reads, phrased for mid-message.
        origin: String,
        /// The languages this build carries.
        available: String,
    },

    /// A `lang:` naming a language mmz supports, in a build compiled without
    /// its grammar. The one failure here whose fix is a build, not an edit.
    #[error(
        "this mmz was built without the `{name}` grammar; rebuild it with `--features lang-{name}` (this build parses: {available})"
    )]
    LanguageMissing {
        /// The language named by `lang:`.
        name: String,
        /// The languages this build carries.
        available: String,
    },

    /// A `lang:` naming something mmz has no grammar for in any build.
    #[error("`{name}` is not a language mmz can parse; this build parses: {available}")]
    LanguageUnsupported {
        /// The language named by `lang:`.
        name: String,
        /// The languages this build carries.
        available: String,
    },

    /// The bytes are not UTF-8, so there is no source text to parse.
    #[error("{origin} is not valid UTF-8, so it cannot be parsed as {lang} source")]
    NotText {
        /// What the probe reads.
        origin: String,
        /// The language it was to be parsed as.
        lang: String,
    },

    /// The `ast:` pattern is not one clean node in the target language — an
    /// empty pattern, two statements where one was meant, or source the grammar
    /// could only parse into an `ERROR` node.
    ///
    /// That last case is the one worth stating: tree-sitter error-recovers a
    /// *pattern* as readily as a file, so `pub fn $N(` compiles into something
    /// that simply matches nothing rather than failing. Left alone it would
    /// reach the caller as an empty match set — the right refusal by luck, and
    /// no refusal at all under `allow_empty: true`. [`select`] asks the
    /// compiled pattern whether it recovered and refuses it here instead, so a
    /// typo is answered where it was made.
    #[error("`{pattern}` is not a usable {lang} pattern: {reason}")]
    Pattern {
        /// The pattern as the manifest wrote it.
        pattern: String,
        /// The language it was compiled for.
        lang: String,
        /// What ast-grep said was wrong with it.
        reason: String,
    },

    /// tree-sitter declined to produce a tree at all. Distinct from a source
    /// with syntax errors, which parses fine into a tree holding `ERROR` nodes.
    #[error("{origin} could not be parsed as {lang}: {reason}")]
    Unparsable {
        /// What the probe reads.
        origin: String,
        /// The language it was parsed as.
        lang: String,
        /// What tree-sitter reported.
        reason: String,
    },

    /// The pattern is valid and matched no node. Refused for the reason an
    /// empty `json:` selection is: a probe measuring nothing reports one digest
    /// whatever the file says, so every rule naming it reads fresh forever.
    ///
    /// [`crate::probe`] owns whether this is raised — `allow_empty: true` opts
    /// out of it — exactly as it owns the same call for `json:`. Only the
    /// wording lives here, beside the matcher that knows what was asked.
    #[error(
        "`{pattern}` matched nothing in {origin}; a pattern that matches nothing reports the same digest whatever the source says, leaving every rule that names it fresh forever — fix the pattern, or set `allow_empty: true` if no match really is a valid input"
    )]
    Empty {
        /// The pattern as the manifest wrote it.
        pattern: String,
        /// What the probe reads.
        origin: String,
    },
}

/// The grammar to parse `origin`'s bytes with: `declared` when the manifest set
/// `lang:`, otherwise inferred from `path`'s extension.
///
/// A `run:` probe has no path, so it must declare one — inferring a language
/// from a command line is a guess, and a guess here silently parses source as
/// the wrong grammar and hashes whatever fell out.
///
/// # Errors
///
/// Returns the [`AstFailure`] language variants, each naming what this build
/// can parse so a miss is answerable without going to look it up.
pub(crate) fn resolve_lang(
    declared: Option<&str>,
    path: Option<&std::path::Path>,
    origin: &str,
) -> Result<SupportLang, AstFailure> {
    let Some(name) = declared else {
        return path
            .and_then(ast_lang::by_extension)
            .ok_or_else(|| AstFailure::LanguageUnknown {
                origin: origin.to_owned(),
                available: ast_lang::available(),
            });
    };
    if let Some(grammar) = ast_lang::by_name(name) {
        return Ok(grammar);
    }
    let available = ast_lang::available();
    if ast_lang::is_known(name) {
        return Err(AstFailure::LanguageMissing {
            name: name.to_owned(),
            available,
        });
    }
    Err(AstFailure::LanguageUnsupported {
        name: name.to_owned(),
        available,
    })
}

/// Matches `pattern` over `input` parsed as `lang`, returning one canonical
/// rendering per match in document order.
///
/// The result is a list rather than one blob for the reason [`crate::json`]
/// returns one: "how many nodes did this match" is the question
/// [`crate::probe`] has to answer to refuse a pattern that matched nothing, and
/// joining first would throw it away.
///
/// # Errors
///
/// Returns [`AstFailure::NotText`], [`AstFailure::Pattern`] or
/// [`AstFailure::Unparsable`]. Emptiness is not decided here.
pub(crate) fn select(
    lang: SupportLang,
    pattern: &str,
    input: &[u8],
    origin: &str,
) -> Result<Vec<Vec<u8>>, AstFailure> {
    let source = std::str::from_utf8(input).map_err(|_| AstFailure::NotText {
        origin: origin.to_owned(),
        lang: format!("{lang:?}"),
    })?;
    let compiled = Pattern::try_new(pattern, lang).map_err(|err| AstFailure::Pattern {
        pattern: pattern.to_owned(),
        lang: format!("{lang:?}"),
        reason: err.to_string(),
    })?;
    if compiled.has_error() {
        return Err(AstFailure::Pattern {
            pattern: pattern.to_owned(),
            lang: format!("{lang:?}"),
            reason: "the grammar could not parse it and recovered into an error node, so it \
                     would match nothing at all"
                .to_owned(),
        });
    }
    let doc = StrDoc::try_new(source, lang).map_err(|reason| AstFailure::Unparsable {
        origin: origin.to_owned(),
        lang: format!("{lang:?}"),
        reason,
    })?;
    let root: AstGrep<Doc> = AstGrep::doc(doc);
    Ok(root
        .root()
        .find_all(&compiled)
        .map(|found| ast_render::render(&found))
        .collect())
}

#[cfg(test)]
#[path = "ast_tests.rs"]
mod tests;
