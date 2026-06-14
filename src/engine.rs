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
use crate::{cache, hashing, matcher, resolve};

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
    let manifest_path = Manifest::discover(cwd).ok_or_else(|| Error::NoManifest {
        start: cwd.to_path_buf(),
    })?;
    let manifest = Manifest::load(&manifest_path)?;
    let base = manifest_path
        .parent()
        .ok_or_else(|| Error::Internal("manifest path has no parent".to_owned()))?;
    match matcher::first_match(&manifest.commands, argv) {
        Some(rule) => memoized(&manifest, rule, base, argv, cwd),
        None => no_match(&manifest, argv, cwd),
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

/// Memoizes a matched rule: skip when fresh, otherwise run and record.
fn memoized(
    manifest: &Manifest,
    rule: &Rule,
    base: &Path,
    argv: &[String],
    cwd: &Path,
) -> Result<u8> {
    let Some(digest) = digest_inputs(manifest, rule, base)? else {
        if manifest.strict.enforces(StrictCase::NoInputs) {
            return Err(Error::NoInputs {
                rule: rule.name.clone(),
            });
        }
        log::warn!(
            "mmz: `{}` matched no input files; running unmemoized",
            rule.name
        );
        return exec(argv, cwd);
    };
    if cache::is_fresh(base, &rule.name, &digest) {
        log::info!("mmz: skip `{}` (inputs unchanged)", rule.name);
        return Ok(0);
    }
    let code = exec(argv, cwd)?;
    cache::write(base, &rule.name, &digest, code == 0);
    Ok(code)
}

/// Resolves a rule's scopes to a content digest, or `None` when the rule
/// matches no files on disk. A glob or I/O failure propagates (fail-closed).
fn digest_inputs(manifest: &Manifest, rule: &Rule, base: &Path) -> Result<Option<String>> {
    let globs = manifest.globs_for(rule)?;
    let files = resolve::expand(&globs, base, manifest.gitignore)?;
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
mod tests {
    use super::run;

    fn write_manifest(dir: &std::path::Path, body: &str) {
        std::fs::write(dir.join("mmz.yaml"), body).expect("write manifest");
    }

    #[test]
    fn skips_second_run_when_inputs_unchanged() {
        let dir = tempfile::tempdir().expect("tempdir");
        let base = dir.path();
        std::fs::write(base.join("a.txt"), b"one").expect("input");
        write_manifest(
            base,
            "scopes:\n  src: [\"*.txt\"]\ncommands:\n  - name: sh\n    inputs: [src]\n",
        );

        let argv = [
            "sh".to_owned(),
            "-c".to_owned(),
            "printf x >> runs.log".to_owned(),
        ];
        assert_eq!(run(&argv, base).expect("run"), 0);
        assert_eq!(run(&argv, base).expect("run"), 0);
        assert_eq!(
            std::fs::read(base.join("runs.log")).expect("log").len(),
            1,
            "skipped once"
        );

        std::fs::write(base.join("a.txt"), b"two").expect("rewrite");
        assert_eq!(run(&argv, base).expect("run"), 0);
        assert_eq!(
            std::fs::read(base.join("runs.log")).expect("log").len(),
            2,
            "input change re-runs"
        );
    }

    #[test]
    fn propagates_exit_code_and_reruns_after_failure() {
        let dir = tempfile::tempdir().expect("tempdir");
        let base = dir.path();
        std::fs::write(base.join("a.txt"), b"one").expect("input");
        write_manifest(
            base,
            "scopes:\n  src: [\"*.txt\"]\ncommands:\n  - name: sh\n    inputs: [src]\n",
        );

        let argv = ["sh".to_owned(), "-c".to_owned(), "exit 3".to_owned()];
        assert_eq!(run(&argv, base).expect("run"), 3, "exit code propagates");
        assert_eq!(
            run(&argv, base).expect("run"),
            3,
            "failure was not cached as fresh"
        );
    }

    #[test]
    fn missing_manifest_is_an_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        let argv = ["sh".to_owned(), "-c".to_owned(), "exit 0".to_owned()];
        assert!(
            matches!(run(&argv, dir.path()), Err(crate::Error::NoManifest { .. })),
            "no manifest is fatal, not passthrough"
        );
    }

    #[test]
    fn invalid_manifest_is_an_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        let base = dir.path();
        write_manifest(base, "commands:\n  - name: sh\n    inputs: [ghost]\n");
        let argv = ["sh".to_owned(), "-c".to_owned(), "exit 0".to_owned()];
        assert!(run(&argv, base).is_err(), "invalid manifest is fatal");
    }

    #[test]
    fn no_match_errors_under_strict_but_passes_through_when_relaxed() {
        let dir = tempfile::tempdir().expect("tempdir");
        let base = dir.path();
        let argv = ["sh".to_owned(), "-c".to_owned(), "exit 5".to_owned()];

        write_manifest(base, "commands:\n  - name: cargo\n");
        assert!(
            matches!(run(&argv, base), Err(crate::Error::NoMatch { .. })),
            "strict default errors on no match"
        );

        write_manifest(base, "commands:\n  - name: cargo\nstrict: []\n");
        assert_eq!(run(&argv, base).expect("run"), 5, "relaxed runs unmemoized");
    }

    #[test]
    fn empty_input_set_errors_under_strict() {
        let dir = tempfile::tempdir().expect("tempdir");
        let base = dir.path();
        write_manifest(
            base,
            "scopes:\n  none: [\"*.none\"]\ncommands:\n  - name: sh\n    inputs: [none]\n",
        );
        let argv = ["sh".to_owned(), "-c".to_owned(), "exit 0".to_owned()];
        assert!(
            matches!(run(&argv, base), Err(crate::Error::NoInputs { .. })),
            "strict default errors on empty input set"
        );
    }

    #[test]
    fn empty_input_set_runs_every_time_when_relaxed() {
        let dir = tempfile::tempdir().expect("tempdir");
        let base = dir.path();
        write_manifest(
            base,
            "scopes:\n  none: [\"*.none\"]\ncommands:\n  - name: sh\n    inputs: [none]\nstrict: [no_match]\n",
        );
        let argv = [
            "sh".to_owned(),
            "-c".to_owned(),
            "printf x >> runs.log".to_owned(),
        ];
        assert_eq!(run(&argv, base).expect("run"), 0);
        assert_eq!(run(&argv, base).expect("run"), 0);
        assert_eq!(
            std::fs::read(base.join("runs.log")).expect("log").len(),
            2,
            "relaxed no-inputs never memoizes"
        );
    }
}
