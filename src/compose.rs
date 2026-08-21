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
//! - `cache_dir`, `gitignore`, `strict`, `on_hit` and `probe_shell` are
//!   root-manifest-only;
//!   any of them in an imported file is [`Error::FragmentPolicyKey`]. A
//!   [`crate::manifest::Command`]'s own `on_hit` is unaffected — different key.
//! - `imports` itself never reaches the merged [`Manifest`]; it is consumed
//!   here.
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
use crate::import_paths::expand_import;
use crate::manifest::{Command, Manifest, Scope, StrictPolicy};
use crate::probe::Probe;
use crate::provenance::Provenance;

/// One manifest file's own declarations, before merge.
///
/// Shaped like [`Manifest`], but every root-only policy field is the
/// double-`Option` idiom (`Option<Option<T>>`, see [`double_option`]) rather
/// than a plain `Option<T>`. A plain `Option<T>` cannot tell three states
/// apart: the key is absent (supplied by `#[serde(default)]`), the key is
/// present and explicitly `null`, or the key is present with a value — the
/// first two both deserialize to `None`. The distinction matters because
/// presence, not value, is what the fragment policy-key check is about: a
/// fragment that writes `gitignore:` (explicit `null`) is still *setting* the
/// key and must be rejected exactly as `gitignore: true` would be, while the
/// root manifest treats an explicit `null` on `gitignore`, `cache_dir` or
/// `strict` as a hard error rather than silently falling through to the
/// default (see [`require_or_default`]). `deny_unknown_fields` applies here
/// exactly as it does on [`Manifest`], so a fragment's syntax is validated per
/// file, same as the root.
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
    #[serde(default, deserialize_with = "double_option")]
    probe_shell: Option<Option<Vec<String>>>,
}

/// Deserializes a field as `Option<Option<T>>`: `#[serde(default)]` supplies
/// the outer `None` when the key is absent, and this function runs only when
/// the key *is* present, wrapping whatever `Option<T>` deserializes to —
/// `None` for an explicit `null`, `Some(value)` otherwise — in one more
/// `Some`. The result is the standard double-`Option` idiom: outer `None` is
/// "omitted", `Some(None)` is "present and `null`", `Some(Some(value))` is
/// "present with a value" — three states a plain `Option<T>` field collapses
/// into two.
fn double_option<'de, D, T>(deserializer: D) -> std::result::Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::deserialize(deserializer).map(Some)
}

/// Accumulates the merge across the whole import tree: the project root (for
/// rendering the paths its errors name), the current chain (for cycle
/// detection), the set of canonical paths already fully merged (for diamond
/// skipping), the order files were first visited in (for
/// [`Provenance::sources`]), and the entries collected so far, each tagged
/// with its source file.
#[derive(Default)]
struct MergeState {
    /// The project root every path these errors name is rendered against —
    /// root-relative under it, absolute otherwise, the rule
    /// [`Provenance::display`] renders a report's rows with. Never resolves
    /// anything: an `imports:` entry resolves against the declaring file's
    /// directory, not this.
    root: PathBuf,
    stack: Vec<PathBuf>,
    loaded: BTreeSet<PathBuf>,
    /// Every visited file, root first, in first-visit order — `loaded` is a
    /// `BTreeSet` for fast membership checks and sorts alphabetically, which
    /// throws away the load order a source list needs, so this tracks it
    /// separately rather than trying to recover it from `loaded`.
    order: Vec<PathBuf>,
    scopes: BTreeMap<String, (Scope, PathBuf)>,
    probes: BTreeMap<String, (Probe, PathBuf)>,
    commands: Vec<(Command, PathBuf)>,
}

