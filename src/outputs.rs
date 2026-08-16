//! Declared outputs: the literal artifact paths a rule's run is expected to
//! produce.
//!
//! A cache record is a claim — this command exited 0 while its inputs hashed to
//! H. For a verdict command (`fmt --check`, `clippy`) that claim holds for as
//! long as H holds. For a producer command the claim carries a side effect, and
//! the effect can be undone without touching a single input: `cargo clean`
//! deletes the artifact and leaves every source byte-identical, so the record
//! is not stale — it is void. Declared outputs are the second way a record can
//! stop being valid: a rule is fresh only when its inputs still hash the same
//! AND every declared output exists.
//!
//! Existence only. mmz never hashes an output: the input digest already proves
//! that an existing artifact is the one those inputs produced, so hashing would
//! buy tamper detection alone — a different feature with a different cost.
//!
//! Outputs are paths, not patterns. Each is stat-ed directly and never walked,
//! so neither the manifest-level `gitignore` filter nor a per-scope override
//! applies to them: an artifact under an ignored `target/` is found by a plain
//! `stat`, with no opt-out to declare.

use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::resolve;

/// Returns the first declared output missing from disk, in declaration order
/// and spelled as the operator wrote it; `None` when every one is present (or
/// none is declared).
///
/// Presence is a plain `stat` of `base.join(output)` — a directory artifact
/// counts as present, and a symlink that resolves to nothing does not. The
/// declared spelling comes back rather than the joined path so a message can
/// name what the manifest names.
#[must_use]
pub fn first_missing(base: &Path, outputs: &[PathBuf]) -> Option<String> {
    outputs
        .iter()
        .find(|output| !base.join(output).exists())
        .map(|output| output.display().to_string())
}

/// Checks that `outputs` are usable literal paths, for [`crate::Manifest`]'s
/// load-time validation.
///
/// # Errors
///
/// Returns [`Error::InvalidOutput`] for a blank path or one carrying a glob
/// metacharacter. A pattern here would never match anything, and an output that
/// never matches is precisely the silent forever-fresh failure this feature
/// exists to end — so it is refused where it is written, not discovered later.
pub fn validate(command: &str, outputs: &[PathBuf]) -> Result<()> {
    for output in outputs {
        let path = output.to_string_lossy();
        let reason = if path.trim().is_empty() {
            "an output path cannot be blank"
        } else if resolve::is_glob(&path) {
            "outputs are literal paths, not patterns; a glob would never match here"
        } else {
            continue;
        };
        return Err(Error::InvalidOutput {
            command: command.to_owned(),
            path: path.into_owned(),
            reason: reason.to_owned(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{first_missing, validate};

    fn paths(raw: &[&str]) -> Vec<PathBuf> {
        raw.iter().map(PathBuf::from).collect()
    }

    #[test]
    fn no_outputs_is_never_missing() {
        let dir = tempfile::tempdir().expect("tempdir");
        assert_eq!(first_missing(dir.path(), &[]), None, "nothing declared");
    }

    #[test]
    fn reports_the_first_missing_in_declaration_order() {
        let dir = tempfile::tempdir().expect("tempdir");
        let base = dir.path();
        std::fs::write(base.join("present.bin"), b"x").expect("write artifact");

        assert_eq!(
            first_missing(base, &paths(&["present.bin"])),
            None,
            "a present output is not missing"
        );
        assert_eq!(
            first_missing(base, &paths(&["present.bin", "gone.bin", "also-gone.bin"])),
            Some("gone.bin".to_owned()),
            "the first missing path is named, not the last"
        );

        std::fs::remove_file(base.join("present.bin")).expect("delete artifact");
        assert_eq!(
            first_missing(base, &paths(&["present.bin"])),
            Some("present.bin".to_owned()),
            "deleting the artifact makes it missing"
        );
    }

    #[test]
    fn a_directory_artifact_counts_as_present() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir(dir.path().join("doc")).expect("mkdir");
        assert_eq!(
            first_missing(dir.path(), &paths(&["doc"])),
            None,
            "a directory output exists, so it is not missing"
        );
    }

    #[test]
    fn nested_output_is_stat_ed_not_walked() {
        let dir = tempfile::tempdir().expect("tempdir");
        let base = dir.path();
        // The gitignore filter governs glob expansion; an output is stat-ed, so
        // an ignored path resolves exactly like any other.
        std::fs::write(base.join(".gitignore"), "/target\n").expect("write .gitignore");
        std::fs::create_dir(base.join("target")).expect("mkdir target");
        std::fs::write(base.join("target/lcov.info"), b"x").expect("write artifact");
        assert_eq!(
            first_missing(base, &paths(&["target/lcov.info"])),
            None,
            "a git-ignored artifact is found without any opt-out"
        );
    }

    #[test]
    fn literal_paths_validate() {
        validate("just cover", &paths(&["target/coverage/lcov.info"])).expect("literal path is ok");
        validate("just cover", &[]).expect("no outputs is ok");
    }

    #[test]
    fn a_glob_metacharacter_is_rejected_as_never_matching() {
        for pattern in ["target/*.info", "target/?.info", "target/[ab].info"] {
            let err = validate("just cover", &paths(&[pattern])).expect_err("glob rejected");
            let message = err.to_string();
            assert!(
                message.contains("literal paths, not patterns"),
                "the message says outputs are literal: {message}"
            );
            assert!(
                message.contains(pattern),
                "the message names the offending path: {message}"
            );
        }
    }

    #[test]
    fn a_blank_output_is_rejected() {
        let err = validate("just cover", &paths(&["   "])).expect_err("blank rejected");
        assert!(
            err.to_string().contains("cannot be blank"),
            "a blank path would stat the project root and read fresh forever"
        );
    }
}
