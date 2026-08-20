//! `mmz --dump-config`: print the merged manifest with the source file of
//! every scope, probe and command.
//!
//! Once a manifest can be assembled from several files (see
//! [`crate::compose`]), the merged model itself hides the import graph that
//! produced it — a scope, probe or command reads the same whether it came
//! from the root manifest or the third fragment three imports deep, and the
//! five keys that govern the whole run (`gitignore`, `cache_dir`, `strict`,
//! `on_hit`, `probe_shell`) are invisible once they have fallen back to
//! their defaults.
//! This module answers two questions composition raises and `--status` does
//! not: a person asking "which file made this rule skip?", "why did this
//! pass straight through instead of erroring?" (`strict`), "why is nothing
//! printed when it skips?" (`on_hit`), or "why does this scope resolve
//! empty?" (`gitignore`) needs more than the rule `--status` already names
//! (see `mmz-report-each-rule-s-source-file-in-status`) — none of those are
//! about a rule's freshness; and a generator emitting a fragment wants a
//! gate hook to assert the fragment it wrote is the one actually in effect,
//! not merely present on disk.
//!
//! Two renderings share one model, exactly as [`crate::status`] does: a human
//! form (`mmz --dump-config`, rendered by [`render_text`]) that leads with
//! the source list in load order — so the import graph is visible before the
//! entries it fed are — then the effective policy (manifest-wide context, so
//! it comes before any entry section), then each entry section, every entry
//! annotated with the file it came from; and a machine form
//! (`mmz --dump-config=json`) carrying the same facts under stable keys,
//! because a gate hook asserting against this output is half the point.
//! There is deliberately no `=json-schema` arm and no
//! `schema/config-dump.schema.json`: the only consumer today is a gate that
//! can assert on keys directly, and a schema for a document with one
//! consumer is premature (see the task's `Deferred` section).
//!
//! Both renderings print the merged manifest *after* validation. A manifest
//! that fails to load or merge exits 4 with the same error every other reader
//! produces (see [`crate::manifest::Manifest::locate`]); this module is not a
//! debugging aid for that failure, and prints no partial dump — [`collect`]
//! either returns a complete model or an error, never a partly-built one, so
//! there is nothing to accidentally emit on the failure path.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::error::{Error, Result};
use crate::manifest::{Manifest, MatchMode, StrictCase};
use crate::provenance::Provenance;

/// The merged manifest, every scope, probe and command carrying the file it
/// came from.
#[derive(Serialize)]
struct Dump {
    /// The root manifest's path, display-rendered exactly like `sources[0]` —
    /// a stable, order-independent key to the same fact, so a consumer does
    /// not have to assume index 0 is special. Deliberately unlike
    /// [`crate::status`]'s own `manifest` field, which stays absolute: every
    /// other path this report carries is root-relative-when-possible, and a
    /// report whose whole point is import-graph legibility should not have
    /// the one field naming the root itself be the odd one out.
    manifest: String,
    /// Every file that contributed to the merge, in load order (see
    /// [`Provenance::sources`]), numbered from 1 in the human form.
    sources: Vec<String>,
    /// The five manifest-wide keys, resolved to their effective values.
    /// Manifest-wide context rather than an entry, so it sits ahead of the
    /// three entry sections rather than among them.
    policy: Policy,
    scopes: Vec<ScopeEntry>,
    probes: Vec<ProbeEntry>,
    commands: Vec<CommandEntry>,
}

/// The five manifest-wide policy keys, resolved to their effective
/// values — defaulted ones included, because "what is mmz actually using"
/// is the question this section answers, and an absent key does not answer
/// it. [`crate::compose::check_no_policy_keys`] rejects every one of these
/// outside the root manifest, so unlike a scope, probe or command there is
/// one `source` for the whole section rather than one per key.
#[derive(Serialize)]
struct Policy {
    source: String,
    gitignore: bool,
    cache_dir: String,
    /// The enforced [`StrictCase`]s, spelled the way the manifest spells
    /// them (`"no_match"`, `"no_inputs"`); empty when `strict: []` relaxes
    /// every case.
    strict: Vec<&'static str>,
    /// `null` when no default `on_hit` is set, present unconditionally
    /// (unlike [`CommandEntry::on_hit`], which omits the key entirely) —
    /// this field answers "what does mmz use", and "the key is missing"
    /// does not distinguish "unset" from "a consumer that forgot to check".
    on_hit: Option<String>,
    /// The argv a probe's `run` line is executed by, resolved to its
    /// effective value — `["sh", "-c"]` unless the root pinned one.
    probe_shell: Vec<String>,
    /// Which of the five keys above the root manifest wrote explicitly
    /// rather than leaving to its default. Read only by the human form's
    /// trailing `(default)` marker — omitted from JSON on purpose: a
    /// defaulted value and a written one are indistinguishable once
    /// resolved, a consumer parsing JSON almost always wants the effective
    /// value rather than its provenance, and a second boolean per key would
    /// be exactly the kind of heavy encoding a boring, stable schema does
    /// not need.
    #[serde(skip)]
    declared: BTreeSet<&'static str>,
}

