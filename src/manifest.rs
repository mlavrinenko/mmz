//! The mmz manifest (`.mmz/config.yaml`): named input scopes and the command
//! rules that reference them.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::{Error, Result};

/// Directory holding mmz's per-project state, found by walking upward. The
/// config lives inside it so a project gains one entry, not two.
pub const CONFIG_DIR: &str = ".mmz";

/// Config file names within [`CONFIG_DIR`], tried in order during discovery.
const CONFIG_NAMES: [&str; 2] = ["config.yaml", "config.yml"];

/// Default for [`Manifest::gitignore`].
const fn default_gitignore() -> bool {
    true
}

/// Default for [`Manifest::cache_dir`] — the gitignored state directory, nested
/// under [`CONFIG_DIR`] so a single `.mmz/.gitignore` can cover it.
fn default_cache_dir() -> String {
    ".mmz/cache".to_owned()
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

    /// Directory for throwaway cache records, relative to the project root —
    /// the directory holding `.mmz` (an absolute path is used as-is). Must be
    /// git-ignored. Defaults to `.mmz/cache`.
    #[serde(default = "default_cache_dir")]
    pub cache_dir: String,

    /// Runtime cases mmz errors on instead of falling back to passthrough.
    /// Omitted means all cases (the default); see [`StrictPolicy`].
    #[serde(default = "StrictPolicy::all")]
    pub strict: StrictPolicy,

    /// Message printed to stderr when a command is skipped (a cache hit).
    /// `{cache:<field>}` macros substitute a field from the matched rule's
    /// cache record (e.g. `command`, `ran_at`, `input_digest`). An empty string
    /// prints nothing. A command's own `on_hit` overrides this. Default: none.
    #[serde(default)]
    pub on_hit: Option<String>,
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

    /// Overrides the manifest-level `on_hit` for this rule; an empty string
    /// suppresses the notice. Default: inherit the manifest's `on_hit`.
    #[serde(default)]
    pub on_hit: Option<String>,
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
    /// `.mmz/config.yaml` (or `.yml`) found.
    #[must_use]
    pub fn discover(start: &Path) -> Option<PathBuf> {
        let mut dir = Some(start);
        while let Some(current) = dir {
            for name in CONFIG_NAMES {
                let candidate = current.join(CONFIG_DIR).join(name);
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
            dir = current.parent();
        }
        None
    }

    /// Discovers, loads, and validates the nearest manifest above `cwd`, pairing
    /// it with the project root its relative paths resolve against.
    ///
    /// # Errors
    ///
    /// Returns [`Error::NoManifest`] when none is found, a load/validation error
    /// from [`Manifest::load`], or [`Error::Internal`] if the config path has no
    /// project root (it always does in practice — `<root>/.mmz/config.yaml`).
    pub fn locate(cwd: &Path) -> Result<Located> {
        let path = Self::discover(cwd).ok_or_else(|| Error::NoManifest {
            start: cwd.to_path_buf(),
        })?;
        let root = path
            .parent()
            .and_then(Path::parent)
            .ok_or_else(|| Error::Internal("config path has no project root".to_owned()))?
            .to_path_buf();
        let manifest = Self::load(&path)?;
        Ok(Located {
            path,
            root,
            manifest,
        })
    }
}

/// A discovered manifest: the config file, the project root its relative paths
/// resolve against, and the parsed, validated model.
///
/// Config lives at `<root>/.mmz/config.yaml`, so the project root is the parent
/// of `.mmz`. Input globs and `cache_dir` resolve against `root`, never `.mmz`.
pub struct Located {
    /// The config file, for display and error messages.
    pub path: PathBuf,
    /// Project root: the directory that holds `.mmz`.
    pub root: PathBuf,
    /// The parsed, validated manifest.
    pub manifest: Manifest,
}

#[cfg(test)]
mod tests {
    use super::{Manifest, MatchMode, StrictCase};

    fn parse(text: &str) -> Manifest {
        serde_yaml_ng::from_str(text).expect("parse")
    }

    #[test]
    fn cache_dir_defaults_and_overrides() {
        assert_eq!(parse("commands: []\n").cache_dir, ".mmz/cache");
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
    fn on_hit_parses_global_and_per_command_and_defaults_none() {
        let manifest = parse(
            "on_hit: \"global note\"\ncommands:\n  - name: cargo test\n    on_hit: \"rule note\"\n  - name: cargo build\n",
        );
        assert_eq!(manifest.on_hit.as_deref(), Some("global note"));
        let overridden = manifest.commands.first().expect("first rule");
        let inherits = manifest.commands.get(1).expect("second rule");
        assert_eq!(
            overridden.on_hit.as_deref(),
            Some("rule note"),
            "per-command override"
        );
        assert_eq!(inherits.on_hit, None, "absent per-command on_hit is None");
        assert_eq!(
            parse("commands: []\n").on_hit,
            None,
            "absent global on_hit is None"
        );
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
        let path = dir.path().join("config.yaml");
        std::fs::write(&path, "commands:\n  - name: sh\n    inputs: [ghost]\n").expect("write");
        assert!(Manifest::load(&path).is_err(), "load runs validation");
    }

    fn write_config(root: &std::path::Path, body: &str) -> std::path::PathBuf {
        let dir = root.join(".mmz");
        std::fs::create_dir_all(&dir).expect("mkdir .mmz");
        let path = dir.join("config.yaml");
        std::fs::write(&path, body).expect("write config");
        path
    }

    #[test]
    fn discovers_walking_upwards() {
        let dir = tempfile::tempdir().expect("tempdir");
        let nested = dir.path().join("a/b");
        std::fs::create_dir_all(&nested).expect("mkdir");
        let path = write_config(dir.path(), "commands: []\n");
        assert_eq!(Manifest::discover(&nested), Some(path));
    }

    #[test]
    fn locate_roots_at_the_parent_of_dot_mmz() {
        let dir = tempfile::tempdir().expect("tempdir");
        let nested = dir.path().join("a/b");
        std::fs::create_dir_all(&nested).expect("mkdir");
        write_config(dir.path(), "commands: []\n");
        let located = Manifest::locate(&nested).expect("locate");
        assert_eq!(located.root, dir.path(), "root is the parent of .mmz");
        assert_eq!(
            located.root.join(&located.manifest.cache_dir),
            dir.path().join(".mmz/cache"),
            "cache_dir resolves under the project root, not inside .mmz",
        );
    }
}
