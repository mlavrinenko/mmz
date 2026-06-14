//! The mmz manifest (`mmz.yaml`): named input scopes and the command rules
//! that reference them.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::{Error, Result};

/// Default manifest file names, tried in order during discovery.
const DEFAULT_NAMES: [&str; 2] = ["mmz.yaml", "mmz.yml"];

/// Default for [`Manifest::gitignore`].
const fn default_gitignore() -> bool {
    true
}

/// Default for [`Manifest::cache_dir`] — the gitignored state directory.
fn default_cache_dir() -> String {
    ".mmz".to_owned()
}

/// How a rule's `name` is matched against an invoked command.
///
/// Both modes split `name` on whitespace into tokens. The difference is whether
/// trailing argv beyond those tokens is allowed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchMode {
    /// The tokens are a leading slice of argv, so `cargo test` matches
    /// `cargo test` and `cargo test --workspace`. The default.
    #[default]
    Prefix,
    /// The tokens equal argv exactly, so `cargo test` matches only the bare
    /// `cargo test`. Narrowing only — never causes a wrongful skip.
    Exact,
}

/// A runtime situation mmz can either error on (strict) or fall back from.
///
/// Only the cases reachable once a manifest has loaded are configurable; a
/// missing or unparseable manifest always errors, since there is no `strict`
/// list to consult.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StrictCase {
    /// No rule's name is a token-prefix of the invoked command.
    NoMatch,
    /// A matched rule's scopes resolve to zero files on disk.
    NoInputs,
}

/// The set of [`StrictCase`]s mmz errors on rather than passing through.
///
/// Absent from the manifest means every case (the safe default); an empty list
/// means none (full passthrough); a subset relaxes the rest.
#[derive(Debug, Clone, Deserialize)]
#[serde(transparent)]
pub struct StrictPolicy {
    cases: BTreeSet<StrictCase>,
}

impl StrictPolicy {
    /// Every case enforced — the default when `strict` is omitted.
    #[must_use]
    pub fn all() -> Self {
        Self {
            cases: [StrictCase::NoMatch, StrictCase::NoInputs]
                .into_iter()
                .collect(),
        }
    }

    /// True when `case` should error instead of falling back.
    #[must_use]
    pub fn enforces(&self, case: StrictCase) -> bool {
        self.cases.contains(&case)
    }
}

/// A declared set of command rules and the input scopes they draw on.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    /// Reusable named glob sets. A scope is defined once and referenced by any
    /// number of commands, so a shared input path is declared in one place.
    #[serde(default)]
    pub scopes: BTreeMap<String, Vec<String>>,

    /// Ordered command rules. The first rule whose `name` is a token-prefix of
    /// the invoked command wins; its inputs determine the cache.
    #[serde(default)]
    pub commands: Vec<Command>,

    /// When true (the default), glob expansion skips paths ignored by git, so
    /// build artifacts never enter an input set. Set false to match every file
    /// on disk.
    #[serde(default = "default_gitignore")]
    pub gitignore: bool,

    /// Directory for throwaway cache records, relative to the manifest root
    /// (an absolute path is used as-is). Must be git-ignored. Defaults to
    /// `.mmz`.
    #[serde(default = "default_cache_dir")]
    pub cache_dir: String,

    /// Runtime cases mmz errors on instead of falling back to passthrough.
    /// Omitted means all cases (the default); see [`StrictPolicy`].
    #[serde(default = "StrictPolicy::all")]
    pub strict: StrictPolicy,
}

/// A single command rule: a matcher plus the scopes that feed its cache.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Command {
    /// Matcher and the cache identity. `cargo test` matches any invocation
    /// beginning with the tokens `cargo test` (see [`Command::match_mode`]).
    pub name: String,

    /// Scope names whose globs, unioned, are this command's inputs.
    #[serde(default)]
    pub inputs: Vec<String>,

    /// How `name` matches argv: token-prefix (default) or exact. Spelled
    /// `match` in the manifest.
    #[serde(rename = "match", default)]
    pub match_mode: MatchMode,
}

impl Manifest {
    /// Loads and validates a manifest from `path`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ManifestParse`] when the file cannot be parsed, or a
    /// validation error ([`Error::EmptyCommandName`], [`Error::DuplicateCommand`],
    /// [`Error::UnknownScope`]) when its contents are inconsistent.
    pub fn load(path: &Path) -> Result<Self> {
        let text = fs::read_to_string(path)?;
        let manifest: Self =
            serde_yaml_ng::from_str(&text).map_err(|source| Error::ManifestParse {
                path: path.to_path_buf(),
                source: Box::new(source),
            })?;
        manifest.validate()?;
        Ok(manifest)
    }

    /// Checks invariants the schema cannot express: command names are present,
    /// unique, and reference only defined scopes.
    ///
    /// # Errors
    ///
    /// Returns [`Error::EmptyCommandName`], [`Error::DuplicateCommand`], or
    /// [`Error::UnknownScope`].
    pub fn validate(&self) -> Result<()> {
        let mut seen: Vec<&str> = Vec::new();
        for (index, command) in self.commands.iter().enumerate() {
            if command.name.trim().is_empty() {
                return Err(Error::EmptyCommandName(index + 1));
            }
            if seen.contains(&command.name.as_str()) {
                return Err(Error::DuplicateCommand(command.name.clone()));
            }
            seen.push(command.name.as_str());
            for scope in &command.inputs {
                if !self.scopes.contains_key(scope) {
                    return Err(Error::UnknownScope {
                        command: command.name.clone(),
                        scope: scope.clone(),
                    });
                }
            }
        }
        Ok(())
    }