/// One merged scope: its declared globs and gitignore override, plus the file
/// that declared it.
#[derive(Serialize)]
struct ScopeEntry {
    name: String,
    globs: Vec<String>,
    /// The scope's own override of the manifest-level `gitignore` filter;
    /// omitted when the scope does not set one and inherits it instead.
    #[serde(skip_serializing_if = "Option::is_none")]
    gitignore: Option<bool>,
    source: String,
}

/// One merged probe: whichever source it declared, whichever selector it
/// narrows with, its `allow_empty` flag, and the file that declared it.
///
/// `run` and `file` are skipped when absent rather than emitted as `null`,
/// because exactly one of them is always set — a reader scanning the dump
/// should see the source the probe has, not a null beside it.
#[derive(Serialize)]
struct ProbeEntry {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    run: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    json: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ast: Option<String>,
    /// The `capture:` list as the manifest wrote it, order included — this is
    /// an audit of the source, not of what the hasher does with it, and the
    /// hasher's own sort is documented in [`crate::ast_render`].
    #[serde(skip_serializing_if = "Option::is_none")]
    capture: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    lang: Option<String>,
    /// Always present, unlike the manifest's own `#[serde(default)]`
    /// spelling of this field: a boring, stable schema names every key
    /// regardless of whether the value happens to be the default.
    allow_empty: bool,
    source: String,
}

/// One merged command rule: its matcher and cache inputs, plus the file that
/// declared it.
#[derive(Serialize)]
struct CommandEntry {
    name: String,
    /// `"prefix"` or `"exact"`, the manifest's own spelling of
    /// [`MatchMode`] — always present, unlike the other fields here, because
    /// every rule has a match mode even when it is the default.
    #[serde(rename = "match")]
    match_mode: &'static str,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    inputs: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    outputs: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    on_hit: Option<String>,
    source: String,
}

/// Builds the human-readable dump of the manifest governing `cwd`.
///
/// # Errors
///
/// Returns [`Error::NoManifest`] when none is found, or any load or
/// validation error the merge produces — see
/// [`crate::manifest::Manifest::locate`].
pub fn dump(cwd: &Path) -> Result<String> {
    let dump = collect(cwd)?;
    Ok(render_text(&dump))
}

/// Builds the `mmz --dump-config=json` report: the same model as [`dump`],
/// serialized to pretty JSON.
///
/// # Errors
///
/// Same as [`dump`], plus [`Error::Internal`] if serialization fails.
pub fn dump_json(cwd: &Path) -> Result<String> {
    let dump = collect(cwd)?;
    let text = serde_json::to_string_pretty(&dump)
        .map_err(|err| Error::Internal(format!("serializing dump-config json: {err}")))?;
    Ok(format!("{text}\n"))
}

