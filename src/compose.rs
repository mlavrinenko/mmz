//! Import resolution, cycle detection and merge for manifest composition.
//!
//! A manifest may declare a top-level `imports:` list of paths; a fragment
//! named there may declare its own `imports:` in turn. [`load`] walks that
//! chain and folds every fragment's `scopes`, `probes` and `commands` into one
//! [`Manifest`], recording which file contributed each entry (see
//! [`Provenance`]) so the duplicate-key errors below can name both sources and
//! later tooling (`--status`, `--dump-config`) can answer "which file made
//! this rule skip?".
//!
//! # Merge rules
//!
//! - `scopes` and `probes` merge by key; a key declared in two files is
//!   [`Error::DuplicateScope`] or [`Error::DuplicateProbe`], never last-wins.
//! - `commands` merge in listed order: the importing file's own rules first,
//!   then each import in listed order, depth-first. A `name` declared in two
//!   *different* files is [`Error::DuplicateCommandAcrossFiles`]; the same name
//!   twice in *one* file is still [`Error::DuplicateCommand`], caught by
//!   [`Manifest::validate`] exactly as it is today.
//! - `cache_dir`, `gitignore`, `strict` and `on_hit` are root-manifest-only,
//!   set or explicit `null` alike (see [`double_option`]); either is
//!   [`Error::FragmentPolicyKey`] in an imported file. A
//!   [`crate::manifest::Command`]'s own `on_hit` is unaffected — different key.
//! - `imports` itself never reaches the merged [`Manifest`]; it is consumed.
//!
//! # Paths
//!
//! A directory entry expands to the `*.yaml` and `*.yml` files directly inside
//! it (not recursive), sorted lexically by file name; an empty directory is
//! fine, a missing file or directory is [`Error::ImportMissing`]. Relative
//! entries resolve against the *declaring file's* directory, not the project
//! root — the one resolution rule under which a fragment can reference a
//! sibling fragment at all. Absolute paths are used as written.
//!
//! Every path is canonicalized before it is recorded, compared, or merged, so
//! a symlinked `conf.d` entry and a store path both behave: a path already on
//! the current import stack is [`Error::ImportCycle`]; a path already fully
//! loaded but not on the stack is a diamond (the same file reached twice by
//! different routes) and loads once, silently.
//!
//! # Validation
//!
//! Each file deserializes on its own — `deny_unknown_fields` and the
//! policy-key rejection both happen per file — but [`Manifest::validate`] runs
//! exactly once, against the merged model. A fragment may reference a scope a
//! sibling defines.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Deserializer};

use crate::error::{Error, Result};
use crate::manifest::{
    Command, Manifest, Scope, StrictPolicy, default_cache_dir, default_gitignore,
};
use crate::probe::Probe;

/// Which file contributed each scope, probe and command in a merged manifest.
///
/// The root manifest is recorded as the source of its own entries — a
/// single-file project is the degenerate case, not a special case — so a
/// lookup never needs to special-case "no imports". Every path is
/// canonicalized; use [`Provenance::display`] to render one the way a user
/// should see it.
#[derive(Debug, Clone, Default)]
pub struct Provenance {
    /// Source file of each scope, keyed by scope name.
    pub scopes: BTreeMap<String, PathBuf>,
    /// Source file of each probe, keyed by probe name.
    pub probes: BTreeMap<String, PathBuf>,
    /// Source file of each command, keyed by command name (unique once the
    /// merged manifest has validated).
    pub commands: BTreeMap<String, PathBuf>,
}

impl Provenance {
    /// Renders `path` project-root-relative when it is under `root`, absolute
    /// otherwise — so an out-of-tree fragment (a Nix store path, say) stays
    /// recognisable instead of collapsing into a long `../../..` climb.
    #[must_use]
    pub fn display(path: &Path, root: &Path) -> String {
        match path.strip_prefix(root) {
            Ok(relative) => relative.display().to_string(),
            Err(_) => path.display().to_string(),
        }
    }
}

/// One manifest file's own declarations, before merge.
///
/// Shaped like [`Manifest`], but every root-only policy field is the
/// double-`Option` idiom ([`double_option`]) rather than a plain `Option<T>`,
/// which cannot tell "absent" from "present and explicitly `null`" apart.
/// `deny_unknown_fields` applies here exactly as it does on [`Manifest`], so a
/// fragment's syntax is validated per file, same as the root.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    #[serde(default)]
    imports: Vec<String>,
    #[serde(default)]
    scopes: BTreeMap<String, Scope>,
    #[serde(default)]
    probes: BTreeMap<String, Probe>,
    #[serde(default)]
    commands: Vec<Command>,
    #[serde(default, deserialize_with = "double_option")]
    gitignore: Option<Option<bool>>,
    #[serde(default, deserialize_with = "double_option")]
    cache_dir: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    strict: Option<Option<StrictPolicy>>,
    #[serde(default, deserialize_with = "double_option")]
    on_hit: Option<Option<String>>,
}

