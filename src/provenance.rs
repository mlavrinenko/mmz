//! Which file contributed each scope, probe and command in a composed
//! manifest.
//!
//! [`crate::compose::load`] builds a [`Provenance`] alongside the merged
//! [`crate::manifest::Manifest`], and it rides on
//! [`crate::manifest::Located`] from there — the duplicate-key errors the
//! loader raises cannot name both source files without recording provenance
//! in the first place, and having paid for it, later tooling gets to read it
//! too: `--status` grows a `source` field per rule and `--dump-config` prints
//! the merged manifest with origins attached, both reading this type rather
//! than reaching back into the loader.
//!
//! The root manifest is recorded as the source of its own entries rather than
//! as a special case: a single-file project — no imports at all — is the
//! degenerate case of composition, not a different code path, so a lookup
//! never needs to ask "did this come from an import" before it can answer
//! "which file".

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Which file contributed each scope, probe and command in a merged manifest.
///
/// The root manifest is recorded as the source of its own entries — a
/// single-file project is the degenerate case, not a special case — so a
/// lookup never needs to special-case "no imports". Every path is
/// canonicalized; use [`Provenance::display`] to render one the way a user
/// should see it.
#[derive(Debug, Clone, Default)]
pub struct Provenance {
    /// Every file that contributed to the merge, in load order: the root
    /// manifest first, then each import depth-first — the same order
    /// `imports:` lists them in. A file reached twice by different routes (a
    /// diamond) appears once, at the point it was first visited, matching the
    /// "loads once" rule for diamonds themselves. `mmz --dump-config` numbers
    /// this list to show the import graph before the entries it fed.
    pub sources: Vec<PathBuf>,
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
        Self::shorten(path, root).display().to_string()
    }

    /// [`Provenance::display`]'s rule as a path rather than a string: `path`
    /// relative to `root` when it sits under it, unchanged otherwise.
    ///
    /// Reports render through `display`; [`crate::compose::load`] builds the
    /// paths its errors name through this, so a composition error names a
    /// file the way `--status` and `--dump-config` do rather than answering
    /// the same question with a second rule. Both readings only line up
    /// because every path either side holds is canonical —
    /// [`crate::manifest::Located::at`] canonicalizes the project root for
    /// exactly that reason.
    #[must_use]
    pub fn shorten(path: &Path, root: &Path) -> PathBuf {
        path.strip_prefix(root).unwrap_or(path).to_path_buf()
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::Provenance;

    #[test]
    fn display_is_root_relative_under_root_and_absolute_otherwise() {
        let root = Path::new("/tmp/project");
        let under = Path::new("/tmp/project/.mmz/conf.d/a.yaml");
        let outside = Path::new("/nix/store/xyz/rules.yaml");
        assert_eq!(Provenance::display(under, root), ".mmz/conf.d/a.yaml");
        assert_eq!(
            Provenance::display(outside, root),
            "/nix/store/xyz/rules.yaml"
        );
    }

    #[test]
    fn shorten_is_the_same_rule_as_display() {
        let root = Path::new("/tmp/project");
        for path in [
            Path::new("/tmp/project/.mmz/conf.d/a.yaml"),
            Path::new("/nix/store/xyz/rules.yaml"),
        ] {
            assert_eq!(
                Provenance::shorten(path, root).display().to_string(),
                Provenance::display(path, root),
                "the loader's error paths and a report's rows answer alike",
            );
        }
    }
}
