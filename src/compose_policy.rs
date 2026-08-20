//! The five root-manifest-only keys (`cache_dir`, `gitignore`, `strict`,
//! `on_hit`, `probe_shell`): the single list naming them, the check that
//! rejects them outside the root, and the query `mmz --dump-config` uses to
//! say whether one was written or left to its default. Split out of `compose.rs` once
//! that file reached its own line cap — a cohesive seam, since every item
//! here is about the same five names, not a slice taken for size alone.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use crate::error::{Error, Result};
use crate::provenance::Provenance;

use crate::manifest::{StrictPolicy, default_cache_dir, default_gitignore, default_probe_shell};

use super::{Document, POLICY_KEYS, parse_text};

/// The five policy values a root manifest resolves to, handed back to
/// [`super::load`] in one piece so the merge does not carry five separate
/// bindings through a function already at its line cap.
pub(super) struct Resolved {
    pub gitignore: bool,
    pub cache_dir: String,
    pub strict: StrictPolicy,
    pub on_hit: Option<String>,
    pub probe_shell: Vec<String>,
}

/// Resolves every root-only policy key on `document`, applying each default
/// and rejecting an explicit `null` (see [`require_or_default`]).
///
/// # Errors
///
/// Returns [`Error::NullPolicyKey`] naming the first key written as an
/// explicit `null`, and `path` rendered against `root` as [`super::load`]
/// renders one.
pub(super) fn resolve(document: &Document, path: &Path, root: &Path) -> Result<Resolved> {
    Ok(Resolved {
        gitignore: require_or_default(
            document.gitignore,
            "gitignore",
            path,
            root,
            default_gitignore,
        )?,
        cache_dir: require_or_default(
            document.cache_dir.clone(),
            "cache_dir",
            path,
            root,
            default_cache_dir,
        )?,
        strict: require_or_default(
            document.strict.clone(),
            "strict",
            path,
            root,
            StrictPolicy::all,
        )?,
        // Unlike the other four, an explicit `on_hit: null` in the root has
        // always been legal — `Manifest::on_hit` is `Option<String>` — so it
        // collapses to `None` exactly like an absent key, not an error.
        on_hit: document.on_hit.clone().flatten(),
        probe_shell: require_or_default(
            document.probe_shell.clone(),
            "probe_shell",
            path,
            root,
            default_probe_shell,
        )?,
    })
}

/// Resolves one root-only policy field's double-`Option` into the value
/// [`crate::manifest::Manifest`] wants: an absent key (`None`) uses `default`, a present value
/// (`Some(Some(value))`) is used as written, and a present-but-explicit
/// `null` (`Some(None)`) is [`Error::NullPolicyKey`] rather than being allowed
/// to fall through to `default` silently. Before composition existed these
/// fields were plain, non-nullable types, so `null` was already a hard parse
/// error; the shared per-file [`Document`] has to accept `null` so a
/// *fragment* setting one is still caught by [`Error::FragmentPolicyKey`],
/// which means the root's own explicit `null` has to be checked here instead
/// — an author who wrote `gitignore:` meaning `false` must not silently
/// resolve to `true` with no diagnostic.
///
/// # Errors
///
/// Returns [`Error::NullPolicyKey`] naming `key` and `path` (rendered against
/// `root`, see [`load`]) when `value` is `Some(None)`.
fn require_or_default<T>(
    value: Option<Option<T>>,
    key: &str,
    path: &Path,
    root: &Path,
    default: impl FnOnce() -> T,
) -> Result<T> {
    match value {
        None => Ok(default()),
        Some(None) => Err(Error::NullPolicyKey {
            key: key.to_owned(),
            path: Provenance::shorten(path, root),
        }),
        Some(Some(resolved)) => Ok(resolved),
    }
}

/// Rejects a fragment that sets a root-only policy key, naming the first one
/// found and the fragment that set it. `Option::is_some` on the outer
/// double-`Option` is exactly "the key is present" regardless of its inner
/// value, so a fragment that writes `gitignore:` (explicit `null`) is caught
/// here exactly as one that writes `gitignore: true` is — presence is what
/// this rule is about, not the value.
///
/// # Errors
///
/// Returns [`Error::FragmentPolicyKey`] if any of [`POLICY_KEYS`] is present,
/// `null` included, naming the fragment as [`super::load`] renders a path:
/// relative to `root` under it, absolute otherwise.
///
/// `pub(super)`, not `pub(crate)`: only `compose::visit_fragment` calls this,
/// and keeping it scoped to the parent module is what lets `Document` — a
/// type private to `compose` — appear in the signature without a
/// private-interface leak.
pub(super) fn check_no_policy_keys(document: &Document, path: &Path, root: &Path) -> Result<()> {
    let [cache_dir, gitignore, strict, on_hit, probe_shell] = POLICY_KEYS;
    let present = [
        (cache_dir, document.cache_dir.is_some()),
        (gitignore, document.gitignore.is_some()),
        (strict, document.strict.is_some()),
        (on_hit, document.on_hit.is_some()),
        (probe_shell, document.probe_shell.is_some()),
    ];
    if let Some((key, _)) = present.into_iter().find(|(_, set)| *set) {
        return Err(Error::FragmentPolicyKey {
            key: key.to_owned(),
            path: Provenance::shorten(path, root),
        });
    }
    Ok(())
}

/// Which of [`POLICY_KEYS`] the root manifest at `path` sets explicitly
/// (`root` only names the file in the parse error, as in [`super::load`]),
/// rather than leaving to its default. [`super::load`] resolves each key to
/// its effective value but does not keep whether it was written or
/// assumed — that distinction is only interesting to `mmz --dump-config`'s
/// human form, which marks a defaulted value so a reader is not left
/// guessing whether a quiet `strict: [no_match, no_inputs]` is written or
/// the default. Re-reads and re-parses `path` rather than threading the
/// answer out of `load` itself: the merge's own return type stays about the
/// resolved model, and a second small read of one file (never a fragment,
/// never recursive) is cheap next to a debugging action that is not on any
/// hot path.
///
/// # Errors
///
/// Returns [`Error::Io`] if `path` cannot be read, or [`Error::ManifestParse`]
/// if it cannot be parsed — in practice unreachable by the time a caller
/// gets here, since [`super::load`] already parsed the same file
/// successfully, but the signature stays honest about doing its own read
/// rather than trusting that.
pub(crate) fn declared_policy_keys(path: &Path, root: &Path) -> Result<BTreeSet<&'static str>> {
    let text = fs::read_to_string(path)?;
    let document = parse_text(&text, path, root)?;
    let [cache_dir, gitignore, strict, on_hit, probe_shell] = POLICY_KEYS;
    Ok([
        (cache_dir, document.cache_dir.is_some()),
        (gitignore, document.gitignore.is_some()),
        (strict, document.strict.is_some()),
        (on_hit, document.on_hit.is_some()),
        (probe_shell, document.probe_shell.is_some()),
    ]
    .into_iter()
    .filter(|(_, set)| *set)
    .map(|(key, _)| key)
    .collect())
}
