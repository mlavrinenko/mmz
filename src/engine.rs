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
use crate::{cache, hashing, notice, parametric, resolve};

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
        if cached.ok && cached.digest == digest {
            log::info!("mmz: skip `{identity}` (inputs unchanged)");
            announce_hit(manifest, rule, &cached);
            return Ok(0);
        }
    }
    let code = exec(argv, cwd)?;
    cache::write(&cache_dir, identity, &digest, code == 0);
    Ok(code)
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
mod tests {
    use super::run;

    fn write_manifest(dir: &std::path::Path, body: &str) {
        let cfg = dir.join(".mmz");
        std::fs::create_dir_all(&cfg).expect("mkdir .mmz");
        std::fs::write(cfg.join("config.yaml"), body).expect("write manifest");
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

    fn run_twice(base: &std::path::Path) {
        let argv = ["sh".to_owned(), "-c".to_owned(), "exit 0".to_owned()];
        assert_eq!(run(&argv, base).expect("first run records"), 0);
        assert_eq!(run(&argv, base).expect("second run is a hit"), 0);
    }

    #[test]
    fn on_hit_paths_are_exercised_on_a_cache_hit() {
        // Per-command on_hit (overrides absent global) with a macro: the hit path
        // resolves and prints the notice.
        let rule = tempfile::tempdir().expect("tempdir");
        std::fs::write(rule.path().join("a.txt"), b"one").expect("input");
        write_manifest(
            rule.path(),
            "scopes:\n  src: [\"*.txt\"]\ncommands:\n  - name: sh\n    inputs: [src]\n    on_hit: \"skip {cache:command}\"\n",
        );
        run_twice(rule.path());

        // An empty global on_hit suppresses the line without erroring.
        let blank = tempfile::tempdir().expect("tempdir");
        std::fs::write(blank.path().join("a.txt"), b"one").expect("input");
        write_manifest(
            blank.path(),
            "scopes:\n  src: [\"*.txt\"]\non_hit: \"\"\ncommands:\n  - name: sh\n    inputs: [src]\n",
        );
        run_twice(blank.path());
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

    /// A parametric rule fanned over `src/**/*.rs`. The executed command appends
    /// a byte to `<file>.ran` via sh's `$1`, so a run is observable while a
    /// cache hit leaves the counter untouched.
    fn write_fan(base: &std::path::Path) {
        std::fs::create_dir_all(base.join("src")).expect("mkdir src");
        std::fs::write(base.join("src/a.rs"), b"a").expect("a");
        std::fs::write(base.join("src/b.rs"), b"b").expect("b");
        write_manifest(
            base,
            "scopes:\n  targets: [\"src/**/*.rs\"]\ncommands:\n  - name: 'sh -c echo>>\"$1\".ran sh {targets}'\n",
        );
    }

    fn fan_argv(file: &str) -> [String; 5] {
        [
            "sh".to_owned(),
            "-c".to_owned(),
            "echo>>\"$1\".ran".to_owned(),
            "sh".to_owned(),
            file.to_owned(),
        ]
    }

    fn ran_count(base: &std::path::Path, file: &str) -> usize {
        std::fs::read(base.join(format!("{file}.ran"))).map_or(0, |bytes| bytes.len())
    }

    #[test]
    fn parametric_rule_busts_only_its_own_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let base = dir.path();
        write_fan(base);

        assert_eq!(run(&fan_argv("src/a.rs"), base).expect("run a"), 0);
        assert_eq!(ran_count(base, "src/a.rs"), 1, "a ran once");
        assert_eq!(run(&fan_argv("src/a.rs"), base).expect("run a"), 0);
        assert_eq!(ran_count(base, "src/a.rs"), 1, "a's second run is a hit");

        // b has its own record, unaffected by a.
        assert_eq!(run(&fan_argv("src/b.rs"), base).expect("run b"), 0);
        assert_eq!(ran_count(base, "src/b.rs"), 1, "b ran once");
        assert_eq!(ran_count(base, "src/a.rs"), 1, "a untouched by b");

        // Editing b busts only b; a stays fresh (tight per-file scoping).
        std::fs::write(base.join("src/b.rs"), b"edited").expect("edit b");
        assert_eq!(run(&fan_argv("src/a.rs"), base).expect("run a"), 0);
        assert_eq!(
            ran_count(base, "src/a.rs"),
            1,
            "sibling edit did not bust a"
        );
        assert_eq!(run(&fan_argv("src/b.rs"), base).expect("run b"), 0);
        assert_eq!(ran_count(base, "src/b.rs"), 2, "b's own edit re-ran it");
    }

    #[test]
    fn off_domain_file_falls_through_to_no_match() {
        let dir = tempfile::tempdir().expect("tempdir");
        let base = dir.path();
        write_fan(base);
        // A .txt path is not in the `src/**/*.rs` domain, so no rule matches.
        let argv = fan_argv("src/c.txt");
        assert!(
            matches!(run(&argv, base), Err(crate::Error::NoMatch { .. })),
            "an off-domain file does not fan a record"
        );
    }

    #[test]
    fn colliding_parametric_expansions_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        let base = dir.path();
        std::fs::write(base.join("a.rs"), b"x").expect("a");
        write_manifest(
            base,
            "scopes:\n  wide: [\"*.rs\"]\n  narrow: [\"a.rs\"]\ncommands:\n  - name: \"do {wide}\"\n  - name: \"do {narrow}\"\n",
        );
        let argv = ["do".to_owned(), "a.rs".to_owned()];
        assert!(
            matches!(
                run(&argv, base),
                Err(crate::Error::CollidingIdentity { .. })
            ),
            "two rules claiming `do a.rs` is a loud error, not a silent winner"
        );
    }

    #[test]
    fn malformed_macro_is_a_config_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        let base = dir.path();
        write_manifest(base, "commands:\n  - name: \"do {a} {b}\"\n");
        let argv = ["do".to_owned(), "x".to_owned()];
        assert!(
            matches!(run(&argv, base), Err(crate::Error::MacroSyntax { .. })),
            "a two-macro name is rejected at load"
        );
    }
}