/// Resolves and validates the manifest, then pairs every scope, probe and
/// command with the file [`Provenance`] recorded for it. Building the whole
/// model before either rendering touches it is what keeps a failed load from
/// ever reaching stdout as a partial dump — an `Err` here is the only thing a
/// caller can do with it.
fn collect(cwd: &Path) -> Result<Dump> {
    let located = Manifest::locate(cwd)?;
    let manifest = &located.manifest;
    let base = located.root.as_path();
    let provenance = &located.provenance;

    let sources = provenance
        .sources
        .iter()
        .map(|path| Provenance::display(path, base))
        .collect();

    let policy = collect_policy(manifest, &located.path, base)?;

    let scopes = manifest
        .scopes
        .iter()
        .map(|(name, scope)| ScopeEntry {
            name: name.clone(),
            globs: scope.globs.clone(),
            gitignore: scope.gitignore,
            source: source_of(&provenance.scopes, name, base),
        })
        .collect();

    let probes = manifest
        .probes
        .iter()
        .map(|(name, probe)| ProbeEntry {
            name: name.clone(),
            run: probe.run.clone(),
            file: probe.file.as_ref().map(|path| path.display().to_string()),
            json: probe.json.clone(),
            ast: probe.ast.clone(),
            capture: probe.capture.clone(),
            lang: probe.lang.clone(),
            allow_empty: probe.allow_empty,
            source: source_of(&provenance.probes, name, base),
        })
        .collect();

    let commands = manifest
        .commands
        .iter()
        .map(|command| CommandEntry {
            name: command.name.clone(),
            match_mode: match command.match_mode {
                MatchMode::Prefix => "prefix",
                MatchMode::Exact => "exact",
            },
            inputs: command.inputs.clone(),
            outputs: command
                .outputs
                .iter()
                .map(|path| path.display().to_string())
                .collect(),
            tags: command.tags.clone(),
            on_hit: command.on_hit.clone(),
            source: source_of(&provenance.commands, &command.name, base),
        })
        .collect();

    Ok(Dump {
        manifest: Provenance::display(&located.path, base),
        sources,
        policy,
        scopes,
        probes,
        commands,
    })
}

/// Builds the [`Policy`] section: the manifest's five policy fields, already
/// resolved to their effective values by [`Manifest::locate`], paired with
/// which of them the root actually wrote (see
/// [`crate::compose::declared_policy_keys`]). Split out of [`collect`] to
/// keep that function under clippy's line-count lint — this is a cohesive
/// unit on its own, not a slice taken for size alone.
fn collect_policy(manifest: &Manifest, root_path: &Path, base: &Path) -> Result<Policy> {
    let declared = crate::compose::declared_policy_keys(root_path, base)?;
    Ok(Policy {
        source: Provenance::display(root_path, base),
        gitignore: manifest.gitignore,
        cache_dir: manifest.cache_dir.clone(),
        strict: [
            (StrictCase::NoMatch, "no_match"),
            (StrictCase::NoInputs, "no_inputs"),
        ]
        .into_iter()
        .filter(|(case, _)| manifest.strict.enforces(*case))
        .map(|(_, label)| label)
        .collect(),
        on_hit: manifest.on_hit.clone(),
        probe_shell: manifest.probe_shell.clone(),
        declared,
    })
}

/// Looks up `name`'s declaring file in one of [`Provenance`]'s maps and
/// renders it through [`Provenance::display`]. Every name iterated out of the
/// merged [`Manifest`] has a provenance entry by construction — the merge
/// records one for every scope, probe and command it absorbs — so a missing
/// entry means the merge and the provenance it carries have fallen out of
/// sync with each other.
fn source_of(
    sources: &std::collections::BTreeMap<String, PathBuf>,
    name: &str,
    base: &Path,
) -> String {
    let path = sources
        .get(name)
        .expect("provenance recorded for every entry the merged manifest carries");
    Provenance::display(path, base)
}

/// Renders the human `mmz --dump-config` output: the source list numbered in
/// load order; then `policy:`, always printed (it is five keys, never
/// empty), naming the root manifest once on its header line rather than per
/// key — every key in it can only come from the root, so a `# <source>` on
/// each one would imply a choice that does not exist — and marking a
/// defaulted key with a trailing `(default)`; then each non-empty entry
/// section, each entry annotated with a trailing `# <source>` comment on its
/// header line — the same shape a merged `config.yaml` would have if one
/// file held everything, so the format needs no legend of its own.
///
/// An entry section with no entries (e.g. a manifest with no probes) is
/// omitted entirely rather than printed with an empty body, matching how
/// `--status`'s own conditional columns only appear when there is something
/// to show.
/// Renders the `policy:` block of the human form: every one of the five keys,
/// each marked `(default)` when the root left it unwritten. Split out of
/// [`render_text`] to keep that function under clippy's line-count lint — the
/// same seam [`collect_policy`] takes on the building side, so the section has
/// one function per side rather than being inlined into a longer whole.
fn render_policy(policy: &Policy) -> String {
    let on_hit = policy
        .on_hit
        .as_deref()
        .map_or_else(|| "(none)".to_owned(), |value| format!("{value:?}"));
    let probe_shell = policy
        .probe_shell
        .iter()
        .map(|part| format!("{part:?}"))
        .collect::<Vec<_>>()
        .join(", ");
    let mut out = format!("\npolicy:  # {}\n", policy.source);
    for (key, value) in [
        ("gitignore", policy.gitignore.to_string()),
        ("cache_dir", policy.cache_dir.clone()),
        ("strict", format!("[{}]", policy.strict.join(", "))),
        ("on_hit", on_hit),
        ("probe_shell", format!("[{probe_shell}]")),
    ] {
        out.push_str(&format!(
            "  {key}: {value}{}\n",
            default_marker(key, &policy.declared)
        ));
    }
    out
}

