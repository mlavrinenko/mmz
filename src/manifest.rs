//! The mmz manifest (`.mmz/config.yaml`): named input scopes and the command
//! rules that reference them.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Deserializer};

use crate::error::{Error, Result};
use crate::probe::Probe;
use crate::resolve::GlobGroup;
use crate::ttl::Ttl;

/// Directory holding mmz's per-project state, found by walking upward. The
/// config lives inside it so a project gains one entry, not two.
pub const CONFIG_DIR: &str = ".mmz";

/// Config file names within [`CONFIG_DIR`], tried in order during discovery.
const CONFIG_NAMES: [&str; 2] = ["config.yaml", "config.yml"];

/// Default for [`Manifest::gitignore`]. `pub(crate)` so [`crate::compose`] can
/// apply the same default when a root manifest omits the key.
pub(crate) const fn default_gitignore() -> bool {
    true
}

/// Default for [`Manifest::cache_dir`] — the gitignored state directory, nested
/// under [`CONFIG_DIR`] so a single `.mmz/.gitignore` can cover it.
/// `pub(crate)` so [`crate::compose`] can apply the same default when a root
/// manifest omits the key.
pub(crate) fn default_cache_dir() -> String {
    ".mmz/cache".to_owned()
}

/// Default for [`Manifest::probe_shell`] — `sh -c`, the shell every probe ran
/// under before the key existed. `pub(crate)` so [`crate::compose`] can apply
/// the same default when a root manifest omits the key.
pub(crate) fn default_probe_shell() -> Vec<String> {
    crate::probe::DEFAULT_SHELL
        .iter()
        .map(|part| (*part).to_owned())
        .collect()
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

/// A named input scope: its patterns, the patterns it leaves out, and an
/// optional override of the manifest's `gitignore` filter.
///
/// The array form inherits the manifest-level setting. The object form names
/// the patterns under `globs`, may pin `gitignore` for a scope naming build
/// artifacts (which resolve empty under the filter, so the rule would read fresh
/// forever), and may list `exclude`:
///
/// ```yaml
/// scopes:
///   src: ["src/**"]
///   lcov:
///     gitignore: false
///     globs: ["target/coverage/lcov.info"]
///   examples:
///     globs: ["examples/**"]
///     exclude: ["**/.cache"]
/// ```
#[derive(Debug, Clone, Deserialize)]
#[serde(try_from = "ScopeSpelling")]
pub struct Scope {
    /// Glob patterns and literal paths this scope contributes.
    pub globs: Vec<String>,
    /// Patterns this scope leaves out of its own `globs`; empty in the array
    /// form. See [`GlobGroup::exclude`] for how a path is matched.
    pub exclude: Vec<String>,
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
    /// The object form — `globs` plus an optional `exclude` and per-scope
    /// `gitignore`.
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
    exclude: Vec<String>,
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
                    exclude: Vec::new(),
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
            exclude: object.exclude,
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

/// The `strict` policy's two types, split out once this file reached its line
/// cap. Re-exported so every `manifest::StrictCase` / `manifest::StrictPolicy`
/// path outside this module keeps working without knowing the split happened.
#[path = "manifest_strict.rs"]
mod strict;
pub use strict::{StrictCase, StrictPolicy};

/// A declared set of command rules and the input scopes they draw on.
///
/// # Adding a key here is not a one-file change
///
/// `deny_unknown_fields` makes this struct the manifest's whole surface, and
/// several sites are derived from it, so a field added here alone fails a test
/// or a gate rather than shipping half-wired:
///
/// 1. The field, with `#[serde(default …)]` and a doc comment. A non-literal
///    default needs a `pub(crate)` `default_*` helper above, for
///    [`crate::compose`] to apply too.
/// 2. `schema/mmz.schema.json`, with a `description`, or `src/schema.rs`'s
///    `schema_documents_every_manifest_field` fails. The fragment schema is the
///    config schema minus the policy keys.
/// 3. `www/utils/config-notes.typ`, keyed as the reference renders it (bare at
///    top level, `commands[].`/`probes[].`/`scopes[].` below);
///    `just check-doc-facts` matches the two key sets both ways.
/// 4. If it can be validated, [`Manifest::validate`], an [`crate::error::Error`]
///    variant and its arm in `main.rs`'s exhaustive `exit_for`. Prefer an
///    existing exit code; a new one is documented CLI surface
///    (`www/utils/exit-code-notes.typ`).
///
/// A key that governs the whole run is also a *policy* key: add it to
/// [`crate::compose::POLICY_KEYS`] (fixed-size, so `compose_policy.rs` stops
/// compiling until updated), resolve it in `compose_policy::resolve`, add it to
/// [`crate::compose::Document`] as `Option<Option<T>>` so an explicit `null` is
/// caught, and render it in `dump.rs`'s `Policy` and `render_policy`.
///
/// Editing this file, `src/probe.rs` or `src/main.rs` puts an `outdatty` group
/// out of date: review its dependents, then `outdatty update --group <name>`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    /// Reusable named glob sets, each an array of patterns or an object with a
    /// per-scope `exclude` and `gitignore` (see [`Scope`]).
    #[serde(default)]
    pub scopes: BTreeMap<String, Scope>,

    /// Named sources of input bytes. A rule's `inputs:` names a probe as it
    /// names a scope, and the probe's hash joins its digest, so a rule can
    /// depend on part of a file or on something that is not a file. A probe
    /// sharing a scope's name is a load error; see [`crate::probe`].
    #[serde(default)]
    pub probes: BTreeMap<String, Probe>,

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

    /// The argv a probe's `run` line is executed by, with the line appended as
    /// one final argument. Defaults to `["sh", "-c"]`. A probe measures
    /// whatever the ambient `PATH` resolves, so pinning this, as
    /// `["direnv", "exec", ".", "sh", "-c"]` does, stops a probe's answer
    /// depending on the caller's shell. An empty list is a load error, since
    /// there would be nothing to spawn. See [`crate::probe`].
    #[serde(default = "default_probe_shell")]
    pub probe_shell: Vec<String>,

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

    /// Input names, each either a scope (whose globs union into this command's
    /// file set) or a probe (whose stdout hash joins its digest). One
    /// namespace, so the two kinds are written identically here.
    #[serde(default)]
    pub inputs: Vec<String>,

    /// Patterns left out of every file this rule's scopes resolve to, its
    /// `{file:scope}` domain included, on top of each scope's own `exclude`.
    /// A path is excluded when it or a directory above it matches one, so an
    /// excluded tree is never listed, hashed or walked. Probes are untouched.
    #[serde(default)]
    pub exclude: Vec<String>,

    /// Literal paths a successful run of this command is expected to produce,
    /// relative to the project root. The rule is fresh only when its inputs
    /// hash the same AND every declared output exists, so a `cargo clean` or a
    /// fresh clone voids the record. Existence only: an output is stat-ed,
    /// never hashed or walked, so `gitignore` never applies. Patterns are
    /// rejected at load; see [`crate::outputs`].
    #[serde(default)]
    pub outputs: Vec<PathBuf>,

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

    /// How long a successful record stays fresh (`30m`, `12h`, `7d`), for a
    /// command whose verdict depends on the world as well as its inputs. With
    /// no `inputs` at all, time is the rule's only input. See [`crate::ttl`].
    #[serde(default)]
    pub ttl: Option<Ttl>,
}

impl Manifest {
    /// Loads and validates a manifest from `path`, following its `imports:`
    /// chain (if any) and merging every fragment it names into one model. See
    /// [`crate::compose`] for the merge rules; [`Manifest::locate`] is the
    /// entry point that also keeps the provenance this discards.
    ///
    /// # Errors
    ///
    /// Same as [`Located::at`], whose result this discards all but the merged
    /// manifest of.
    pub fn load(path: &Path) -> Result<Self> {
        Ok(Located::at(path)?.manifest)
    }

    /// Checks invariants the schema cannot express: no probe shadows a scope;
    /// command names are present, unique, and reference only declared inputs;
    /// tags are unique per command; declared outputs are literal paths.
    ///
    /// # Errors
    ///
    /// Returns [`Error::NameCollision`], [`Error::EmptyCommandName`],
    /// [`Error::DuplicateCommand`], [`Error::UnknownScope`],
    /// [`Error::UnknownInput`], [`Error::DuplicateTag`], or
    /// [`Error::InvalidOutput`].
    pub fn validate(&self) -> Result<()> {
        crate::probe::validate(&self.probes, &self.scopes, &self.probe_shell)?;
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
            for input in &command.inputs {
                if !self.scopes.contains_key(input) && !self.probes.contains_key(input) {
                    return Err(Error::UnknownInput {
                        command: command.name.clone(),
                        input: input.clone(),
                    });
                }
            }
            crate::outputs::validate(&command.name, &command.outputs)?;
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
    /// gitignore setting they expand under and the patterns they exclude: the
    /// scopes honouring the filter come first, and scopes sharing both a
    /// setting and an exclusion share one deduplicated group.
    ///
    /// Bucketing rather than one group per scope keeps the filesystem walk
    /// count at one per distinct setting and exclusion, so a manifest with no
    /// `exclude` walks at most twice however many scopes a rule references.
    ///
    /// An `inputs` entry naming a probe contributes no patterns and is skipped
    /// here; [`crate::probe::Resolver`] owns that half of the input set.
    ///
    /// # Errors
    ///
    /// Returns [`Error::UnknownInput`] if an entry is neither a declared scope
    /// nor a declared probe.
    pub fn glob_groups(&self, command: &Command) -> Result<Vec<GlobGroup>> {
        let mut groups: Vec<GlobGroup> = Vec::new();
        for name in &command.inputs {
            let Some(scope) = self.scopes.get(name) else {
                if self.probes.contains_key(name) {
                    continue;
                }
                return Err(Error::UnknownInput {
                    command: command.name.clone(),
                    input: name.clone(),
                });
            };
            let wanted = self.scope_group(scope, command);
            let shared = groups.iter_mut().find(|group| {
                group.gitignore == wanted.gitignore && group.exclude == wanted.exclude
            });
            match shared {
                Some(group) => push_unique(&mut group.globs, &wanted.globs),
                None => groups.push(wanted),
            }
        }
        groups.sort_by_key(|group| !group.gitignore);
        Ok(groups)
    }

    /// The group one scope expands as under `command`: its globs, its own
    /// `exclude` followed by the rule's, and its effective gitignore setting.
    #[must_use]
    pub fn scope_group(&self, scope: &Scope, command: &Command) -> GlobGroup {
        let mut globs = Vec::new();
        push_unique(&mut globs, &scope.globs);
        let mut exclude = Vec::new();
        push_unique(&mut exclude, &scope.exclude);
        push_unique(&mut exclude, &command.exclude);
        GlobGroup {
            globs,
            exclude,
            gitignore: scope.honours_gitignore(self.gitignore),
        }
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
    /// it with the project root its relative paths resolve against and the
    /// provenance of every scope, probe and command in the merged model.
    ///
    /// # Errors
    ///
    /// Returns [`Error::NoManifest`] when none is found, plus anything
    /// [`Located::at`] returns for the manifest it found.
    pub fn locate(cwd: &Path) -> Result<Located> {
        let discovered = Self::discover(cwd).ok_or_else(|| Error::NoManifest {
            start: cwd.to_path_buf(),
        })?;
        Located::at(&discovered)
    }
}

/// Appends each of `items` to `into` unless it is already there, keeping order.
fn push_unique(into: &mut Vec<String>, items: &[String]) {
    for item in items {
        if !into.contains(item) {
            into.push(item.clone());
        }
    }
}

/// The discovered manifest and the root it is anchored to — the pairing every
/// entry point above hands back, split out to `manifest_located.rs` once this
/// file reached its own line cap. [`Located`] keeps its original
/// `crate::manifest::Located` path via the re-export below, so nothing outside
/// this module has to know the split happened.
#[path = "manifest_located.rs"]
mod located;
pub use located::Located;

#[cfg(test)]
#[path = "manifest_tests.rs"]
mod tests;