    /// Returns the deduplicated union of glob patterns a command draws from.
    ///
    /// # Errors
    ///
    /// Returns [`Error::UnknownScope`] if a referenced scope is undefined.
    pub fn globs_for(&self, command: &Command) -> Result<Vec<String>> {
        let mut out: Vec<String> = Vec::new();
        for scope in &command.inputs {
            let globs = self.scopes.get(scope).ok_or_else(|| Error::UnknownScope {
                command: command.name.clone(),
                scope: scope.clone(),
            })?;
            for glob in globs {
                if !out.contains(glob) {
                    out.push(glob.clone());
                }
            }
        }
        Ok(out)
    }

    /// Walks up from `start` to the filesystem root, returning the first
    /// manifest found.
    #[must_use]
    pub fn discover(start: &Path) -> Option<PathBuf> {
        let mut dir = Some(start);
        while let Some(current) = dir {
            for name in DEFAULT_NAMES {
                let candidate = current.join(name);
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
            dir = current.parent();
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::{Manifest, MatchMode, StrictCase};

    fn parse(text: &str) -> Manifest {
        serde_yaml_ng::from_str(text).expect("parse")
    }

    #[test]
    fn cache_dir_defaults_and_overrides() {
        assert_eq!(parse("commands: []\n").cache_dir, ".mmz");
        assert_eq!(
            parse("commands: []\ncache_dir: .cache/mmz\n").cache_dir,
            ".cache/mmz"
        );
    }

    #[test]
    fn match_mode_defaults_to_prefix_and_parses_exact() {
        let manifest =
            parse("commands:\n  - name: cargo test\n  - name: cargo build\n    match: exact\n");
        let prefix = manifest.commands.first().expect("first rule");
        let exact = manifest.commands.get(1).expect("second rule");
        assert_eq!(prefix.match_mode, MatchMode::Prefix, "default");
        assert_eq!(exact.match_mode, MatchMode::Exact, "explicit");
    }

    #[test]
    fn strict_defaults_to_all_cases() {
        let manifest = parse("commands: []\n");
        assert!(manifest.strict.enforces(StrictCase::NoMatch));
        assert!(manifest.strict.enforces(StrictCase::NoInputs));
    }

    #[test]
    fn strict_list_selects_a_subset() {
        let manifest = parse("commands: []\nstrict: [no_match]\n");
        assert!(manifest.strict.enforces(StrictCase::NoMatch));
        assert!(
            !manifest.strict.enforces(StrictCase::NoInputs),
            "unlisted case relaxed"
        );

        let none = parse("commands: []\nstrict: []\n");
        assert!(!none.strict.enforces(StrictCase::NoMatch));
        assert!(!none.strict.enforces(StrictCase::NoInputs));
    }

    #[test]
    fn strict_rejects_unknown_case() {
        let parsed: Result<Manifest, _> = serde_yaml_ng::from_str("strict: [bogus]\n");
        assert!(parsed.is_err(), "unknown strict case is rejected");
    }

    #[test]
    fn parses_scopes_and_commands() {
        let manifest = parse(
            "scopes:\n  rust: [\"**/*.rs\"]\ncommands:\n  - name: cargo test\n    inputs: [rust]\n",
        );
        assert_eq!(manifest.commands.len(), 1);
        let command = manifest.commands.first().expect("command");
        assert_eq!(command.name, "cargo test");
        assert_eq!(
            manifest.globs_for(command).expect("globs"),
            vec!["**/*.rs".to_owned()]
        );
        assert!(manifest.gitignore, "gitignore defaults on");
    }

    #[test]
    fn rejects_unknown_fields() {
        let parsed: Result<Manifest, _> =
            serde_yaml_ng::from_str("scopes: {}\ncommands: []\nbogus: 1\n");
        assert!(parsed.is_err(), "unknown top-level fields are rejected");
    }

    #[test]
    fn validate_rejects_blank_and_duplicate_names() {
        let blank = parse("commands:\n  - name: \"  \"\n");
        assert!(blank.validate().is_err(), "blank name rejected");

        let dup = parse("commands:\n  - name: sh\n  - name: sh\n");
        assert!(dup.validate().is_err(), "duplicate name rejected");
    }

    #[test]
    fn validate_rejects_unknown_scope() {
        let manifest = parse("commands:\n  - name: sh\n    inputs: [ghost]\n");
        assert!(manifest.validate().is_err(), "missing scope rejected");
    }

    #[test]
    fn load_validates_from_disk() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("mmz.yaml");
        std::fs::write(&path, "commands:\n  - name: sh\n    inputs: [ghost]\n").expect("write");
        assert!(Manifest::load(&path).is_err(), "load runs validation");
    }

    #[test]
    fn discovers_walking_upwards() {
        let dir = tempfile::tempdir().expect("tempdir");
        let nested = dir.path().join("a/b");
        std::fs::create_dir_all(&nested).expect("mkdir");
        let path = dir.path().join("mmz.yaml");
        std::fs::write(&path, "commands: []\n").expect("write");
        assert_eq!(Manifest::discover(&nested), Some(path));
    }
}
