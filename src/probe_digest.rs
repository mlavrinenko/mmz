//! What a probe's bytes become once it has them: raw, a `json:` selection, or
//! an `ast:` match set. Split out of `probe.rs` once that file reached its line
//! cap; these are one concern, and the concern is the last step before the
//! hasher.
//!
//! Every function here returns before hashing on any failure, which is the
//! invariant the whole module exists to keep: a command that failed, a document
//! that would not parse, a selector that matched nothing and a grammar mmz was
//! not built with all stop here rather than contributing bytes to an input
//! digest.

use crate::ast::{self, AstFailure};
use crate::error::{Error, Result};
use crate::hashing;
use crate::json;

use super::Probe;

/// Hashes the bytes as they came, for a probe with no selector. Only a `run:`
/// probe reaches this, so the empty-output message can quote its command line.
pub(super) fn hash_raw(name: &str, probe: &Probe, bytes: &[u8]) -> Result<String> {
    if !probe.allow_empty && bytes.iter().all(u8::is_ascii_whitespace) {
        return Err(Error::ProbeEmpty {
            name: name.to_owned(),
            run: probe.run.clone().unwrap_or_default(),
        });
    }
    Ok(hashing::hash_bytes(bytes))
}

/// Selects out of the bytes with `program` and hashes the canonical rendering
/// of every value it produced.
///
/// The outputs are joined one per line, the shape `jq -c` itself prints, so a
/// two-output selector (`.a, .b`) has a stable digest and one that grew a
/// second output has a different one. Ordering is jq's own evaluation order,
/// which is deterministic.
pub(super) fn hash_selection(
    name: &str,
    probe: &Probe,
    program: &str,
    bytes: &[u8],
) -> Result<String> {
    let selected = json::select(program, bytes).map_err(|failure| match failure {
        json::Failure::Input(reason) => Error::ProbeJsonInput {
            name: name.to_owned(),
            origin: probe.origin(),
            reason,
        },
        json::Failure::Program(reason) | json::Failure::Run(reason) => Error::ProbeJsonFailed {
            name: name.to_owned(),
            program: program.to_owned(),
            origin: probe.origin(),
            reason,
        },
    })?;
    if !probe.allow_empty && selected_nothing(&selected) {
        return Err(Error::ProbeJsonEmpty {
            name: name.to_owned(),
            program: program.to_owned(),
            origin: probe.origin(),
        });
    }
    Ok(hash_joined(&selected))
}

/// Parses the bytes as source and hashes the canonical rendering of every node
/// `pattern` matched — or, when the probe declares `capture:`, of the parts of
/// each match it named.
///
/// Joined one per line, the same shape [`hash_selection`] uses, so the two
/// selectors differ in what they select and in nothing else. Matches stay in
/// the document order the tree walk produced them in — see [`crate::ast`] for
/// why that order is kept rather than sorted away.
pub(super) fn hash_matches(
    name: &str,
    probe: &Probe,
    pattern: &str,
    bytes: &[u8],
) -> Result<String> {
    let origin = probe.origin();
    let lang = ast::resolve_lang(probe.lang.as_deref(), probe.file.as_deref(), &origin)
        .map_err(|failure| ast_error(name, failure))?;
    let matched = ast::select(lang, pattern, probe.capture.as_deref(), bytes, &origin)
        .map_err(|failure| ast_error(name, failure))?;
    if !probe.allow_empty && matched.is_empty() {
        return Err(ast_error(
            name,
            AstFailure::Empty {
                pattern: pattern.to_owned(),
                origin,
            },
        ));
    }
    Ok(hash_joined(&matched))
}

/// Wraps a matcher failure in the error that names the probe it came from.
/// [`crate::ast`] knows what went wrong; only this layer knows whose it was.
fn ast_error(name: &str, failure: AstFailure) -> Error {
    Error::ProbeAst {
        name: name.to_owned(),
        source: Box::new(failure),
    }
}

/// Hashes a selection's values joined one per line. Shared so a probe's digest
/// cannot come to depend on which selector produced the values.
fn hash_joined(values: &[Vec<u8>]) -> String {
    let mut joined = Vec::new();
    for value in values {
        joined.extend_from_slice(value);
        joined.push(b'\n');
    }
    hashing::hash_bytes(&joined)
}

/// Whether a selection measured nothing: no outputs at all, or the single
/// output `null`. See `probe.rs`'s module docs for why `false` is not in this
/// list.
fn selected_nothing(selected: &[Vec<u8>]) -> bool {
    match selected {
        [] => true,
        [only] => only.as_slice() == b"null",
        _ => false,
    }
}
