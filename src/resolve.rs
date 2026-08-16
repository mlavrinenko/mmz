//! Expansion of scope patterns into concrete input paths.

use std::path::Path;

use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use ignore::{DirEntry, Walk, WalkBuilder};

use crate::error::{Error, Result};

/// Returns true if `pattern` contains glob metacharacters.
#[must_use]
pub fn is_glob(pattern: &str) -> bool {
    pattern.chars().any(|ch| matches!(ch, '*' | '?' | '['))
}

/// A set of patterns and the ignore setting they expand under.
///
/// A scope can override the manifest's `gitignore`, so one rule's patterns no
/// longer share a single filter; callers hand [`expand_groups`] the patterns
/// already bucketed by effective setting.
#[derive(Debug, Clone)]
pub struct GlobGroup {
    /// Patterns to expand — globs and literal paths, as in [`expand`].
    pub globs: Vec<String>,
    /// Whether git-ignored paths are skipped while expanding these patterns.
    pub gitignore: bool,
}

/// Expands every group against `base` and unions the results into one sorted,
/// de-duplicated list of paths relative to `base`.
///
/// Each group carries its own ignore setting, so a rule mixing a filtered scope
/// with an artifact scope resolves both in a single call. The gitignore chain
/// prunes directories *during* traversal — an ignored tree is never descended
/// into — so it cannot be re-applied per pattern after one shared walk; this
/// runs a walk per group instead, which [`crate::manifest::Manifest::glob_groups`]
/// caps at two by bucketing scopes rather than emitting one group each. A
/// single group costs exactly what [`expand`] always did.
///
/// # Errors
///
/// Returns [`Error::Pattern`] if a glob pattern is invalid.
pub fn expand_groups(groups: &[GlobGroup], base: &Path) -> Result<Vec<String>> {
    if let [only] = groups {
        return expand(&only.globs, base, only.gitignore);
    }
    let mut out: Vec<String> = Vec::new();
    for group in groups {
        out.extend(expand(&group.globs, base, group.gitignore)?);
    }
    out.sort();
    out.dedup();
    Ok(out)
}

/// Expands `patterns` (interpreted relative to `base`) into a sorted,
/// de-duplicated list of input paths relative to `base`.
///
/// Glob patterns may match zero files. A literal path absent on disk is skipped
/// with a warning (a removed input then shifts the digest rather than erroring).
/// When `gitignore` is true, glob matches ignored by git are dropped; explicitly
/// listed literals are always kept. The `.git` directory is never traversed.
/// Symlinked directories are never traversed, but a symlink that resolves to a
/// regular file is treated as a file (matching literal-path resolution), so a
/// symlinked source is not silently dropped from a glob's input set. Paths use
/// forward slashes.
///
/// # Errors
///
/// Returns [`Error::Pattern`] if a glob pattern is invalid.
pub fn expand(patterns: &[String], base: &Path, gitignore: bool) -> Result<Vec<String>> {
    let mut out: Vec<String> = Vec::new();
    let mut globs: Vec<&str> = Vec::new();
    for pattern in patterns {
        if is_glob(pattern) {
            globs.push(pattern);
        } else {
            expand_literal(pattern, base, &mut out);
        }
    }
    if !globs.is_empty() {
        expand_globs(&globs, base, gitignore, &mut out)?;
    }
    out.sort();
    out.dedup();
    Ok(out)
}

fn expand_literal(pattern: &str, base: &Path, out: &mut Vec<String>) {
    if base.join(pattern).is_file() {
        out.push(normalize(pattern));
    } else {
        log::warn!("literal path `{pattern}` is missing; treating as removed");
    }
}