/// Deserializes a field as `Option<Option<T>>`, wrapping a present key —
/// `null` or a value — in `Some`; `#[serde(default)]` handles absence. The
/// standard double-`Option` idiom for telling "omitted" from "explicitly
/// null" apart, which a plain `Option<T>` field cannot.
fn double_option<'de, D, T>(deserializer: D) -> std::result::Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::deserialize(deserializer).map(Some)
}

/// Accumulates the merge across the whole import tree: the current chain (for
/// cycle detection), the set of canonical paths already fully merged (for
/// diamond skipping), and the entries collected so far, each tagged with its
/// source file.
#[derive(Default)]
struct MergeState {
    stack: Vec<PathBuf>,
    loaded: BTreeSet<PathBuf>,
    scopes: BTreeMap<String, (Scope, PathBuf)>,
    probes: BTreeMap<String, (Probe, PathBuf)>,
    commands: Vec<(Command, PathBuf)>,
}

impl MergeState {
    /// Folds one file's scopes, probes and commands into the merge, tagging each
    /// with `source`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::DuplicateScope`] or [`Error::DuplicateProbe`] when a
    /// key was already contributed by an earlier file.
    fn absorb(
        &mut self,
        source: &Path,
        scopes: BTreeMap<String, Scope>,
        probes: BTreeMap<String, Probe>,
        commands: Vec<Command>,
    ) -> Result<()> {
        for (name, scope) in scopes {
            if let Some((_, first)) = self.scopes.get(&name) {
                return Err(Error::DuplicateScope {
                    name,
                    first: first.clone(),
                    second: source.to_path_buf(),
                });
            }
            self.scopes.insert(name, (scope, source.to_path_buf()));
        }
        for (name, probe) in probes {
            if let Some((_, first)) = self.probes.get(&name) {
                return Err(Error::DuplicateProbe {
                    name,
                    first: first.clone(),
                    second: source.to_path_buf(),
                });
            }
            self.probes.insert(name, (probe, source.to_path_buf()));
        }
        for command in commands {
            self.commands.push((command, source.to_path_buf()));
        }
        Ok(())
    }

    /// Resolves and visits every entry in `importer`'s `imports:` list, in
    /// order — the depth-first half of "host first, then imports depth-first".
    ///
    /// # Errors
    ///
    /// Returns [`Error::ImportMissing`] for a path that does not exist, or
    /// propagates an error from loading a target fragment.
    fn visit_imports(&mut self, importer: &Path, imports: &[String]) -> Result<()> {
        let base_dir = importer
            .parent()
            .ok_or_else(|| Error::Internal("import path has no parent directory".to_owned()))?;
        for entry in imports {
            for target in expand_import(importer, base_dir, entry)? {
                self.visit_fragment(&target)?;
            }
        }
        Ok(())
    }

    /// Loads one fragment: cycle- and diamond-checks it, reads and parses it,
    /// rejects any root-only policy key, then absorbs its declarations before
    /// recursing into its own `imports:`. A path already on the stack is a
    /// cycle; already loaded but off the stack is a diamond, skipped silently.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ImportCycle`], [`Error::ImportNotReadable`],
    /// [`Error::ManifestParse`], [`Error::FragmentPolicyKey`], or propagates a
    /// merge error from [`MergeState::absorb`] or a nested
    /// [`MergeState::visit_imports`].
    fn visit_fragment(&mut self, path: &Path) -> Result<()> {
        let canonical = fs::canonicalize(path)?;
        if self.stack.contains(&canonical) {
            let mut chain = self.stack.clone();
            chain.push(canonical);
            return Err(Error::ImportCycle {
                chain: format_chain(&chain),
            });
        }
        if self.loaded.contains(&canonical) {
            return Ok(());
        }

        let text = fs::read_to_string(&canonical).map_err(|source| Error::ImportNotReadable {
            path: canonical.clone(),
            source,
        })?;
        let document = parse_text(&text, &canonical)?;
        check_no_policy_keys(&document, &canonical)?;

        self.stack.push(canonical.clone());
        self.absorb(
            &canonical,
            document.scopes,
            document.probes,
            document.commands,
        )?;
        self.visit_imports(&canonical, &document.imports)?;
        self.stack.pop();
        self.loaded.insert(canonical);
        Ok(())
    }
}