impl MergeState {
    /// Folds one file's own scopes, probes and commands into the merge,
    /// tagging each with `source`.
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
                    first: Provenance::shorten(first, &self.root),
                    second: Provenance::shorten(source, &self.root),
                });
            }
            self.scopes.insert(name, (scope, source.to_path_buf()));
        }
        for (name, probe) in probes {
            if let Some((_, first)) = self.probes.get(&name) {
                return Err(Error::DuplicateProbe {
                    name,
                    first: Provenance::shorten(first, &self.root),
                    second: Provenance::shorten(source, &self.root),
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
    /// order, folding each target's own declarations into the merge before
    /// moving to the next entry — the depth-first half of "host first, then
    /// imports depth-first".
    ///
    /// # Errors
    ///
    /// Returns [`Error::ImportMissing`] for a path that does not exist, or
    /// propagates any error from loading a target fragment.
    fn visit_imports(&mut self, importer: &Path, imports: &[String]) -> Result<()> {
        let base_dir = importer
            .parent()
            .ok_or_else(|| Error::Internal("import path has no parent directory".to_owned()))?;
        for entry in imports {
            for target in expand_import(importer, base_dir, entry, &self.root)? {
                self.visit_fragment(&target)?;
            }
        }
        Ok(())
    }

    /// Loads one fragment file: cycle- and diamond-checks it, reads and
    /// parses it, rejects any root-only policy key, then absorbs its own
    /// declarations before recursing into its own `imports:`.
    ///
    /// A path already on the current stack is a cycle; a path already fully
    /// loaded but not on the stack is a diamond and is skipped silently.
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
                chain: format_chain(&chain, &self.root),
            });
        }
        if self.loaded.contains(&canonical) {
            return Ok(());
        }

        let text = fs::read_to_string(&canonical).map_err(|source| Error::ImportNotReadable {
            path: Provenance::shorten(&canonical, &self.root),
            source,
        })?;
        let document = parse_text(&text, &canonical, &self.root)?;
        check_no_policy_keys(&document, &canonical, &self.root)?;

        self.stack.push(canonical.clone());
        self.order.push(canonical.clone());
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
/// `root` is the project root, and is used for one thing: rendering the paths
/// these errors name, root-relative under it and absolute otherwise. That is
/// [`Provenance::display`]'s rule, reused rather than reinvented, so an error
/// names a file exactly as `--status` and `--dump-config` do — and so the
/// store-path case composition exists for still reads as an absolute path,
/// which is the useful form for a file outside the tree. Resolution never
/// consults it: an `imports:` entry resolves against the declaring file's
/// directory. [`crate::manifest::Located::at`] canonicalizes it, which is
/// what lets it strip a prefix off the canonical paths recorded here.
///
/// # Errors
///
/// Returns [`Error::Io`] if `path` cannot be read, [`Error::ManifestParse`] if
/// any file in the chain fails to parse, an import error
/// ([`Error::ImportMissing`], [`Error::ImportNotReadable`],
/// [`Error::ImportCycle`], [`Error::DuplicateScope`], [`Error::DuplicateProbe`],
/// [`Error::DuplicateCommandAcrossFiles`], [`Error::FragmentPolicyKey`],
/// [`Error::NullPolicyKey`]), or a validation error from [`Manifest::validate`].
pub(crate) fn load(path: &Path, root: &Path) -> Result<(Manifest, Provenance)> {
    let text = fs::read_to_string(path)?;
    let document = parse_text(&text, path, root)?;
    let canonical = fs::canonicalize(path)?;

    let Resolved {
        gitignore,
        cache_dir,
        strict,
        on_hit,
        probe_shell,
    } = policy::resolve(&document, path, root)?;

    let mut state = MergeState {
        root: root.to_path_buf(),
        ..MergeState::default()
    };
    state.stack.push(canonical.clone());
    state.order.push(canonical.clone());
    state.absorb(
        &canonical,
        document.scopes,
        document.probes,
        document.commands,
    )?;
    state.visit_imports(&canonical, &document.imports)?;
    state.stack.pop();
    state.loaded.insert(canonical);

    check_cross_file_duplicate_commands(&state.commands, root)?;

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
        probe_shell,
    };
    manifest.validate()?;

    Ok((
        manifest,
        Provenance {
            sources: state.order,
            scopes: scope_sources,
            probes: probe_sources,
            commands: command_sources,
        },
    ))
}

/// Parses `text` (read from `path`) into a [`Document`], naming `path` on
/// failure — always the file actually being parsed, root or fragment, so a
/// syntax error inside a fragment never gets blamed on the root manifest —
/// rendered against `root` (see [`load`]).
fn parse_text(text: &str, path: &Path, root: &Path) -> Result<Document> {
    serde_yaml_ng::from_str(text).map_err(|source| Error::ManifestParse {
        path: Provenance::shorten(path, root),
        source: Box::new(source),
    })
}

/// The root-manifest-only keys: `cache_dir`, `gitignore`, `strict`, `on_hit`
/// and `probe_shell` may be set on the root manifest but never on an imported
/// fragment. This is the single source of truth for that surface — the only
/// place the four names are spelled as literals — and it has two readers:
/// `policy::check_no_policy_keys`, the rule actually enforced at load time,
/// and the derivation test in `crate::schema` that asserts the fragment
/// JSON Schema forbids exactly what this array names, so the schema (the
/// discoverable form of the rule) cannot drift from the loader (the rule
/// itself). Defined here rather than in `policy` below despite being used
/// almost entirely there: `policy`'s own non-test code is its only
/// unconditional user, and a `pub(crate) use` re-export of a name nothing
/// outside `#[cfg(test)]` calls is flagged unused in a plain build — the
/// const itself, sitting in the module that also reaches for it via
/// `super::POLICY_KEYS`, has no such problem.
pub(crate) const POLICY_KEYS: [&str; 5] =
    ["cache_dir", "gitignore", "strict", "on_hit", "probe_shell"];

/// The check that rejects a policy key outside the root, and the query for
/// whether one was written or defaulted — split out to `compose_policy.rs`
/// once this file reached its own line cap. [`declared_policy_keys`] stays
/// reachable at its original `crate::compose::…` path via the re-export
/// below, so nothing outside this module has to know the split happened.
#[path = "compose_policy.rs"]
mod policy;
pub(crate) use policy::declared_policy_keys;
use policy::{Resolved, check_no_policy_keys};

/// Renders an import chain root first, e.g. `a.yaml -> b.yaml -> a.yaml`,
/// each link rendered against `root` (see [`load`]).
fn format_chain(chain: &[PathBuf], root: &Path) -> String {
    chain
        .iter()
        .map(|entry| Provenance::display(entry, root))
        .collect::<Vec<_>>()
        .join(" -> ")
}

/// Checks the fully merged command list for a name declared in two
/// *different* files. A name repeated within one file is left for
/// [`Manifest::validate`] to catch with its existing single-file message.
///
/// # Errors
///
/// Returns [`Error::DuplicateCommandAcrossFiles`] naming the two files,
/// rendered against `root` (see [`load`]).
fn check_cross_file_duplicate_commands(commands: &[(Command, PathBuf)], root: &Path) -> Result<()> {
    let mut first_seen: BTreeMap<&str, &Path> = BTreeMap::new();
    for (command, source) in commands {
        match first_seen.get(command.name.as_str()) {
            Some(&existing) if existing != source.as_path() => {
                return Err(Error::DuplicateCommandAcrossFiles {
                    name: command.name.clone(),
                    first: Provenance::shorten(existing, root),
                    second: Provenance::shorten(source, root),
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

#[cfg(test)]
#[path = "compose_merge_tests.rs"]
mod merge_tests;

#[cfg(test)]
#[path = "compose_cycles_tests.rs"]
mod cycles_tests;
