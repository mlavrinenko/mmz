//! `mmz --is-fresh`: assert a rule's cache is fresh without running it.
//!
//! The memoization engine ([`crate::engine`]) is run-or-skip: a stale rule runs.
//! A gate wants the opposite — to confirm a command was already memoized fresh
//! and otherwise fail, never launching the (often slow) command itself. That is
//! this module: resolve a rule's inputs, compare their digest to the record, and
//! report the verdict. It executes nothing.
//!
//! The reach-for case is a git hook. A pre-push that must not boot a VM, yet must
//! refuse a push whose checks were never run, calls `mmz --is-fresh -- just
//! check` and trusts the exit code. With no command it gates every rule at once.

use std::collections::BTreeMap;
use std::path::Path;

use crate::error::{Error, Result};
use crate::manifest::{Command, Manifest};
use crate::parametric;
use crate::status::{self, State, expansion_state};

/// Every kept rule's parametric matches, plus its pre-resolved shared
/// `inputs` file set keyed by rule name — [`expand_matching`]'s result.
type Expanded<'a> = (Vec<parametric::Match<'a>>, BTreeMap<String, Vec<String>>);

/// One expansion's freshness, the unit a gate reports on. For a parametric
/// rule this is one per-file expansion; for a static rule, the rule itself.
pub struct Verdict {
    /// The expansion's cache identity: the bare rule name for a static rule,
    /// or the rule name with its `{scope}` macro substituted for a
    /// parametric expansion.
    pub rule: String,
    state: State,
}

impl Verdict {
    /// True when the rule is fresh — its inputs are unchanged since it last
    /// succeeded, so a gate over it passes.
    #[must_use]
    pub const fn is_fresh(&self) -> bool {
        self.state.is_fresh()
    }

    /// The rule's freshness label (`fresh`, `stale`, `never`, `failed`,
    /// `no-inputs`), matching `mmz --status`.
    #[must_use]
    pub const fn state(&self) -> &'static str {
        self.state.label()
    }

    /// Why the rule is not fresh, for a gate's message; `None` when it is fresh.
    #[must_use]
    pub const fn reason(&self) -> Option<&'static str> {
        self.state.reason()
    }
}

/// Evaluates freshness against the nearest manifest above `cwd`, running
/// nothing. A parametric rule (a `{scope}`-fanned `name`) expands exactly as
/// `mmz --status` does: one [`Verdict`] per file its scope resolves to,
/// keyed on that expansion's concrete identity rather than the rule's literal
/// template name.
///
/// With `argv` given and `tags` empty, resolves the single expansion `argv`
/// binds to (via [`parametric::resolve_matches`]) and returns its lone
/// verdict — for a parametric rule this gates the one per-file expansion
/// `argv` names, not the whole rule. With `tags` non-empty, expands every
/// rule that carries every listed tag (an AND filter — a rule with no tags
/// never matches) and returns one verdict per expansion; `argv` must be
/// `None` in that case, since a targeted command already resolves to a
/// single expansion. With both empty, expands every rule in manifest order
/// and returns one verdict per expansion, so a caller can gate the whole
/// manifest at once.
///
/// # Errors
///
/// Returns [`Error::TagWithCommand`] when `tags` is non-empty and `argv` is
/// also given, [`Error::NoManifest`] when no manifest is found, a manifest
/// error when one cannot be loaded, [`Error::NoMatch`] when `argv` matches no
/// rule, [`Error::CollidingIdentity`] when two expansions share a cache
/// identity, or a resolution/hashing error when a rule's inputs cannot be
/// read.
pub fn evaluate(cwd: &Path, argv: Option<&[String]>, tags: &[String]) -> Result<Vec<Verdict>> {
    let located = Manifest::locate(cwd)?;
    let manifest = &located.manifest;
    let base = located.root.as_path();
    let cache_dir = base.join(&manifest.cache_dir);

    if !tags.is_empty() {
        if argv.is_some() {
            return Err(Error::TagWithCommand);
        }
        let (matches, shared) = expand_matching(manifest, base, |rule| {
            tags.iter().all(|tag| rule.tags.contains(tag))
        })?;
        return verdicts_for(&matches, &shared, base, &cache_dir);
    }

    if let Some(argv) = argv {
        let matches = parametric::resolve_matches(manifest, base, argv)?;
        parametric::detect_collision(&matches)?;
        let hit = matches.first().ok_or_else(|| Error::NoMatch {
            command: argv.join(" "),
        })?;
        let shared = status::shared_inputs(manifest, hit.rule, base)?;
        return Ok(vec![verdict_for(hit, &shared, base, &cache_dir)?]);
    }

    let (matches, shared) = expand_matching(manifest, base, |_| true)?;
    verdicts_for(&matches, &shared, base, &cache_dir)
}

/// Expands every rule passing `keep` into its parametric matches (one per
/// domain file, or the rule itself when static), then checks the collected
/// expansions for a colliding identity — the untargeted and tag-filtered
/// gates share this shape, differing only in which rules they keep. Also
/// resolves each kept rule's shared `inputs` once, keyed by rule name, so
/// every expansion a rule fans into reuses the same resolved file set
/// instead of re-walking the tree per expansion.
fn expand_matching<'a>(
    manifest: &'a Manifest,
    base: &Path,
    keep: impl Fn(&Command) -> bool,
) -> Result<Expanded<'a>> {
    let mut matches = Vec::new();
    let mut shared = BTreeMap::new();
    for rule in manifest.commands.iter().filter(|rule| keep(rule)) {
        shared.insert(
            rule.name.clone(),
            status::shared_inputs(manifest, rule, base)?,
        );
        matches.extend(parametric::expand_rule(manifest, base, rule)?);
    }
    parametric::detect_collision(&matches)?;
    Ok((matches, shared))
}

/// Builds one [`Verdict`] per expansion in `matches`, looking each
/// expansion's rule up in `shared` for its pre-resolved shared inputs.
fn verdicts_for(
    matches: &[parametric::Match],
    shared: &BTreeMap<String, Vec<String>>,
    base: &Path,
    cache_dir: &Path,
) -> Result<Vec<Verdict>> {
    matches
        .iter()
        .map(|hit| {
            let files = shared
                .get(hit.rule.name.as_str())
                .expect("shared inputs resolved for every kept rule");
            verdict_for(hit, files, base, cache_dir)
        })
        .collect()
}

/// Builds one expansion's [`Verdict`] — the step every branch of [`evaluate`]
/// needs once it has settled on which expansion(s) to report and resolved
/// that expansion's rule's shared inputs.
fn verdict_for(
    hit: &parametric::Match,
    shared: &[String],
    base: &Path,
    cache_dir: &Path,
) -> Result<Verdict> {
    Ok(Verdict {
        rule: hit.exp.identity.clone(),
        state: expansion_state(hit, shared, base, cache_dir)?,
    })
}

#[cfg(test)]
#[path = "freshness_tests.rs"]
mod tests;