/// Loads `path` as a manifest, following its `imports:` chain and merging
/// every fragment it names into one model, then validates the merge exactly
/// once. The root manifest — `path` itself — behaves exactly as it did before
/// composition existed: the same read, the same parse, the same validation,
/// so a manifest with no `imports:` key is unaffected byte-for-byte.
///
/// # Errors
///
/// Returns [`Error::Io`] if `path` cannot be read, [`Error::ManifestParse`] if
/// any file in the chain fails to parse, an import error
/// ([`Error::ImportMissing`], [`Error::ImportNotReadable`],
/// [`Error::ImportCycle`], [`Error::DuplicateScope`], [`Error::DuplicateProbe`],
/// [`Error::DuplicateCommandAcrossFiles`], [`Error::FragmentPolicyKey`],
/// [`Error::NullPolicyKey`]), or a validation error from [`Manifest::validate`].
pub(crate) fn load(path: &Path) -> Result<(Manifest, Provenance)> {
    let text = fs::read_to_string(path)?;
    let document = parse_text(&text, path)?;
    let canonical = fs::canonicalize(path)?;

    let gitignore = require_or_default(document.gitignore, "gitignore", path, default_gitignore)?;
    let cache_dir = require_or_default(document.cache_dir, "cache_dir", path, default_cache_dir)?;
    let strict = require_or_default(document.strict, "strict", path, StrictPolicy::all)?;
    // Unlike the other three, an explicit `on_hit: null` in the root has
    // always been legal — `Manifest::on_hit` is `Option<String>` — so it
    // collapses to `None` exactly like an absent key, not an error.
    let on_hit = document.on_hit.flatten();

    let mut state = MergeState::default();
    state.stack.push(canonical.clone());
    state.absorb(
        &canonical,
        document.scopes,
        document.probes,
        document.commands,
    )?;
    state.visit_imports(&canonical, &document.imports)?;
    state.stack.pop();
    state.loaded.insert(canonical);

    check_cross_file_duplicate_commands(&state.commands)?;

    let (scopes, scope_sources) = split(state.scopes);
    let (probes, probe_sources) = split(state.probes);
    let (commands, command_sources) = split_commands(state.commands);

    let manifest = Manifest {
        scopes,
        probes,
        commands,
        gitignore,
        cache_dir,
        strict,
        on_hit,
    };
    manifest.validate()?;

    Ok((
        manifest,
        Provenance {
            scopes: scope_sources,
            probes: probe_sources,
            commands: command_sources,
        },
    ))
}

/// Resolves one root-only policy field's double-`Option`: absent uses
/// `default`, a value is used as written, and an explicit `null` is
/// [`Error::NullPolicyKey`] rather than falling through to `default` — an
/// author who wrote `gitignore:` meaning `false` must not silently get `true`.
///
/// # Errors
///
/// Returns [`Error::NullPolicyKey`] naming `key` and `path` on `Some(None)`.
fn require_or_default<T>(
    value: Option<Option<T>>,
    key: &str,
    path: &Path,
    default: impl FnOnce() -> T,
) -> Result<T> {
    match value {
        None => Ok(default()),
        Some(None) => Err(Error::NullPolicyKey {
            key: key.to_owned(),
            path: path.to_path_buf(),
        }),
        Some(Some(resolved)) => Ok(resolved),
    }
}

/// Parses `text` (read from `path`) into a [`Document`], naming `path` on
/// failure — always the file actually being parsed, root or fragment, so a
/// syntax error inside a fragment never gets blamed on the root manifest.
fn parse_text(text: &str, path: &Path) -> Result<Document> {
    serde_yaml_ng::from_str(text).map_err(|source| Error::ManifestParse {
        path: path.to_path_buf(),
        source: Box::new(source),
    })
}

/// Rejects a fragment that sets a root-only policy key, naming the first one
/// found and the fragment that set it. `is_some` on the outer double-`Option`
/// is exactly "present", so `gitignore:` (explicit `null`) is caught here just
/// as `gitignore: true` is — presence is the rule, not the value.
///
/// # Errors
///
/// Returns [`Error::FragmentPolicyKey`] if any of `gitignore`, `cache_dir`,
/// `strict` or `on_hit` is present, `null` included.
fn check_no_policy_keys(document: &Document, path: &Path) -> Result<()> {
    let present = [
        ("gitignore", document.gitignore.is_some()),
        ("cache_dir", document.cache_dir.is_some()),
        ("strict", document.strict.is_some()),
        ("on_hit", document.on_hit.is_some()),
    ];
    if let Some((key, _)) = present.into_iter().find(|(_, set)| *set) {
        return Err(Error::FragmentPolicyKey {
            key: key.to_owned(),
            path: path.to_path_buf(),
        });
    }
    Ok(())
}