/// Compiles `globs` into a matcher, then walks `base` once (honouring the
/// gitignore chain when `gitignore` is set) and collects every file that
/// matches at least one pattern. Patterns matching nothing are warned about.
fn expand_globs(globs: &[&str], base: &Path, gitignore: bool, out: &mut Vec<String>) -> Result<()> {
    let set = build_globset(globs)?;
    let mut matched = vec![false; globs.len()];
    for entry in build_walker(base, gitignore) {
        let Ok(entry) = entry else { continue };
        let path = entry.path();
        // `follow_links(false)` keeps the walker from descending into a
        // symlinked directory (its own file_type stays "symlink", not
        // "file"), but a symlink that resolves to a regular file is still
        // accepted here so it matches `expand_literal`'s
        // `base.join(pattern).is_file()`, which follows the link.
        let is_regular_file = entry.file_type().is_some_and(|kind| kind.is_file());
        let is_symlinked_file = !is_regular_file
            && entry.file_type().is_some_and(|kind| kind.is_symlink())
            && path.is_file();
        if !is_regular_file && !is_symlinked_file {
            continue;
        }
        let rel = path.strip_prefix(base).unwrap_or(path);
        let candidate = normalize(&rel.to_string_lossy());
        let hits = set.matches(&candidate);
        if hits.is_empty() {
            continue;
        }
        for index in hits {
            if let Some(flag) = matched.get_mut(index) {
                *flag = true;
            }
        }
        out.push(candidate);
    }
    for (index, pattern) in globs.iter().enumerate() {
        if matched.get(index) == Some(&false) {
            log::warn!("pattern `{pattern}` matched no files");
        }
    }
    Ok(())
}

/// Builds a [`GlobSet`] from `globs`, treating `/` as a literal separator so
/// `*` does not cross directories and `**` does.
fn build_globset(globs: &[&str]) -> Result<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    for pattern in globs {
        let glob = GlobBuilder::new(pattern)
            .literal_separator(true)
            .build()
            .map_err(|source| Error::Pattern {
                pattern: (*pattern).to_owned(),
                source,
            })?;
        builder.add(glob);
    }
    builder.build().map_err(|source| Error::Pattern {
        pattern: globs.join(", "),
        source,
    })
}

/// Builds a file walker rooted at `base`. Hidden files are included (only
/// ignore rules filter), the `.git` directory is always pruned, and when
/// `gitignore` is set the full gitignore chain applies — `.gitignore` is also
/// added as a custom ignore file so it is honoured outside a git repository.
fn build_walker(base: &Path, gitignore: bool) -> Walk {
    let mut builder = WalkBuilder::new(base);
    builder
        .hidden(false)
        .follow_links(false)
        .ignore(false)
        .git_ignore(gitignore)
        .git_global(gitignore)
        .git_exclude(gitignore)
        .parents(gitignore)
        .filter_entry(skip_git_dir);
    if gitignore {
        builder.add_custom_ignore_filename(".gitignore");
    }
    builder.build()
}

fn skip_git_dir(entry: &DirEntry) -> bool {
    entry.file_name() != ".git"
}

