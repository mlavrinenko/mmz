//! Cache-hit notices: expand a user template into the line printed when a
//! command is skipped.
//!
//! A template may embed `{namespace:key}` macros. The `cache` namespace pulls a
//! field straight from the matched rule's cache record (`{cache:command}`,
//! `{cache:ran_at}`, ...). An unknown namespace or key is left verbatim, so a
//! typo is visible rather than silently dropped, and a stray brace is preserved.
//! The namespace split leaves room for future sources (e.g. `runtime`).

use std::collections::BTreeMap;

/// Expands every `{namespace:key}` macro in `template` against `cache` (the
/// record fields). Unresolved macros and unmatched braces are preserved as-is.
#[must_use]
pub fn expand(template: &str, cache: &BTreeMap<String, String>) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        if let Some(close) = after.find('}') {
            let token = &after[..close];
            if let Some(value) = resolve(token, cache) {
                out.push_str(&value);
            } else {
                out.push('{');
                out.push_str(token);
                out.push('}');
            }
            rest = &after[close + 1..];
        } else {
            out.push('{');
            rest = after;
        }
    }
    out.push_str(rest);
    out
}

/// Resolves one `namespace:key` token, or `None` when the namespace or key is
/// unknown (so the caller can keep it literal).
fn resolve(token: &str, cache: &BTreeMap<String, String>) -> Option<String> {
    let (namespace, key) = token.split_once(':')?;
    match namespace {
        "cache" => cache.get(key).cloned(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::expand;

    fn cache() -> BTreeMap<String, String> {
        [
            ("command".to_owned(), "cargo test".to_owned()),
            ("ran_at".to_owned(), "1718000000".to_owned()),
        ]
        .into_iter()
        .collect()
    }

    #[test]
    fn substitutes_known_cache_fields() {
        assert_eq!(
            expand("skip {cache:command} @ {cache:ran_at}", &cache()),
            "skip cargo test @ 1718000000"
        );
    }

    #[test]
    fn leaves_unknown_key_and_namespace_literal() {
        assert_eq!(
            expand("{cache:nope} {runtime:now}", &cache()),
            "{cache:nope} {runtime:now}",
            "unknown key and unknown namespace are kept verbatim"
        );
    }

    #[test]
    fn preserves_braces_without_a_macro() {
        assert_eq!(expand("plain {text} a{b", &cache()), "plain {text} a{b");
    }

    #[test]
    fn empty_template_expands_to_empty() {
        assert_eq!(expand("", &cache()), "");
    }
}
