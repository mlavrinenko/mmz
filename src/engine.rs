//! Orchestration: discover the manifest, match the command, and either skip a
//! fresh command or run it and record the outcome.
//!
//! Fail-closed by default. A missing or unparseable manifest always errors. The
//! runtime cases — no matching rule, an empty input set — error too unless the
//! manifest's `strict` list relaxes them, in which case they fall back to
//! running the command unmemoized. mmz never wrongly skips a command it claims
//! is fresh; the asymmetry it protects is silent under-skipping, not loud
//! refusal.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::{Command, ExitStatus};

use crate::clock::Clock;
use crate::error::{Error, Result};
use crate::manifest::{Command as Rule, Manifest, StrictCase};
use crate::{cache, hashing, notice, outputs, parametric, probe, resolve};

/// A rule's resolved input digest and the probe digests that fed it.
///
/// The record stores both: a stale verdict can only name the probe that moved
/// when the last run's per-probe digests sit beside its input digest.
struct Digested {
    digest: String,
    probes: BTreeMap<String, String>,
}

/// One invocation's ambient facts: the command line, the directory it runs in,
/// and the clock any record it writes is stamped from.
///
/// Bundled rather than passed as three parameters because they are constant for
/// the whole run and travel together down every path through the engine — and
/// because the clock in particular must be the one value [`run`] resolved, not
/// something a callee could be tempted to read for itself.
struct Invocation<'a> {
    argv: &'a [String],
    cwd: &'a Path,
    clock: Clock,
}

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
/// empty, [`Error::InvalidNow`] if `MMZ_NOW` is set to something that is not a
/// Unix epoch, or [`Error::Spawn`] if the command cannot be launched.
pub fn run(argv: &[String], cwd: &Path) -> Result<u8> {
    // Resolved before anything else, and once: a record this invocation writes
    // is stamped from it, and a malformed pin is a misconfigured invocation —
    // caught before the wrapped command runs rather than after, when the stamp
    // would be all that is left to refuse.
    let call = Invocation {
        argv,
        cwd,
        clock: Clock::resolve()?,
    };
    let located = Manifest::locate(cwd)?;
    let manifest = &located.manifest;
    let base = located.root.as_path();
    let matches = parametric::resolve_matches(manifest, base, argv)?;
    parametric::detect_collision(&matches)?;
    match matches.first() {
        Some(hit) => memoized(manifest, hit, base, &call),
        None => no_match(manifest, &call),
    }
}

/// Handles an unmatched command: error under `no_match` strictness, else run.
fn no_match(manifest: &Manifest, call: &Invocation) -> Result<u8> {
    if manifest.strict.enforces(StrictCase::NoMatch) {
        return Err(Error::NoMatch {
            command: call.argv.join(" "),
        });
    }
    log::debug!("mmz: no rule matches; running unmemoized");
    exec(call)
}

/// Memoizes a matched expansion: skip when fresh, otherwise run and record. The
/// cache identity is the expansion's concrete name; a parametric expansion also
/// folds its bound file into the inputs, so the record busts on that file alone.
fn memoized(
    manifest: &Manifest,
    hit: &parametric::Match,
    base: &Path,
    call: &Invocation,
) -> Result<u8> {
    let identity = hit.exp.identity.as_str();
    let rule = hit.rule;
    let Some(resolved) = digest_inputs(manifest, rule, hit.exp.file.as_deref(), base)? else {
        if manifest.strict.enforces(StrictCase::NoInputs) {
            return Err(Error::NoInputs {
                rule: identity.to_owned(),
            });
        }
        log::warn!("mmz: `{identity}` matched no input files; running unmemoized");
        return exec(call);
    };
    let digest = resolved.digest;
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
    let code = exec(call)?;
    confirm_outputs(rule, identity, base, code)?;
    cache::write(
        &cache_dir,
        identity,
        call.clock,
        &cache::Outcome {
            digest: &digest,
            ok: code == 0,
            outputs: &rule.outputs,
            probes: resolved.probes,
        },
    );
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

/// Resolves a rule's whole input set — its scopes, its probes, plus an optional
/// bound file for a parametric expansion — to a content digest, or `None` when
/// nothing resolves at all. A glob, probe, or I/O failure propagates
/// (fail-closed), so a probe that failed stops the run before the command is
/// spawned and before any record is written.
///
/// A rule that names only probes still has inputs: `None` means no files *and*
/// no probes, never "no files".
fn digest_inputs(
    manifest: &Manifest,
    rule: &Rule,
    extra: Option<&str>,
    base: &Path,
) -> Result<Option<Digested>> {
    let probes = probe::Resolver::new(manifest, base).for_rule(rule)?;
    let groups = manifest.glob_groups(rule)?;
    let mut files = resolve::expand_groups(&groups, base)?;
    if let Some(file) = extra {
        files.push(file.to_owned());
        files.sort();
        files.dedup();
    }
    if files.is_empty() && probes.is_empty() {
        return Ok(None);
    }
    Ok(Some(Digested {
        digest: hashing::digest_with(base, &files, &probes)?,
        probes,
    }))
}

/// Spawns the command with inherited stdio and returns its exit code.
fn exec(call: &Invocation) -> Result<u8> {
    let Some((program, rest)) = call.argv.split_first() else {
        return Err(Error::EmptyCommand);
    };
    let status = Command::new(program)
        .args(rest)
        .current_dir(call.cwd)
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
