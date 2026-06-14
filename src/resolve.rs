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

/// Expands `patterns` (interpreted relative to `base`) into a sorted,
/// de-duplicated list of input paths relative to `base`.
///
/// Glob patterns may match zero files. A literal path absent on disk is skipped
/// with a warning (a removed input then shifts the digest rather than erroring).
/// When `gitignore` is true, glob matches ignored by git are dropped; explicitly
/// listed literals are always kept. The `.git` directory is never traversed and
/// symlinks are not followed. Paths use forward slashes.
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
        if !entry.file_type().is_some_and(|kind| kind.is_file()) {
            continue;
        }
        let path = entry.path();
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

    use super::{expand, is_glob};

    fn touch(dir: &Path, name: &str) {
        std::fs::write(dir.join(name), b"x").expect("write");
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
    fn missing_literal_is_skipped() {
        let dir = tempfile::tempdir().expect("tempdir");
        let gone = expand(&["absent.txt".to_owned()], dir.path(), true).expect("resolve");
        assert!(gone.is_empty(), "missing literal is not an error");
    }
}
