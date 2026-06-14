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
    #[error("no mmz.yaml found in `{start}` or any parent; create one with `mmz --init`")]
    NoManifest {
        /// Directory the upward search started from.
        start: PathBuf,
    },

    /// No rule matched the invoked command and `no_match` strictness is on.
    #[error("no rule matches `{command}`; add a matching rule to mmz.yaml or relax `strict`")]
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

    /// A command rule references a scope that the manifest does not define.
    #[error("command `{command}` references unknown scope `{scope}`")]
    UnknownScope {
        /// Name of the command rule.
        command: String,
        /// The missing scope name.
        scope: String,
    },

    /// A command rule has a blank `name`.
    #[error("command #{0} has an empty `name`; every command must declare a name")]
    EmptyCommandName(usize),

    /// Two command rules share the same name (the cache identity).
    #[error("duplicate command name: {0} (command names must be unique)")]
    DuplicateCommand(String),

    /// A glob pattern was invalid.
    #[error("invalid pattern `{pattern}`: {source}")]
    Pattern {
        /// The offending pattern.
        pattern: String,
        /// Underlying glob error.
        source: globset::Error,
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
        assert!(no_manifest.to_string().contains("no mmz.yaml found"));
    }
}
