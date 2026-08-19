//! Error types for the `mmz` library.

use std::path::PathBuf;

use thiserror::Error;

/// Errors produced while loading the manifest, resolving inputs, hashing
/// files, or spawning the wrapped command.
#[derive(Debug, Error)]
pub enum Error {
    /// An I/O operation failed.
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),

    /// No manifest was found searching upward from the working directory.
    #[error("no .mmz/config.yaml found in `{start}` or any parent; create one with `mmz --init`")]
    NoManifest {
        /// Directory the upward search started from.
        start: PathBuf,
    },

    /// No rule matched the invoked command and `no_match` strictness is on.
    #[error(
        "no rule matches `{command}`; add a matching rule to .mmz/config.yaml or relax `strict`"
    )]
    NoMatch {
        /// The invoked command, joined for display.
        command: String,
    },

    /// A matched rule resolved to zero input files and `no_inputs` strictness
    /// is on.
    #[error("rule `{rule}` matched no input files; fix its scopes or relax `strict`")]
    NoInputs {
        /// Name of the rule that resolved to nothing.
        rule: String,
    },

    /// The manifest failed to parse.
    #[error("failed to parse manifest {path}: {source}")]
    ManifestParse {
        /// Path of the offending manifest.
        path: PathBuf,
        /// Underlying parser error.
        source: Box<serde_yaml_ng::Error>,
    },

    /// A command rule's `{scope}` fan macro names a scope the manifest does not
    /// define. A macro fans over files, so a probe cannot stand in for it.
    #[error("command `{command}` references unknown scope `{scope}`")]
    UnknownScope {
        /// Name of the command rule.
        command: String,
        /// The missing scope name.
        scope: String,
    },

    /// A command rule's `inputs` names something the manifest declares neither
    /// as a scope nor as a probe. One namespace, so one error.
    #[error(
        "command `{command}` references unknown input `{input}`; declare it under `scopes:` or `probes:`"
    )]
    UnknownInput {
        /// Name of the command rule.
        command: String,
        /// The unresolvable `inputs` entry.
        input: String,
    },

    /// A probe and a scope share a name, so an `inputs:` entry naming it would
    /// be ambiguous.
    #[error(
        "`{name}` is declared as both a scope and a probe; `inputs:` has one namespace, so a name must be one or the other"
    )]
    NameCollision {
        /// The name claimed twice.
        name: String,
    },

    /// A probe command exited non-zero, so its stdout is not a usable input.
    #[error(
        "probe `{name}` failed (exit {code}); mmz consumed no output and wrote no cache record\n  command: {run}\n  stderr: {stderr}"
    )]
    ProbeFailed {
        /// Name of the offending probe.
        name: String,
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
        "probe `{name}` could not be run; mmz consumed no output and wrote no cache record\n  command: {run}\n  {source}"
    )]
    ProbeSpawn {
        /// Name of the offending probe.
        name: String,
        /// The `run` line, as the manifest spells it.
        run: String,
        /// Underlying spawn error.
        source: std::io::Error,
    },

    /// A probe printed nothing and did not opt into that with `allow_empty`.
    #[error(
        "probe `{name}` produced no output; that is almost always a selector that matched nothing — set `allow_empty: true` on the probe if empty really is a valid input\n  command: {run}"
    )]
    ProbeEmpty {
        /// Name of the offending probe.
        name: String,
        /// The `run` line, as the manifest spells it.
        run: String,
    },

    /// A command rule has a blank `name`.
    #[error("command #{0} has an empty `name`; every command must declare a name")]
    EmptyCommandName(usize),

    /// Two command rules share the same name (the cache identity).
    #[error("duplicate command name: {0} (command names must be unique)")]
    DuplicateCommand(String),

    /// A command rule declares an output that is not a usable literal path.
    #[error("command `{command}` declares invalid output `{path}`: {reason}")]
    InvalidOutput {
        /// Name of the offending command.
        command: String,
        /// The offending output path, as written.
        path: String,
        /// Why it cannot be used as an output.
        reason: String,
    },

    /// A rule's command exited 0 without producing a declared output, so no
    /// record was written: a record here would claim an artifact that is not
    /// on disk, and every later invocation would skip on that claim.
    #[error(
        "`{rule}` succeeded without producing its declared output `{path}`; no cache record was written, so the rule stays stale"
    )]
    MissingOutput {
        /// Cache identity of the rule that ran.
        rule: String,
        /// The declared output that never appeared.
        path: String,
    },

    /// A command rule declares the same tag twice.
    #[error("command `{command}` declares tag `{tag}` twice")]
    DuplicateTag {
        /// Name of the offending command.
        command: String,
        /// The duplicated tag.
        tag: String,
    },

    /// `--is-fresh` (or another tag-filtered action) was given both a `--tag`
    /// filter and a specific command to target.
    #[error(
        "`--tag` cannot be combined with a command; a command already resolves to a single rule"
    )]
    TagWithCommand,

    /// A command rule's `name` carries a malformed `{scope}` fan macro.
    #[error("command `{name}` has a malformed `{{scope}}` macro: {reason}")]
    MacroSyntax {
        /// The offending rule name.
        name: String,
        /// What is wrong with the macro.
        reason: String,
    },

    /// Two rules resolve to the same cache identity, so which one owns the
    /// record — and its inputs — is ambiguous.
    #[error(
        "cache identity `{identity}` is claimed by multiple rules ({rules}); make their file sets or names disjoint"
    )]
    CollidingIdentity {
        /// The shared expanded identity.
        identity: String,
        /// The colliding rule names, for the operator to reconcile.
        rules: String,
    },

    /// A glob pattern was invalid.
    #[error("invalid pattern `{pattern}`: {source}")]
    Pattern {
        /// The offending pattern.
        pattern: String,
        /// Underlying glob error.
        source: globset::Error,
    },

    /// `MMZ_NOW` is set to something that is not a Unix epoch in seconds.
    ///
    /// Refused rather than ignored: falling back to the system clock would hide
    /// the misconfiguration and quietly restore the non-determinism the pin
    /// exists to remove.
    #[error(
        "`MMZ_NOW` is set to `{value}`, which is not a Unix epoch in seconds; set it to a whole number of seconds (e.g. `date +%s`) or unset it to use the system clock"
    )]
    InvalidNow {
        /// The offending value, as the environment spells it.
        value: String,
    },

    /// A cache record could not be serialized.
    #[error("failed to serialize cache record: {0}")]
    Serialize(Box<serde_yaml_ng::Error>),

    /// The wrapped command could not be spawned.
    #[error("failed to run `{program}`: {source}")]
    Spawn {
        /// The program mmz tried to execute.
        program: String,
        /// Underlying spawn error.
        source: std::io::Error,
    },

    /// mmz was invoked with no command to run.
    #[error("no command given")]
    EmptyCommand,

    /// `mmz --init` found a manifest already in place.
    #[error("{path} already exists; remove it first or edit it directly")]
    ManifestExists {
        /// Path of the existing manifest.
        path: PathBuf,
    },

    /// An invariant that should hold by construction did not.
    #[error("internal error: {0}")]
    Internal(String),

    /// A path named by `imports:` does not exist.
    #[error("import in {importer} names `{path}`, which does not exist")]
    ImportMissing {
        /// The file whose `imports:` list named the missing path.
        importer: PathBuf,
        /// The resolved path (directory or file) that was not found.
        path: PathBuf,
    },

    /// A path named by `imports:` exists but could not be read.
    ///
    /// Named so this cannot be confused with [`Error::Io`]'s pathless message —
    /// the root manifest keeps using that generic variant, since a missing or
    /// unreadable root manifest is not a new failure mode this feature adds.
    #[error("failed to read import {path}: {source}")]
    ImportNotReadable {
        /// The unreadable path.
        path: PathBuf,
        /// Underlying I/O error.
        source: std::io::Error,
    },

    /// An `imports:` chain leads back to a file already being loaded.
    #[error("import cycle: {chain}")]
    ImportCycle {
        /// The chain of files, root first, ending back at the repeated path.
        chain: String,
    },

    /// Two different files each declare a scope with the same name.
    #[error("scope `{name}` is declared in both {first} and {second}")]
    DuplicateScope {
        /// The name declared twice.
        name: String,
        /// The file that declared it first.
        first: PathBuf,
        /// The file that declared it again.
        second: PathBuf,
    },

    /// Two different files each declare a probe with the same name.
    #[error("probe `{name}` is declared in both {first} and {second}")]
    DuplicateProbe {
        /// The name declared twice.
        name: String,
        /// The file that declared it first.
        first: PathBuf,
        /// The file that declared it again.
        second: PathBuf,
    },

    /// Two different files each declare a command rule with the same `name`.
    ///
    /// [`Error::DuplicateCommand`] keeps its single-file message unchanged, so
    /// a manifest with no imports sees no change; this is the cross-file
    /// spelling, naming both files.
    #[error("command `{name}` is declared in both {first} and {second}")]
    DuplicateCommandAcrossFiles {
        /// The command name declared twice.
        name: String,
        /// The file that declared it first.
        first: PathBuf,
        /// The file that declared it again.
        second: PathBuf,
    },

    /// An imported file (not the root manifest) sets a key that only the root
    /// manifest may set (`cache_dir`, `gitignore`, `strict`, `on_hit`).
    #[error("`{key}` is set in {path}, but may only be set in the root manifest")]
    FragmentPolicyKey {
        /// The offending top-level key.
        key: String,
        /// The fragment that set it.
        path: PathBuf,
    },
}