fn normalize(path: &str) -> String {
    path.replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{GlobGroup, expand, expand_groups, is_glob};

    fn touch(dir: &Path, name: &str) {
        std::fs::write(dir.join(name), b"x").expect("write");
    }

    fn group(pattern: &str, gitignore: bool) -> GlobGroup {
        GlobGroup {
            globs: vec![pattern.to_owned()],
            gitignore,
        }
    }

    /// A tree with `/target` git-ignored: one tracked source, one ignored
    /// build artifact, and one ignored source next to it.
    fn artifact_tree() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join(".gitignore"), "/target\n").expect("gitignore");
        std::fs::create_dir(dir.path().join("target")).expect("mkdir");
        touch(dir.path(), "kept.rs");
        std::fs::write(dir.path().join("target/lcov.info"), b"x").expect("write");
        std::fs::write(dir.path().join("target/built.rs"), b"x").expect("write");
        dir
    }

    #[test]
    fn detects_glob_patterns() {
        assert!(is_glob("src/*.rs"));
        assert!(!is_glob("plain/path.rs"));
    }

    #[test]
    fn globs_sort_dedup_and_respect_depth() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir(dir.path().join("sub")).expect("mkdir");
        touch(dir.path(), "top.rs");
        std::fs::write(dir.path().join("sub/nested.rs"), b"x").expect("write");

        let shallow = expand(&["*.rs".to_owned()], dir.path(), true).expect("resolve");
        assert_eq!(shallow, vec!["top.rs".to_owned()], "single star stays flat");

        let deep = expand(&["**/*.rs".to_owned()], dir.path(), true).expect("resolve");
        assert_eq!(deep, vec!["sub/nested.rs".to_owned(), "top.rs".to_owned()]);
    }

    #[test]
    fn gitignore_filters_globs_but_not_literals() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join(".gitignore"), "/target\n").expect("gitignore");
        std::fs::create_dir(dir.path().join("target")).expect("mkdir");
        touch(dir.path(), "kept.rs");
        std::fs::write(dir.path().join("target/built.rs"), b"x").expect("write");

        let on = expand(&["**/*.rs".to_owned()], dir.path(), true).expect("resolve");
        assert_eq!(on, vec!["kept.rs".to_owned()], "ignored path dropped");

        let literal = expand(&["target/built.rs".to_owned()], dir.path(), true).expect("resolve");
        assert_eq!(
            literal,
            vec!["target/built.rs".to_owned()],
            "literal overrides ignore"
        );
    }

    #[test]
    fn a_group_that_opted_out_sees_an_ignored_file() {
        let dir = artifact_tree();
        let found = expand_groups(&[group("target/**", false)], dir.path()).expect("resolve");
        assert_eq!(
            found,
            vec!["target/built.rs".to_owned(), "target/lcov.info".to_owned()],
            "gitignore off, so the artifact tree is walked"
        );
    }

    #[test]
    fn a_sibling_group_keeps_filtering_in_the_same_call() {
        let dir = artifact_tree();
        let found = expand_groups(
            &[group("**/*.rs", true), group("target/*.info", false)],
            dir.path(),
        )
        .expect("resolve");
        assert_eq!(
            found,
            vec!["kept.rs".to_owned(), "target/lcov.info".to_owned()],
            "the artifact resolves while its neighbour `target/built.rs` stays filtered out"
        );
    }

    #[test]
    fn one_group_resolves_exactly_like_a_bare_expand() {
        let dir = artifact_tree();
        let patterns = ["**/*.rs".to_owned()];
        assert_eq!(
            expand_groups(&[group("**/*.rs", true)], dir.path()).expect("groups"),
            expand(&patterns, dir.path(), true).expect("bare"),
            "the array form's behaviour is unchanged"
        );
    }

    #[test]
    fn overlapping_groups_yield_each_path_once() {
        let dir = artifact_tree();
        let found = expand_groups(
            &[group("**/*.rs", true), group("**/*.rs", false)],
            dir.path(),
        )
        .expect("resolve");
        assert_eq!(
            found,
            vec!["kept.rs".to_owned(), "target/built.rs".to_owned()],
            "a path matched by both groups is not digested twice"
        );
    }

    #[test]
    fn missing_literal_is_skipped() {
        let dir = tempfile::tempdir().expect("tempdir");
        let gone = expand(&["absent.txt".to_owned()], dir.path(), true).expect("resolve");
        assert!(gone.is_empty(), "missing literal is not an error");
    }

    #[test]
    #[cfg(unix)]
    fn glob_includes_symlinked_file_like_literal_does() {
        use std::os::unix::fs::symlink;

        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir(dir.path().join("real")).expect("mkdir");
        touch(&dir.path().join("real"), "target.rs");
        symlink(
            dir.path().join("real/target.rs"),
            dir.path().join("link.rs"),
        )
        .expect("symlink");

        let via_glob = expand(&["*.rs".to_owned()], dir.path(), true).expect("resolve glob");
        let via_literal =
            expand(&["link.rs".to_owned()], dir.path(), true).expect("resolve literal");

        assert!(
            via_glob.contains(&"link.rs".to_owned()),
            "glob should include a symlink that resolves to a file, same as a literal path: {via_glob:?}"
        );
        assert_eq!(
            via_glob
                .iter()
                .filter(|path| *path == "link.rs")
                .collect::<Vec<_>>(),
            via_literal.iter().collect::<Vec<_>>(),
            "glob and literal resolution must agree on a symlinked file"
        );
    }

    #[test]
    #[cfg(unix)]
    fn glob_does_not_traverse_symlinked_directory() {
        use std::os::unix::fs::symlink;

        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir(dir.path().join("real_dir")).expect("mkdir");
        touch(&dir.path().join("real_dir"), "inside.rs");
        symlink(dir.path().join("real_dir"), dir.path().join("linked_dir")).expect("symlink");
        touch(dir.path(), "top.rs");

        let matches = expand(&["**/*.rs".to_owned()], dir.path(), true).expect("resolve");

        assert!(
            !matches.iter().any(|path| path.starts_with("linked_dir/")),
            "walker must not descend into a symlinked directory: {matches:?}"
        );
        assert_eq!(
            matches,
            vec!["real_dir/inside.rs".to_owned(), "top.rs".to_owned()],
            "only the real tree and top-level file are found, not the symlinked dir's contents a second time"
        );
    }
}