/// Resolves one `imports:` entry declared in `importer` (whose directory is
/// `base_dir`) to the manifest files it names: itself, if it names a file, or
/// the `*.yaml`/`*.yml` files directly inside it, lexically sorted, if it
/// names a directory.
///
/// # Errors
///
/// Returns [`Error::ImportMissing`] if the resolved path is neither a file nor
/// a directory.
fn expand_import(importer: &Path, base_dir: &Path, entry: &str) -> Result<Vec<PathBuf>> {
    let candidate = PathBuf::from(entry);
    let resolved = if candidate.is_absolute() {
        candidate
    } else {
        base_dir.join(candidate)
    };
    if resolved.is_dir() {
        return list_yaml_files(&resolved);
    }
    if resolved.is_file() {
        return Ok(vec![resolved]);
    }
    Err(Error::ImportMissing {
        importer: importer.to_path_buf(),
        path: resolved,
    })
}

/// Lists the `*.yaml` and `*.yml` files directly inside `dir` (not recursive),
/// sorted lexically by file name.
fn list_yaml_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let entries = fs::read_dir(dir).map_err(|source| Error::ImportNotReadable {
        path: dir.to_path_buf(),
        source,
    })?;
    for entry in entries {
        let entry = entry.map_err(|source| Error::ImportNotReadable {
            path: dir.to_path_buf(),
            source,
        })?;
        let candidate = entry.path();
        if !candidate.is_file() {
            continue;
        }
        let is_yaml = candidate
            .extension()
            .and_then(std::ffi::OsStr::to_str)
            .is_some_and(|extension| extension == "yaml" || extension == "yml");
        if is_yaml {
            files.push(candidate);
        }
    }
    files.sort();
    Ok(files)
}

/// Renders an import chain root first, e.g. `a.yaml -> b.yaml -> a.yaml`.
fn format_chain(chain: &[PathBuf]) -> String {
    chain
        .iter()
        .map(|entry| entry.display().to_string())
        .collect::<Vec<_>>()
        .join(" -> ")
}

/// Checks the fully merged command list for a name declared in two
/// *different* files. A name repeated within one file is left for
/// [`Manifest::validate`] to catch with its existing single-file message.
///
/// # Errors
///
/// Returns [`Error::DuplicateCommandAcrossFiles`] naming the two files.
fn check_cross_file_duplicate_commands(commands: &[(Command, PathBuf)]) -> Result<()> {
    let mut first_seen: BTreeMap<&str, &Path> = BTreeMap::new();
    for (command, source) in commands {
        match first_seen.get(command.name.as_str()) {
            Some(&existing) if existing != source.as_path() => {
                return Err(Error::DuplicateCommandAcrossFiles {
                    name: command.name.clone(),
                    first: existing.to_path_buf(),
                    second: source.clone(),
                });
            }
            Some(_) => {}
            None => {
                first_seen.insert(command.name.as_str(), source.as_path());
            }
        }
    }
    Ok(())
}

/// Splits a merged `name -> (value, source)` map into the plain value map
/// [`Manifest`] wants and the parallel source map [`Provenance`] wants.
fn split<V>(
    merged: BTreeMap<String, (V, PathBuf)>,
) -> (BTreeMap<String, V>, BTreeMap<String, PathBuf>) {
    let mut values = BTreeMap::new();
    let mut sources = BTreeMap::new();
    for (name, (value, source)) in merged {
        sources.insert(name.clone(), source);
        values.insert(name, value);
    }
    (values, sources)
}

/// Splits the merged, ordered command list into the plain `Vec<Command>`
/// [`Manifest`] wants and a `name -> source` map for [`Provenance`]. Keeps the
/// *first* source per name: a same-file duplicate leaves the merge invalid
/// (caught right after by [`Manifest::validate`]) so its provenance is never
/// read.
fn split_commands(merged: Vec<(Command, PathBuf)>) -> (Vec<Command>, BTreeMap<String, PathBuf>) {
    let mut commands = Vec::with_capacity(merged.len());
    let mut sources: BTreeMap<String, PathBuf> = BTreeMap::new();
    for (command, source) in merged {
        sources.entry(command.name.clone()).or_insert(source);
        commands.push(command);
    }
    (commands, sources)
}

#[cfg(test)]
#[path = "compose_tests.rs"]
mod tests;
