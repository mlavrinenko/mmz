//! Thin entry point: parse argv, init logging, dispatch to the engine or an
//! `mmz --…` action, and map errors to exit codes.

use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use mmz::error::Error;

/// This binary's version, embedded at build time from `Cargo.toml`.
const VERSION: &str = env!("CARGO_PKG_VERSION");

const USAGE: &str = concat!(
    "mmz ",
    env!("CARGO_PKG_VERSION"),
    " — memoized command runner

Usage:
    mmz <command> [args...]           run a command, skipping it when its inputs are unchanged
    mmz --init                        write a starter .mmz/config.yaml in the current directory
    mmz --status [--tag t]...         show each rule's freshness as a table
    mmz --status=json [--tag t]...    the same as JSON, with each rule's inputs and hashes
    mmz --status=json-schema          print the JSON Schema for --status=json
    mmz --is-fresh [--tag t]... [-- cmd]
                                      exit 0 if cmd's rule (or every/tagged rule) is fresh; runs nothing
    mmz --prune                       delete cache records whose rule no longer exists
    mmz --schema                      print the config JSON Schema
    mmz --schema=fragment             print the JSON Schema for an imported fragment
    mmz --dump-config                 print the merged manifest and where each entry came from
    mmz --dump-config=json            the same as JSON
    mmz --version                     print version
    mmz --help                        print this help
    mmz -- <command> [args]           run a command whose name begins with a dash

Config lives in .mmz/config.yaml (nearest one, searching upward). mmz errors when no
manifest is found, the manifest is invalid, no rule matches, or a matched rule
has no inputs; relax the last two per project with the `strict` list.

`--tag`/`-t <tag>` (repeatable) narrows --is-fresh/--status to rules carrying
every listed tag (AND, not OR); untagged rules never match. Combining --tag
with a targeted command is a usage error — a command already resolves to one rule.
A --is-fresh whose selection holds no rule is refused (exit 7) rather than passing
on the strength of having checked nothing; --status reports it and exits 0.

Environment:
    MMZ_NOW    pin \"now\" to a Unix epoch in seconds, so a record's ran_at and
               the AGE column read the same on every run. A malformed value is
               refused, never ignored. Unset, mmz reads the system clock.

Exit codes:
    0    fresh, skipped, or succeeded        6    a probe failed, could not run, or
    1    --is-fresh: not fresh                    printed nothing (nothing recorded)
    2    usage error                         7    --is-fresh: nothing to gate (the
    3    strict refusal (no rule / inputs)        selection holds no rule)
    4    manifest missing or invalid         70   internal error
    5    declared output missing after a     127  command could not be spawned
         successful run (nothing recorded)
    otherwise the wrapped command's own exit code"
);

fn main() -> ExitCode {
    env_logger::init();
    let args: Vec<String> = std::env::args().skip(1).collect();
    run_cli(&args)
}

/// Splits mmz's own actions from a wrapped command. The first token decides:
/// `--` forces the rest to be a command, a recognized `--action` runs mmz
/// itself, anything else is the command to memoize.
fn run_cli(args: &[String]) -> ExitCode {
    let Some((first, rest)) = args.split_first() else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    if first == "--" {
        return wrap(rest);
    }
    if let Some(code) = action(first, rest) {
        return code;
    }
    wrap(args)
}

/// Dispatches a recognized `mmz --…` action. Returns `None` when `first` is the
/// start of a wrapped command rather than an mmz action.
fn action(first: &str, rest: &[String]) -> Option<ExitCode> {
    match first {
        "--version" | "-V" => Some(meta(rest, &format!("mmz {VERSION}\n"))),
        "--help" | "-h" => Some(meta(rest, &format!("{USAGE}\n"))),
        schema if schema == "--schema" || schema.starts_with("--schema=") => {
            Some(run_schema(schema, rest))
        }
        dump if dump == "--dump-config" || dump.starts_with("--dump-config=") => {
            Some(run_dump_config(dump, rest))
        }
        "--init" => Some(run_init(rest)),
        "--prune" => Some(run_prune(rest)),
        "--is-fresh" => Some(run_is_fresh(rest)),
        status if status == "--status" || status.starts_with("--status=") => {
            Some(run_status(status, rest))
        }
        other if other.starts_with('-') => Some(unknown_option(other)),
        _ => None,
    }
}

/// Emits a zero-argument meta action's output, or a usage error if given extras.
fn meta(rest: &[String], text: &str) -> ExitCode {
    if rest.is_empty() {
        emit(text)
    } else {
        usage("this option takes no arguments")
    }
}

fn run_init(rest: &[String]) -> ExitCode {
    if !rest.is_empty() {
        return usage("`--init` takes no arguments");
    }
    let cwd = match current_dir() {
        Ok(dir) => dir,
        Err(code) => return code,
    };
    match mmz::init::init(&cwd) {
        Ok(path) => emit(&format!("wrote {}\n", path.display())),
        Err(err) => report_error(&err),
    }
}

