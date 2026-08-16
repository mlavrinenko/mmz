//! Orchestration: discover the manifest, match the command, and either skip a
//! fresh command or run it and record the outcome.
//!
//! Fail-closed by default. A missing or unparseable manifest always errors. The
//! runtime cases — no matching rule, an empty input set — error too unless the
//! manifest's `strict` list relaxes them, in which case they fall back to
//! running the command unmemoized. mmz never wrongly skips a command it claims
//! is fresh; the asymmetry it protects is silent under-skipping, not loud
//! refusal.

use std::path::Path;
use std::process::{Command, ExitStatus};

use crate::error::{Error, Result};
use crate::manifest::{Command as Rule, Manifest, StrictCase};
use crate::{cache, hashing, notice, outputs, parametric, resolve};

/// Runs `argv` (a program and its arguments) with memoization, from `cwd`.
///
/// Returns the exit code to propagate. Input globs resolve relative to the
/// manifest's directory; the command itself runs in `cwd` with inherited stdio.
///
/// # Errors
///
/// Returns [`Error::NoManifest`] when no manifest is found, a manifest error
/// when one cannot be loaded, [`Error::NoMatch`] / [`Error::NoInputs`] when the
/// relevant strict case is enforced, [`Error::EmptyCommand`] if `argv` is
/// empty, or [`Error::Spawn`] if the command cannot be launched.
pub fn run(argv: &[String], cwd: &Path) -> Result<u8> {
    let located = Manifest::locate(cwd)?;
    let manifest = &located.manifest;
    let base = located.root.as_path();
    let matches = parametric::resolve_matches(manifest, base, argv)?;
    parametric::detect_collision(&matches)?;
    match matches.first() {
        Some(hit) => memoized(manifest, hit, base, argv, cwd),
        None => no_match(manifest, argv, cwd),
    }
}

/// Handles an unmatched command: error under `no_match` strictness, else run.
fn no_match(manifest: &Manifest, argv: &[String], cwd: &Path) -> Result<u8> {
    if manifest.strict.enforces(StrictCase::NoMatch) {
        return Err(Error::NoMatch {
            command: argv.join(" "),
        });
    }
    log::debug!("mmz: no rule matches; running unmemoized");
    exec(argv, cwd)
}

/// Memoizes a matched expansion: skip when fresh, otherwise run and record. The
/// cache identity is the expansion's concrete name; a parametric expansion also
/// folds its bound file into the inputs, so the record busts on that file alone.
fn memoized(
    manifest: &Manifest,
    hit: &parametric::Match,
    base: &Path,
    argv: &[String],
    cwd: &Path,
) -> Result<u8> {
    let identity = hit.exp.identity.as_str();
    let rule = hit.rule;
    let Some(digest) = digest_inputs(manifest, rule, hit.exp.file.as_deref(), base)? else {
        if manifest.strict.enforces(StrictCase::NoInputs) {
            return Err(Error::NoInputs {
                rule: identity.to_owned(),
            });
        }
        log::warn!("mmz: `{identity}` matched no input files; running unmemoized");
        return exec(argv, cwd);
    };
    let cache_dir = base.join(&manifest.cache_dir);
    if let Some(cached) = cache::read(&cache_dir, identity) {
        let voided = outputs::first_missing(base, &rule.outputs);
        if cached.ok && cached.digest == digest && voided.is_none() {
            log::info!("mmz: skip `{identity}` (inputs unchanged)");
            announce_hit(manifest, rule, &cached);
            return Ok(0);
        }
        if let Some(path) = voided {
            log::info!("mmz: `{identity}` declared output `{path}` is missing; the record is void");
        }
    }
    let code = exec(argv, cwd)?;
    confirm_outputs(rule, identity, base, code)?;
    cache::write(&cache_dir, identity, &digest, code == 0, &rule.outputs);
    Ok(code)
}

/// Confirms a successful run produced every artifact its rule declared.
///
/// A command that exits 0 without writing its output is a hard error, not a
/// silent skip of the record: recording it would claim an artifact that is not
/// there, and recording nothing at all would leave a rule that quietly never
/// hits again. A failing run is left alone — its own exit code is the story,
/// and its record is written as a failure exactly as before.
///
/// # Errors
///
/// Returns [`Error::MissingOutput`] naming the first missing artifact.
fn confirm_outputs(rule: &Rule, identity: &str, base: &Path, code: u8) -> Result<()> {
    if code != 0 {
        return Ok(());
    }
    match outputs::first_missing(base, &rule.outputs) {
        Some(path) => Err(Error::MissingOutput {
            rule: identity.to_owned(),
            path,
        }),
        None => Ok(()),
    }
}

/// Prints the resolved cache-hit notice to stderr, if one is configured. A
/// rule's own `on_hit` overrides the manifest default; an empty template at
/// either level suppresses the line. The notice goes to stderr so it never
/// pollutes a pipeline reading the wrapped command's stdout.
fn announce_hit(manifest: &Manifest, rule: &Rule, cached: &cache::Cached) {
    let Some(template) = rule.on_hit.as_deref().or(manifest.on_hit.as_deref()) else {
        return;
    };
    if template.is_empty() {
        return;
    }
    eprintln!("{}", notice::expand(template, &cached.fields));
}

/// Resolves a rule's scopes (plus an optional bound file for a parametric
/// expansion) to a content digest, or `None` when nothing resolves on disk. A
/// glob or I/O failure propagates (fail-closed).
fn digest_inputs(
    manifest: &Manifest,
    rule: &Rule,
    extra: Option<&str>,
    base: &Path,
) -> Result<Option<String>> {
    let groups = manifest.glob_groups(rule)?;
    let mut files = resolve::expand_groups(&groups, base)?;
    if let Some(file) = extra {
        files.push(file.to_owned());
        files.sort();
        files.dedup();
    }
    if files.is_empty() {
        return Ok(None);
    }
    Ok(Some(hashing::digest_files(base, &files)?))
}

/// Spawns the command with inherited stdio and returns its exit code.
fn exec(argv: &[String], cwd: &Path) -> Result<u8> {
    let Some((program, rest)) = argv.split_first() else {
        return Err(Error::EmptyCommand);
    };
    let status = Command::new(program)
        .args(rest)
        .current_dir(cwd)
        .status()
        .map_err(|source| Error::Spawn {
            program: program.clone(),
            source,
        })?;
    Ok(exit_code(status))
}

/// Maps an [`ExitStatus`] to a propagatable code. A signal death or an
/// out-of-range code both become `1`.
fn exit_code(status: ExitStatus) -> u8 {
    let code = status.code().unwrap_or(1);
    u8::try_from(code).unwrap_or(1)
}

#[cfg(test)]
#[path = "engine_tests.rs"]
mod tests;
