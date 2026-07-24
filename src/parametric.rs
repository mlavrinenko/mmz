//! Glob-fanned parametric rules.
//!
//! A command whose `name` carries a single `{scope}` macro stands for one
//! per-file cache identity per file that scope resolves to. `cargo mutants -f
//! {targets}` with `targets: ["src/**/*.rs"]` expands to `cargo mutants -f
//! src/a.rs`, `cargo mutants -f src/b.rs`, … — each a distinct record, keyed by
//! and scoped to its own file plus the rule's shared `inputs` pins. This is the
//! one-rule-per-file boilerplate the static form needs, without the hand-list.
//!
//! The macro occupies one whitespace token but may sit inside it
//! (`--file={scope}` binds `prefix="--file="`, `suffix=""`). At runtime an
//! invocation binds the macro to the argv token carrying that literal
//! prefix/suffix, and the bound value must be a member of the scope's resolved
//! file set — only declared files fan, so an off-list path falls through to the
//! normal no-match path rather than inventing a record.

use std::path::Path;

use crate::error::{Error, Result};
use crate::manifest::{Command, Manifest, MatchMode};
use crate::resolve;

/// A parsed `{scope}` macro found in a rule's `name`.
pub struct MacroRef {
    /// The scope whose files the rule fans over.
    pub scope: String,
    /// Index of the whitespace token holding the macro.
    token_index: usize,
    /// Literal text before `{scope}` within that token.
    prefix: String,
    /// Literal text after `{scope}` within that token.
    suffix: String,
}

/// One expansion of a rule: the concrete cache identity and, for a parametric
/// rule, the file that expansion is bound to (unioned into its inputs).
pub struct Expansion {
    /// The cache identity (the rule name with any macro substituted).
    pub identity: String,
    /// The bound file for a parametric expansion; `None` for a static rule.
    pub file: Option<String>,
}

/// A rule paired with one of its expansions, as matched or enumerated.
pub struct Match<'a> {
    /// The rule this expansion came from (for its `inputs`, `on_hit`, …).
    pub rule: &'a Command,
    /// The concrete identity and bound file.
    pub exp: Expansion,
}

/// Parses the `{scope}` macro from `name`, or `None` when the name is static.
///
/// # Errors
///
/// Returns [`Error::MacroSyntax`] when the name has an unmatched brace, an empty
/// `{}`, or more than one macro.
pub fn parse(name: &str) -> Result<Option<MacroRef>> {
    let mut found: Option<MacroRef> = None;
    for (index, token) in name.split_whitespace().enumerate() {
        let Some(open) = token.find('{') else {
            if token.contains('}') {
                return Err(syntax(name, "unmatched `}`"));
            }
            continue;
        };
        if found.is_some() {
            return Err(syntax(name, "more than one macro"));
        }
        let rest = &token[open + 1..];
        let close = rest
            .find('}')
            .ok_or_else(|| syntax(name, "unmatched `{`"))?;
        let scope = &rest[..close];
        let suffix = &rest[close + 1..];
        if scope.is_empty() {
            return Err(syntax(name, "empty `{}` macro"));
        }
        if scope.contains('{') || suffix.contains('{') || suffix.contains('}') {
            return Err(syntax(name, "more than one macro"));
        }
        found = Some(MacroRef {
            scope: scope.to_owned(),
            token_index: index,
            prefix: token[..open].to_owned(),
            suffix: suffix.to_owned(),
        });
    }
    Ok(found)
}

fn syntax(name: &str, reason: &str) -> Error {
    Error::MacroSyntax {
        name: name.to_owned(),
        reason: reason.to_owned(),
    }
}

/// Substitutes `file` for the `{scope}` macro in `name`, yielding the identity.
#[must_use]
pub fn expand_name(name: &str, scope: &str, file: &str) -> String {
    name.replacen(&format!("{{{scope}}}"), file, 1)
}