fn run_prune(rest: &[String]) -> ExitCode {
    if !rest.is_empty() {
        return usage("`--prune` takes no arguments");
    }
    let cwd = match current_dir() {
        Ok(dir) => dir,
        Err(code) => return code,
    };
    match mmz::prune::prune(&cwd) {
        Ok(text) => emit(&text),
        Err(err) => report_error(&err),
    }
}

/// Handles `--schema` and `--schema=fragment`. `arg` is the full token, so its
/// `=suffix` selects which document prints: the config manifest schema, or
/// the narrower schema for a file its `imports:` names — see
/// `mmz::schema` for how the two are kept from drifting apart.
fn run_schema(arg: &str, rest: &[String]) -> ExitCode {
    if !rest.is_empty() {
        return usage("`--schema` takes no arguments");
    }
    let format = arg.strip_prefix("--schema").unwrap_or("");
    match format {
        "" => emit(mmz::schema::SCHEMA),
        "=fragment" => emit(mmz::schema::FRAGMENT_SCHEMA),
        other => usage(&format!(
            "unknown `--schema` format `{}`; use fragment, or omit the suffix for the config schema",
            other.trim_start_matches('=')
        )),
    }
}

/// Handles `--dump-config` and `--dump-config=json`. `arg` is the full token,
/// so its `=suffix` selects the rendering. There is no `=json-schema` arm —
/// unlike `--schema` and `--status`, this document's only consumer today is a
/// gate that can assert on keys directly (see `mmz::dump`), so a schema for
/// it is deferred until a second consumer needs one.
fn run_dump_config(arg: &str, rest: &[String]) -> ExitCode {
    if !rest.is_empty() {
        return usage("`--dump-config` takes no arguments");
    }
    let cwd = match current_dir() {
        Ok(dir) => dir,
        Err(code) => return code,
    };
    let format = arg.strip_prefix("--dump-config").unwrap_or("");
    let rendered = match format {
        "" => mmz::dump::dump(&cwd),
        "=json" => mmz::dump::dump_json(&cwd),
        other => {
            return usage(&format!(
                "unknown `--dump-config` format `{}`; use json, or omit the suffix for the human form",
                other.trim_start_matches('=')
            ));
        }
    };
    match rendered {
        Ok(text) => emit(&text),
        Err(err) => report_error(&err),
    }
}

/// Handles `--status`, `--status=json`, and `--status=json-schema`. `arg` is the
/// full token, so its `=suffix` selects the rendering.
fn run_status(arg: &str, rest: &[String]) -> ExitCode {
    let (tags, rest) = match parse_tags(rest) {
        Ok(parsed) => parsed,
        Err(code) => return code,
    };
    if !rest.is_empty() {
        return usage("`--status` takes no arguments");
    }
    let format = arg.strip_prefix("--status").unwrap_or("");
    match format {
        "=json-schema" => return emit(mmz::status::SCHEMA),
        "" | "=json" => {}
        other => {
            return usage(&format!(
                "unknown `--status` format `{}`; use json or json-schema",
                other.trim_start_matches('=')
            ));
        }
    }
    let cwd = match current_dir() {
        Ok(dir) => dir,
        Err(code) => return code,
    };
    let rendered = if format == "=json" {
        mmz::status::report_json(&cwd, &tags)
    } else {
        mmz::status::report(&cwd, &tags)
    };
    match rendered {
        Ok(text) => emit(&text),
        Err(err) => report_error(&err),
    }
}

/// Handles `mmz --is-fresh [--tag t]... [-- <command>]`: assert freshness
/// without running. A bare `--is-fresh` gates every rule; a `--tag` filter
/// (repeatable, `ANDed`) narrows that to rules carrying every listed tag; a
/// trailing command (optionally behind `--`, to allow a leading dash) gates
/// the one rule it matches — combining a command with `--tag` is a library
/// usage error. Exit 0 when fresh, 1 when not, or a library error's code.
fn run_is_fresh(rest: &[String]) -> ExitCode {
    let cwd = match current_dir() {
        Ok(dir) => dir,
        Err(code) => return code,
    };
    let (tags, rest) = match parse_tags(rest) {
        Ok(parsed) => parsed,
        Err(code) => return code,
    };
    let argv = strip_separator(rest);
    let target = if argv.is_empty() { None } else { Some(argv) };
    match mmz::freshness::evaluate(&cwd, target, &tags) {
        Ok(verdicts) => report_freshness(&verdicts),
        Err(err) => report_error(&err),
    }
}

/// Peels leading `-t`/`--tag <value>` pairs off `rest`, one tag per occurrence
/// (repeats AND together — a rule must carry every listed tag). Stops at the
/// first token that isn't a tag flag, leaving the remainder for
/// [`strip_separator`] to resolve into the wrapped command.
fn parse_tags(rest: &[String]) -> Result<(Vec<String>, &[String]), ExitCode> {
    let mut tags = Vec::new();
    let mut cursor = rest;
    while let Some((flag, tail)) = cursor.split_first() {
        if flag != "-t" && flag != "--tag" {
            break;
        }
        let Some((value, after)) = tail.split_first() else {
            return Err(usage(&format!("`{flag}` requires a value")));
        };
        tags.push(value.clone());
        cursor = after;
    }
    Ok((tags, cursor))
}

