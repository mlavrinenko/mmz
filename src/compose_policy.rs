//! The four root-manifest-only keys (`cache_dir`, `gitignore`, `strict`,
//! `on_hit`): the single list naming them, the check that rejects them
//! outside the root, and the query `mmz --dump-config` uses to say whether
//! one was written or left to its default. Split out of `compose.rs` once
//! that file reached its own line cap — a cohesive seam, since every item
//! here is about the same four names, not a slice taken for size alone.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use crate::error::{Error, Result};
use crate::provenance::Provenance;

use super::{Document, POLICY_KEYS, parse_text};

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
    let [cache_dir, gitignore, strict, on_hit] = POLICY_KEYS;
    let present = [
        (cache_dir, document.cache_dir.is_some()),
        (gitignore, document.gitignore.is_some()),
        (strict, document.strict.is_some()),
        (on_hit, document.on_hit.is_some()),
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
    let [cache_dir, gitignore, strict, on_hit] = POLICY_KEYS;
    Ok([
        (cache_dir, document.cache_dir.is_some()),
        (gitignore, document.gitignore.is_some()),
        (strict, document.strict.is_some()),
        (on_hit, document.on_hit.is_some()),
    ]
    .into_iter()
    .filter(|(_, set)| *set)
    .map(|(key, _)| key)
    .collect())
}