/// Binds a parametric rule against `argv`, returning the raw file token when the
/// literal skeleton matches, else `None`. Pure: membership is checked by the
/// caller against the resolved domain.
#[must_use]
pub fn bind(mac: &MacroRef, rule: &Command, argv: &[String]) -> Option<String> {
    let tokens: Vec<&str> = rule.name.split_whitespace().collect();
    let fits = match rule.match_mode {
        MatchMode::Prefix => argv.len() >= tokens.len(),
        MatchMode::Exact => argv.len() == tokens.len(),
    };
    if !fits {
        return None;
    }
    let mut file = None;
    for (index, token) in tokens.iter().enumerate() {
        let arg = argv.get(index)?;
        if index == mac.token_index {
            let bound = arg.strip_prefix(&mac.prefix)?.strip_suffix(&mac.suffix)?;
            if bound.is_empty() {
                return None;
            }
            file = Some(bound.to_owned());
        } else if arg != token {
            return None;
        }
    }
    file
}

/// Enumerates every expansion of `rule`: one per domain file for a parametric
/// rule, or just the rule itself for a static one.
///
/// # Errors
///
/// Returns [`Error::MacroSyntax`] for a malformed macro, [`Error::UnknownScope`]
/// when the macro's scope is undefined, or a resolution error.
pub fn expand_rule<'a>(
    manifest: &Manifest,
    base: &Path,
    rule: &'a Command,
) -> Result<Vec<Match<'a>>> {
    let Some(mac) = parse(&rule.name)? else {
        return Ok(vec![Match {
            rule,
            exp: Expansion {
                identity: rule.name.clone(),
                file: None,
            },
        }]);
    };
    let domain = resolve_domain(manifest, base, rule, &mac.scope)?;
    Ok(domain
        .into_iter()
        .map(|file| Match {
            rule,
            exp: Expansion {
                identity: expand_name(&rule.name, &mac.scope, &file),
                file: Some(file),
            },
        })
        .collect())
}

/// Returns every rule whose skeleton matches `argv`, each with its concrete
/// identity. A parametric rule matches only when the bound file is in its
/// domain, so off-list files never fan.
///
/// # Errors
///
/// Same as [`expand_rule`].
pub fn resolve_matches<'a>(
    manifest: &'a Manifest,
    base: &Path,
    argv: &[String],
) -> Result<Vec<Match<'a>>> {
    let mut out = Vec::new();
    for rule in &manifest.commands {
        if let Some(matched) = match_rule(manifest, base, rule, argv)? {
            out.push(matched);
        }
    }
    Ok(out)
}

/// Matches one rule against `argv`: a static token-prefix match, or a parametric
/// bind-then-membership check.
fn match_rule<'a>(
    manifest: &Manifest,
    base: &Path,
    rule: &'a Command,
    argv: &[String],
) -> Result<Option<Match<'a>>> {
    let Some(mac) = parse(&rule.name)? else {
        return Ok(crate::matcher::matches(rule, argv).then(|| Match {
            rule,
            exp: Expansion {
                identity: rule.name.clone(),
                file: None,
            },
        }));
    };
    let Some(raw) = bind(&mac, rule, argv) else {
        return Ok(None);
    };
    let norm = normalize(&raw, base);
    let domain = resolve_domain(manifest, base, rule, &mac.scope)?;
    if !domain.contains(&norm) {
        return Ok(None);
    }
    Ok(Some(Match {
        rule,
        exp: Expansion {
            identity: expand_name(&rule.name, &mac.scope, &norm),
            file: Some(norm),
        },
    }))
}

/// Errors when two matches resolve to the same identity — an ambiguous record
/// owner that mmz refuses to silently pick a winner for.
///
/// # Errors
///
/// Returns [`Error::CollidingIdentity`] on the first collision found.
pub fn detect_collision(matches: &[Match]) -> Result<()> {
    for (index, left) in matches.iter().enumerate() {
        for right in matches.iter().skip(index + 1) {
            if left.exp.identity == right.exp.identity {
                return Err(Error::CollidingIdentity {
                    identity: left.exp.identity.clone(),
                    rules: format!("`{}`, `{}`", left.rule.name, right.rule.name),
                });
            }
        }
    }
    Ok(())
}

/// Resolves a macro scope to its file set, honouring the gitignore filter.
fn resolve_domain(
    manifest: &Manifest,
    base: &Path,
    rule: &Command,
    scope: &str,
) -> Result<Vec<String>> {
    let globs = manifest
        .scopes
        .get(scope)
        .ok_or_else(|| Error::UnknownScope {
            command: rule.name.clone(),
            scope: scope.to_owned(),
        })?;
    resolve::expand(globs, base, manifest.gitignore)
}

