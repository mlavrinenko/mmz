//! The mmz manifest (`.mmz/config.yaml`): named input scopes and the command
//! rules that reference them.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Deserializer};

use crate::error::{Error, Result};
use crate::resolve::GlobGroup;

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

/// Trims each declared tag and drops the ones left blank, so a stray
/// whitespace-only entry can never silently fail to match `--tag`. Case is
/// left untouched — tags compare exactly.
fn normalize_tags<'de, D>(deserializer: D) -> std::result::Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = Vec::<String>::deserialize(deserializer)?;
    Ok(raw
        .into_iter()
        .map(|tag| tag.trim().to_owned())
        .filter(|tag| !tag.is_empty())
        .collect())
}

/// A named input scope: the patterns it contributes, plus an optional
/// per-scope override of the manifest's `gitignore` filter.
///
/// The array form is the common spelling and inherits the manifest-level
/// setting:
///
/// ```yaml
/// scopes:
///   src: ["src/**"]
/// ```
///
/// The object form names the patterns under `globs` and may pin `gitignore` for
/// this scope alone. That is the escape hatch for a scope naming build
/// artifacts: they live in git-ignored paths by definition, so under the
/// default filter the scope resolves empty and every rule referencing it is
/// fresh forever.
///
/// ```yaml
/// scopes:
///   lcov:
///     gitignore: false
///     globs: ["target/coverage/lcov.info"]
/// ```
#[derive(Debug, Clone, Deserialize)]
#[serde(try_from = "ScopeSpelling")]
pub struct Scope {
    /// Glob patterns and literal paths this scope contributes.
    pub globs: Vec<String>,
    /// Per-scope override of [`Manifest::gitignore`]; `None` inherits it.
    pub gitignore: Option<bool>,
}

impl Scope {
    /// Whether this scope's globs skip git-ignored paths: its own `gitignore`
    /// when set, else the manifest-level `inherited` value.
    #[must_use]
    pub fn honours_gitignore(&self, inherited: bool) -> bool {
        self.gitignore.unwrap_or(inherited)
    }
}

/// The two manifest spellings of a scope value, normalized into [`Scope`].
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum ScopeSpelling {
    /// The array form — patterns only, inheriting the manifest's `gitignore`.
    Patterns(Vec<String>),
    /// The object form — `globs` plus an optional per-scope `gitignore`.
    Object(ScopeObject),
}

/// The object spelling's fields. `globs` is optional here only so that omitting
/// it is reported against the object rather than as a failure to match either
/// spelling; [`Scope`]'s conversion rejects both a missing and an empty list.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ScopeObject {
    #[serde(default)]
    globs: Option<Vec<String>>,
    #[serde(default)]
    gitignore: Option<bool>,
}

impl TryFrom<ScopeSpelling> for Scope {
    type Error = String;

    fn try_from(spelling: ScopeSpelling) -> std::result::Result<Self, Self::Error> {
        let object = match spelling {
            ScopeSpelling::Patterns(globs) => {
                return Ok(Self {
                    globs,
                    gitignore: None,
                });
            }
            ScopeSpelling::Object(object) => object,
        };
        let globs = object
            .globs
            .ok_or_else(|| "a scope object must list its patterns under `globs`".to_owned())?;
        if globs.is_empty() {
            return Err("a scope object's `globs` must list at least one pattern".to_owned());
        }
        Ok(Self {
            globs,
            gitignore: object.gitignore,
        })
    }
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
    /// Each value is either an array of patterns or an object carrying a
    /// per-scope `gitignore` override (see [`Scope`]).
    #[serde(default)]
    pub scopes: BTreeMap<String, Scope>,

    /// Ordered command rules. The first rule whose `name` is a token-prefix of
    /// the invoked command wins; its inputs determine the cache.
    #[serde(default)]
    pub commands: Vec<Command>,

    /// When true (the default), glob expansion skips paths ignored by git, so
    /// build artifacts never enter an input set. Set false to match every file
    /// on disk. A scope may override this for itself (see [`Scope`]).
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

    /// Free-form labels filtered by `mmz --is-fresh --tag <tag>` (and
    /// `--status --tag <tag>`); a rule with no tags never matches a `--tag`
    /// filter. Case-faithful; each entry is trimmed and blanks are dropped
    /// (see [`normalize_tags`]). Duplicates within one rule are rejected by
    /// [`Manifest::validate`].
    #[serde(default, deserialize_with = "normalize_tags")]
    pub tags: Vec<String>,

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
    /// unique, and reference only defined scopes; tags are unique per command.
    ///
    /// # Errors
    ///
    /// Returns [`Error::EmptyCommandName`], [`Error::DuplicateCommand`],
    /// [`Error::UnknownScope`], or [`Error::DuplicateTag`].
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
            if let Some(mac) = crate::parametric::parse(&command.name)? {
                if !self.scopes.contains_key(&mac.scope) {
                    return Err(Error::UnknownScope {
                        command: command.name.clone(),
                        scope: mac.scope,
                    });
                }
            }
            for scope in &command.inputs {
                if !self.scopes.contains_key(scope) {
                    return Err(Error::UnknownScope {
                        command: command.name.clone(),
                        scope: scope.clone(),
                    });
                }
            }
            let mut seen_tags: Vec<&str> = Vec::new();
            for tag in &command.tags {
                if seen_tags.contains(&tag.as_str()) {
                    return Err(Error::DuplicateTag {
                        command: command.name.clone(),
                        tag: tag.clone(),
                    });
                }
                seen_tags.push(tag.as_str());
            }
        }
        Ok(())
    }

    /// Returns the glob patterns a command draws from, bucketed by the
    /// gitignore setting they expand under: one group for the scopes honouring
    /// the filter and one for the scopes that opted out, each deduplicated and
    /// omitted when no scope feeds it.
    ///
    /// Bucketing rather than one group per scope keeps the filesystem walk
    /// count at two in the worst case, however many scopes a rule references.
    ///
    /// # Errors
    ///
    /// Returns [`Error::UnknownScope`] if a referenced scope is undefined.
    pub fn glob_groups(&self, command: &Command) -> Result<Vec<GlobGroup>> {
        let mut honoured: Vec<String> = Vec::new();
        let mut opted_out: Vec<String> = Vec::new();
        for name in &command.inputs {
            let scope = self.scopes.get(name).ok_or_else(|| Error::UnknownScope {
                command: command.name.clone(),
                scope: name.clone(),
            })?;
            let bucket = if scope.honours_gitignore(self.gitignore) {
                &mut honoured
            } else {
                &mut opted_out
            };
            for glob in &scope.globs {
                if !bucket.contains(glob) {
                    bucket.push(glob.clone());
                }
            }
        }
        Ok([(true, honoured), (false, opted_out)]
            .into_iter()
            .filter(|(_, globs)| !globs.is_empty())
            .map(|(gitignore, globs)| GlobGroup { globs, gitignore })
            .collect())
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
#[path = "manifest_tests.rs"]
mod tests;