fn render_text(dump: &Dump) -> String {
    let mut out = String::from("sources:\n");
    for (index, source) in dump.sources.iter().enumerate() {
        out.push_str(&format!("  {}  {source}\n", index + 1));
    }

    out.push_str(&render_policy(&dump.policy));

    if !dump.scopes.is_empty() {
        out.push_str("\nscopes:\n");
        for scope in &dump.scopes {
            out.push_str(&format!("  {}:  # {}\n", scope.name, scope.source));
            out.push_str(&format!("    globs: [{}]\n", scope.globs.join(", ")));
            if let Some(gitignore) = scope.gitignore {
                out.push_str(&format!("    gitignore: {gitignore}\n"));
            }
        }
    }

    render_probes(&dump.probes, &mut out);

    if !dump.commands.is_empty() {
        out.push_str("\ncommands:\n");
        for command in &dump.commands {
            out.push_str(&format!("  {}:  # {}\n", command.name, command.source));
            out.push_str(&format!("    match: {}\n", command.match_mode));
            if !command.inputs.is_empty() {
                out.push_str(&format!("    inputs: [{}]\n", command.inputs.join(", ")));
            }
            if !command.outputs.is_empty() {
                out.push_str(&format!("    outputs: [{}]\n", command.outputs.join(", ")));
            }
            if !command.tags.is_empty() {
                out.push_str(&format!("    tags: [{}]\n", command.tags.join(", ")));
            }
            if let Some(on_hit) = &command.on_hit {
                out.push_str(&format!("    on_hit: {on_hit}\n"));
            }
        }
    }

    out
}

/// The `probes:` block of the human form, split out of [`render_text`] so
/// neither function outgrows the line cap as a probe's key set grows.
///
/// Every key a probe declared is echoed, `capture:` included: `--dump-config`
/// is what a reader audits a composed manifest with, so a key it does not print
/// is a key nobody can check without opening the fragment that set it.
fn render_probes(probes: &[ProbeEntry], out: &mut String) {
    if probes.is_empty() {
        return;
    }
    out.push_str("\nprobes:\n");
    for probe in probes {
        out.push_str(&format!("  {}:  # {}\n", probe.name, probe.source));
        if let Some(run) = &probe.run {
            out.push_str(&format!("    run: {run}\n"));
        }
        if let Some(file) = &probe.file {
            out.push_str(&format!("    file: {file}\n"));
        }
        if let Some(json) = &probe.json {
            out.push_str(&format!("    json: {json}\n"));
        }
        if let Some(ast) = &probe.ast {
            out.push_str(&format!("    ast: {ast}\n"));
        }
        if let Some(capture) = &probe.capture {
            out.push_str(&format!("    capture: [{}]\n", capture.join(", ")));
        }
        if let Some(lang) = &probe.lang {
            out.push_str(&format!("    lang: {lang}\n"));
        }
        out.push_str(&format!("    allow_empty: {}\n", probe.allow_empty));
    }
}

/// `"  (default)"` when `key` is not in `declared` (the root manifest never
/// wrote it), else the empty string — the human form's only nod to whether a
/// policy value was written or assumed; see [`Policy::declared`].
fn default_marker(key: &str, declared: &BTreeSet<&'static str>) -> &'static str {
    if declared.contains(key) {
        ""
    } else {
        "  (default)"
    }
}

#[cfg(test)]
#[path = "dump_tests.rs"]
mod tests;
