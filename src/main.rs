//! Thin entry point: parse argv, init logging, dispatch to the engine or an
//! `mmz --…` action, and map errors to exit codes.

use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use mmz::error::Error;

const USAGE: &str = "\
mmz — memoized command runner

Usage:
    mmz <command> [args...]   run a command, skipping it when its inputs are unchanged
    mmz --init                write a starter mmz.yaml in the current directory
    mmz --status              show each rule's freshness as a table
    mmz --status=json         the same as JSON, with each rule's inputs and hashes
    mmz --status=json-schema  print the JSON Schema for --status=json
    mmz --schema              print the mmz.yaml JSON Schema
    mmz --version             print version
    mmz --help                print this help
    mmz -- <command> [args]   run a command whose name begins with a dash

Config lives in mmz.yaml (nearest one, searching upward). mmz errors when no
manifest is found, the manifest is invalid, no rule matches, or a matched rule
has no inputs; relax the last two per project with the `strict` list.

Exit codes:
    0    skipped (fresh) or succeeded        4    manifest missing or invalid
    2    usage error                         70   internal error
    3    strict refusal (no rule / inputs)   127  command could not be spawned
    otherwise the wrapped command's own exit code";

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
        "--version" | "-V" => Some(meta(rest, &format!("mmz {}\n", env!("CARGO_PKG_VERSION")))),
        "--help" | "-h" => Some(meta(rest, &format!("{USAGE}\n"))),
        "--schema" => Some(meta(rest, mmz::schema::SCHEMA)),
        "--init" => Some(run_init(rest)),
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

/// Handles `--status`, `--status=json`, and `--status=json-schema`. `arg` is the
/// full token, so its `=suffix` selects the rendering.
fn run_status(arg: &str, rest: &[String]) -> ExitCode {
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
        mmz::status::report_json(&cwd)
    } else {
        mmz::status::report(&cwd)
    };
    match rendered {
        Ok(text) => emit(&text),
        Err(err) => report_error(&err),
    }
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
        Error::EmptyCommand | Error::ManifestExists { .. } => 2,
        Error::NoMatch { .. } | Error::NoInputs { .. } => 3,
        Error::NoManifest { .. }
        | Error::ManifestParse { .. }
        | Error::UnknownScope { .. }
        | Error::EmptyCommandName(_)
        | Error::DuplicateCommand(_)
        | Error::Pattern { .. } => 4,
        Error::Spawn { .. } => 127,
        Error::Io(_) | Error::Serialize(_) | Error::Internal(_) => 70,
    }
}