/// Convenience alias for fallible operations in this crate.
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::Error;

    #[test]
    fn messages_are_actionable() {
        let scope = Error::UnknownScope {
            command: "cargo test".to_owned(),
            scope: "rust".to_owned(),
        };
        assert!(scope.to_string().contains("unknown scope `rust`"));
        let dup = Error::DuplicateCommand("sh".to_owned());
        assert!(dup.to_string().contains("duplicate command name"));
        let blank = Error::EmptyCommandName(2);
        assert!(blank.to_string().contains("empty `name`"));

        let no_match = Error::NoMatch {
            command: "cargo build".to_owned(),
        };
        assert!(
            no_match
                .to_string()
                .contains("no rule matches `cargo build`")
        );
        let no_inputs = Error::NoInputs {
            rule: "cargo test".to_owned(),
        };
        assert!(no_inputs.to_string().contains("matched no input files"));
        let no_manifest = Error::NoManifest {
            start: std::path::PathBuf::from("/tmp/x"),
        };
        assert!(
            no_manifest
                .to_string()
                .contains("no .mmz/config.yaml found")
        );

        let bad_output = Error::InvalidOutput {
            command: "just cover".to_owned(),
            path: "target/*.info".to_owned(),
            reason: "outputs are literal paths, not patterns".to_owned(),
        };
        assert!(
            bad_output
                .to_string()
                .contains("declares invalid output `target/*.info`")
        );
        let missing = Error::MissingOutput {
            rule: "just cover".to_owned(),
            path: "target/coverage/lcov.info".to_owned(),
        };
        let text = missing.to_string();
        assert!(
            text.contains("target/coverage/lcov.info"),
            "the missing artifact is named: {text}"
        );
        assert!(
            text.contains("no cache record was written"),
            "and the consequence is spelled out: {text}"
        );
    }

    #[test]
    fn probe_messages_name_the_probe_and_the_consequence() {
        let failed = Error::ProbeFailed {
            name: "fmt-recipe".to_owned(),
            run: "just --dump | jq .recipes".to_owned(),
            code: 5,
            stderr: "jq: error: no such key".to_owned(),
        };
        let text = failed.to_string();
        assert!(
            text.contains("probe `fmt-recipe`"),
            "names the probe: {text}"
        );
        assert!(text.contains("exit 5"), "names the exit code: {text}");
        assert!(text.contains("jq: error"), "carries stderr: {text}");
        assert!(
            text.contains("wrote no cache record"),
            "a failed probe never reaches the hasher, and says so: {text}"
        );

        let spawn = Error::ProbeSpawn {
            name: "toolchain".to_owned(),
            run: "rustc -vV".to_owned(),
            source: std::io::Error::other("no such file"),
        };
        let text = spawn.to_string();
        assert!(
            text.contains("probe `toolchain`"),
            "names the probe: {text}"
        );
        assert!(
            text.contains("wrote no cache record"),
            "an unspawnable probe is the same hard stop: {text}"
        );

        let empty = Error::ProbeEmpty {
            name: "selector".to_owned(),
            run: "jq -c .missing".to_owned(),
        };
        let text = empty.to_string();
        assert!(text.contains("probe `selector`"), "names the probe: {text}");
        assert!(
            text.contains("allow_empty"),
            "points at the opt-in rather than leaving it a dead end: {text}"
        );
    }

    #[test]
    fn input_namespace_messages_are_actionable() {
        let unknown = Error::UnknownInput {
            command: "cargo test".to_owned(),
            input: "ghost".to_owned(),
        };
        let text = unknown.to_string();
        assert!(
            text.contains("unknown input `ghost`"),
            "names the entry: {text}"
        );
        assert!(
            text.contains("`scopes:` or `probes:`"),
            "names both places it could be declared: {text}"
        );

        let clash = Error::NameCollision {
            name: "rust".to_owned(),
        };
        assert!(
            clash.to_string().contains("one namespace"),
            "the collision explains why one name cannot be both"
        );
    }
}
