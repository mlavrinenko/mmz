//! In-process jq selection: the engine behind a probe's `json:` key.
//!
//! A probe with `json:` never spawns anything to reach its value. mmz reads the
//! bytes — from a `file:` or from a `run:` line's stdout — parses them once,
//! runs the jq program against them, and renders the outputs back to bytes for
//! the hasher. jq is the language rather than a narrower path syntax because
//! this repo's own probes already use `,` and
//! `with_entries(select(...))`: a selector spelling that could not express them
//! would have to change meaning in a later version, and a manifest key's
//! semantics must not break under a reader.
//!
//! # Why the rendering is canonical, not the input's bytes
//!
//! [`render`] sorts object keys on the way out. That is deliberate and it is
//! the point of parsing at all: a probe piping a tool through `jq` hashes the
//! bytes some renderer chose, so object key order — a thing the renderer is
//! free to move between versions — silently becomes an input the probe never
//! meant to declare. It bit this repo for real, which is why every shelled-out
//! probe here carries `jq -S`. A `json:` probe cannot forget, because mmz does
//! the rendering.
//!
//! Arrays keep their order: that is content, not presentation. Numbers are
//! rendered by jaq's own writer, so an integer stays an integer.
//!
//! # What this module does not decide
//!
//! Whether an empty selection is an error, and which mmz error a failure
//! becomes, both belong to [`crate::probe`] — this module reports what went
//! wrong ([`Failure`]) and leaves the policy there. Nothing jq-shaped escapes
//! it: callers hand in bytes and a program, and get bytes back.

use jaq_core::load::{Arena, File, Loader};
use jaq_core::{Compiler, Ctx, Vars, data, unwrap_valr};
use jaq_json::write::Pp;
use jaq_json::{Val, read};

/// Where a selection went wrong, so [`crate::probe`] can name the right thing
/// in the message: the manifest's program, or the bytes it was pointed at.
#[derive(Debug)]
pub(crate) enum Failure {
    /// The bytes are not one JSON value — an empty stdout, a tool that printed
    /// a log line before its JSON, a truncated file.
    Input(String),
    /// The `json:` program is not valid jq, or names a filter jaq does not
    /// define. A manifest defect, not a state of the world.
    Program(String),
    /// The program compiled and then raised while running — a type error like
    /// `.a` applied to a number, or an explicit `error`.
    Run(String),
}

/// Runs `program` over `input`, returning one canonical rendering per output
/// value.
///
/// The result is a list rather than one blob because "how many values did this
/// select" is the question [`crate::probe`] has to answer to refuse a selector
/// that matched nothing, and joining first would throw it away.
///
/// `input` must be exactly one JSON value (trailing whitespace aside). A stream
/// of values is refused rather than folded together: mmz would have to invent a
/// rule for how a filter maps over the stream, and every probe this feature
/// exists for reads a single document.
pub(crate) fn select(program: &str, input: &[u8]) -> Result<Vec<Vec<u8>>, Failure> {
    let value = read::parse_single(input).map_err(|err| Failure::Input(err.to_string()))?;
    let defs = jaq_core::defs()
        .chain(jaq_std::defs())
        .chain(jaq_json::defs());
    let funs = jaq_core::funs()
        .chain(jaq_std::funs())
        .chain(jaq_json::funs());
    let arena = Arena::default();
    let file = File {
        code: program,
        path: (),
    };
    let modules = Loader::new(defs)
        .load(&arena, file)
        .map_err(|errs| Failure::Program(load_errors(&errs)))?;
    let filter = Compiler::default()
        .with_funs(funs)
        .compile(modules)
        .map_err(|errs| Failure::Program(compile_errors(&errs)))?;

    let ctx = Ctx::<data::JustLut<Val>>::new(&filter.lut, Vars::new([]));
    let mut rendered = Vec::new();
    for output in filter.id.run((ctx, value)).map(unwrap_valr) {
        let value = output.map_err(|err| Failure::Run(err.to_string()))?;
        rendered.push(render(&value));
    }
    Ok(rendered)
}

/// One value as canonical JSON: compact, object keys sorted, arrays left in
/// order. See the module docs for why the sort is not optional.
fn render(value: &Val) -> Vec<u8> {
    let pp = Pp::<String> {
        indent: None,
        sort_keys: true,
        sep_space: false,
        styles: Pp::default().styles,
    };
    let mut out = Vec::new();
    // The writer is infallible over a Vec<u8>, which cannot fail to grow short
    // of an allocation failure; there is no error state to report upward.
    if jaq_json::write::write(&mut out, &pp, 0, value).is_err() {
        return Vec::new();
    }
    out
}

/// Renders the loader's lex/parse errors as one line each: what jaq expected
/// and what it found instead, which is the pair a manifest author needs to fix
/// the program.
fn load_errors(errors: &jaq_core::load::Errors<&str, ()>) -> String {
    use jaq_core::load::Error;
    let mut lines: Vec<String> = Vec::new();
    for (_file, error) in errors {
        match error {
            Error::Io(errs) => lines.extend(
                errs.iter()
                    .map(|(what, err)| format!("cannot read `{what}`: {err}")),
            ),
            Error::Lex(errs) => lines.extend(
                errs.iter()
                    .map(|(expect, found)| expected(expect.as_str(), found)),
            ),
            Error::Parse(errs) => lines.extend(
                errs.iter()
                    .map(|(expect, found)| expected(expect.as_str(), found)),
            ),
        }
    }
    join(&lines)
}

/// Renders the compiler's undefined-symbol errors — the shape a typo'd filter
/// name takes, which is by far the likeliest way a `json:` program fails to
/// compile.
fn compile_errors(errors: &jaq_core::compile::Errors<&str, ()>) -> String {
    let lines: Vec<String> = errors
        .iter()
        .flat_map(|(_file, errs)| errs.iter())
        .map(|(name, undefined)| format!("undefined {} `{name}`", undefined.as_str()))
        .collect();
    join(&lines)
}

/// `expected X, found Y`, with an empty `found` spelled out rather than left as
/// a dangling backtick pair at the end of the line.
fn expected(what: &str, found: &str) -> String {
    if found.is_empty() {
        return format!("expected {what}, found end of program");
    }
    format!("expected {what}, found `{found}`")
}

/// Joins rendered diagnostics, naming the silent case explicitly so a message
/// never trails off into nothing.
fn join(lines: &[String]) -> String {
    if lines.is_empty() {
        return "the program could not be compiled".to_owned();
    }
    lines.join("; ")
}

#[cfg(test)]
#[path = "json_tests.rs"]
mod tests;