/// Normalizes an argv file token to a `base`-relative, forward-slash path so it
/// compares against a resolved domain entry.
fn normalize(raw: &str, base: &Path) -> String {
    let path = Path::new(raw);
    let rel = path.strip_prefix(base).unwrap_or(path);
    let text = rel.to_string_lossy().replace('\\', "/");
    text.strip_prefix("./").unwrap_or(&text).to_owned()
}

#[cfg(test)]
mod tests {
    use super::{bind, expand_name, parse};
    use crate::manifest::{Command, MatchMode};

    fn rule(name: &str, mode: MatchMode) -> Command {
        Command {
            name: name.to_owned(),
            inputs: Vec::new(),
            match_mode: mode,
            tags: Vec::new(),
            on_hit: None,
        }
    }

    fn argv(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|part| (*part).to_owned()).collect()
    }

    #[test]
    fn parse_none_for_static_name() {
        assert!(parse("cargo test").expect("parse").is_none());
    }

    #[test]
    fn parse_extracts_scope_and_affixes() {
        let mac = parse("cargo mutants -f {targets}")
            .expect("parse")
            .expect("macro");
        assert_eq!(mac.scope, "targets");
        assert_eq!(mac.token_index, 3);
        assert_eq!(mac.prefix, "");
        assert_eq!(mac.suffix, "");

        let embedded = parse("ruff --stdin-filename={files} check")
            .expect("parse")
            .expect("macro");
        assert_eq!(embedded.scope, "files");
        assert_eq!(embedded.prefix, "--stdin-filename=");
        assert_eq!(embedded.suffix, "");
        assert_eq!(embedded.token_index, 1);
    }

    #[test]
    fn parse_rejects_malformed_macros() {
        assert!(parse("a {b} {c}").is_err(), "two macros");
        assert!(parse("a {b").is_err(), "unmatched open");
        assert!(parse("a b}").is_err(), "unmatched close");
        assert!(parse("a {}").is_err(), "empty macro");
    }

    #[test]
    fn expand_name_substitutes_once() {
        assert_eq!(
            expand_name("cargo mutants -f {targets}", "targets", "src/a.rs"),
            "cargo mutants -f src/a.rs"
        );
    }

    #[test]
    fn bind_extracts_the_file_token() {
        let mac = parse("cargo mutants -f {targets}")
            .expect("parse")
            .expect("macro");
        let command = rule("cargo mutants -f {targets}", MatchMode::Prefix);
        assert_eq!(
            bind(
                &mac,
                &command,
                &argv(&["cargo", "mutants", "-f", "src/a.rs"])
            ),
            Some("src/a.rs".to_owned())
        );
        assert_eq!(
            bind(
                &mac,
                &command,
                &argv(&["cargo", "mutants", "-f", "src/a.rs", "--jobs", "4"])
            ),
            Some("src/a.rs".to_owned()),
            "prefix mode keeps trailing args"
        );
        assert_eq!(
            bind(&mac, &command, &argv(&["cargo", "build", "-f", "src/a.rs"])),
            None,
            "literal token mismatch"
        );
    }

    #[test]
    fn bind_honours_affixes_and_exact_mode() {
        let mac = parse("ruff --file={files}").expect("parse").expect("macro");
        let command = rule("ruff --file={files}", MatchMode::Exact);
        assert_eq!(
            bind(&mac, &command, &argv(&["ruff", "--file=src/a.rs"])),
            Some("src/a.rs".to_owned())
        );
        assert_eq!(
            bind(&mac, &command, &argv(&["ruff", "--other=src/a.rs"])),
            None,
            "prefix must match"
        );
        assert_eq!(
            bind(&mac, &command, &argv(&["ruff", "--file=src/a.rs", "x"])),
            None,
            "exact mode rejects trailing args"
        );
        assert_eq!(
            bind(&mac, &command, &argv(&["ruff", "--file="])),
            None,
            "empty binding is not a file"
        );
    }
}