/// Drops a leading `--` separator, so `--is-fresh -- just check` and
/// `--is-fresh just check` both name the command `just check`.
fn strip_separator(rest: &[String]) -> &[String] {
    match rest.split_first() {
        Some((first, tail)) if first == "--" => tail,
        _ => rest,
    }
}

/// Reports a freshness gate: each not-fresh rule on stderr, then exit 0 when all
/// are fresh, 1 otherwise. The stale lines name the rule and why it would re-run,
/// and a single remediation hint follows them when any non-fresh verdict is
/// remediable, since mmz only observes a command it wraps — a standalone pass is
/// not tracked and leaves the rule stale or `never`.
fn report_freshness(verdicts: &[mmz::freshness::Verdict]) -> ExitCode {
    let mut fresh = true;
    let mut any_remediable = false;
    for verdict in verdicts {
        if verdict.is_fresh() {
            continue;
        }
        fresh = false;
        let reason = verdict.reason().unwrap_or_else(|| "not fresh".to_owned());
        eprintln!("mmz: `{}` is {} ({reason})", verdict.rule, verdict.state());
        any_remediable |= verdict.is_remediable();
    }
    if fresh {
        return ExitCode::SUCCESS;
    }
    if any_remediable {
        eprintln!(
            "mmz: re-run each listed command under mmz (e.g. `mmz just check`) to record a pass — a standalone run is not tracked"
        );
    }
    ExitCode::from(1)
}

/// Writes `text` to stdout, treating a closed pipe (e.g. `mmz --schema | head`)
/// as success rather than panicking, since `unsafe` SIGPIPE reset is disallowed.
fn emit(text: &str) -> ExitCode {
    let mut out = std::io::stdout().lock();
    match out.write_all(text.as_bytes()).and_then(|()| out.flush()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) if err.kind() == std::io::ErrorKind::BrokenPipe => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("mmz: cannot write to stdout: {err}");
            ExitCode::from(70)
        }
    }
}

/// Runs `argv` through the memoization engine, mapping its result to a code.
fn wrap(argv: &[String]) -> ExitCode {
    let cwd = match current_dir() {
        Ok(dir) => dir,
        Err(code) => return code,
    };
    match mmz::run(argv, &cwd) {
        Ok(code) => ExitCode::from(code),
        Err(err) => report_error(&err),
    }
}

fn current_dir() -> Result<PathBuf, ExitCode> {
    std::env::current_dir().map_err(|err| {
        eprintln!("mmz: cannot determine working directory: {err}");
        ExitCode::from(70)
    })
}

fn unknown_option(opt: &str) -> ExitCode {
    eprintln!(
        "mmz: unknown option `{opt}`; run `mmz --help`, or `mmz -- {opt} …` to wrap a command starting with a dash"
    );
    ExitCode::from(2)
}

fn usage(message: &str) -> ExitCode {
    eprintln!("mmz: {message}");
    ExitCode::from(2)
}

fn report_error(err: &Error) -> ExitCode {
    eprintln!("mmz: {err}");
    ExitCode::from(exit_for(err))
}

/// Maps a library error to its documented exit code.
fn exit_for(err: &Error) -> u8 {
    match err {
        Error::EmptyCommand
        | Error::ManifestExists { .. }
        | Error::TagWithCommand
        | Error::InvalidNow { .. } => 2,
        Error::NoMatch { .. } | Error::NoInputs { .. } => 3,
        Error::NoManifest { .. }
        | Error::ManifestParse { .. }
        | Error::UnknownScope { .. }
        | Error::UnknownInput { .. }
        | Error::NameCollision { .. }
        | Error::EmptyCommandName(_)
        | Error::DuplicateCommand(_)
        | Error::DuplicateTag { .. }
        | Error::InvalidOutput { .. }
        | Error::MacroSyntax { .. }
        | Error::CollidingIdentity { .. }
        | Error::Pattern { .. }
        | Error::ImportMissing { .. }
        | Error::ImportNotReadable { .. }
        | Error::ImportCycle { .. }
        | Error::DuplicateScope { .. }
        | Error::DuplicateProbe { .. }
        | Error::DuplicateCommandAcrossFiles { .. }
        | Error::FragmentPolicyKey { .. }
        | Error::NullPolicyKey { .. }
        | Error::EmptyProbeShell => 4,
        Error::MissingOutput { .. } => 5,
        Error::ProbeFailed { .. } | Error::ProbeSpawn { .. } | Error::ProbeEmpty { .. } => 6,
        Error::NoRules { .. } | Error::NoTaggedRules { .. } | Error::NoExpansions { .. } => 7,
        Error::Spawn { .. } => 127,
        Error::Io(_) | Error::Serialize(_) | Error::Internal(_) => 70,
    }
}
