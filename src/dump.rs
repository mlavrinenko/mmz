//! `mmz --dump-config`: print the merged manifest with the source file of
//! every scope, probe and command.
//!
//! Once a manifest can be assembled from several files (see
//! [`crate::compose`]), the merged model itself hides the import graph that
//! produced it — a scope, probe or command reads the same whether it came
//! from the root manifest or the third fragment three imports deep. This
//! module answers two questions composition raises and `--status` does not:
//! a person asking "which file made this rule skip?" needs more than the
//! rule `--status` already names (see
//! `mmz-report-each-rule-s-source-file-in-status`) when the surprise is in a
//! scope's globs or a probe's command line, not a rule's freshness; and a
//! generator emitting a fragment wants a gate hook to assert the fragment it
//! wrote is the one actually in effect, not merely present on disk.
//!
//! Two renderings share one model, exactly as [`crate::status`] does: a human
//! form (`mmz --dump-config`, rendered by [`render_text`]) that leads with
//! the source list in load order — so the import graph is visible before the
//! entries it fed are — then each section's entries annotated with the file
//! they came from; and a machine form (`mmz --dump-config=json`) carrying the
//! same facts under stable keys, because a gate hook asserting against this
//! output is half the point. There is deliberately no `=json-schema` arm and
//! no `schema/config-dump.schema.json`: the only consumer today is a gate
//! that can assert on keys directly, and a schema for a document with one
//! consumer is premature (see the task's `Deferred` section).
//!
//! Both renderings print the merged manifest *after* validation. A manifest
//! that fails to load or merge exits 4 with the same error every other reader
//! produces (see [`crate::manifest::Manifest::locate`]); this module is not a
//! debugging aid for that failure, and prints no partial dump — [`collect`]
//! either returns a complete model or an error, never a partly-built one, so
//! there is nothing to accidentally emit on the failure path.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::error::{Error, Result};
use crate::manifest::{Manifest, MatchMode};
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
    scopes: Vec<ScopeEntry>,
    probes: Vec<ProbeEntry>,
    commands: Vec<CommandEntry>,
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

/// One merged probe: its `run` line and `allow_empty` flag, plus the file
/// that declared it.
#[derive(Serialize)]
struct ProbeEntry {
    name: String,
    run: String,
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
        scopes,
        probes,
        commands,
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
/// load order, then each non-empty section's entries, each annotated with a
/// trailing `# <source>` comment on its header line — the same shape a merged
/// `config.yaml` would have if one file held everything, so the format needs
/// no legend of its own.
///
/// A section with no entries (e.g. a manifest with no probes) is omitted
/// entirely rather than printed with an empty body, matching how `--status`'s
/// own conditional columns only appear when there is something to show.
fn render_text(dump: &Dump) -> String {
    let mut out = String::from("sources:\n");
    for (index, source) in dump.sources.iter().enumerate() {
        out.push_str(&format!("  {}  {source}\n", index + 1));
    }

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

    if !dump.probes.is_empty() {
        out.push_str("\nprobes:\n");
        for probe in &dump.probes {
            out.push_str(&format!("  {}:  # {}\n", probe.name, probe.source));
            out.push_str(&format!("    run: {}\n", probe.run));
            out.push_str(&format!("    allow_empty: {}\n", probe.allow_empty));
        }
    }

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

#[cfg(test)]
#[path = "dump_tests.rs"]
mod tests;
