//! Tests for the in-process AST matcher.
//!
//! Most of these are about the *rendering*, because that is where an `ast:`
//! probe can quietly lie. A digest that cannot tell `+` from `-`, or that
//! collapses the whitespace inside a string literal, reports fresh against a
//! file whose meaning moved — and unlike a wrong scope, nothing downstream
//! catches it.

use super::{AstFailure, resolve_lang};

/// Every rendering assertion needs a language, and Rust is the one the default
/// build carries. A build without it skips these rather than failing: the
/// claims are about the renderer, which is language-agnostic, and the table's
/// own tests cover whatever grammars a build does have.
#[cfg(feature = "lang-rust")]
mod rust {
    use ast_grep_language::SupportLang;
    use std::path::Path;

    use super::super::{AstFailure, resolve_lang, select};

    /// Renders every match of `pattern` over `source`, joined the way
    /// `probe_digest` joins them — so these compare exactly what a digest would.
    fn rendered(pattern: &str, source: &str) -> String {
        let matched = select(SupportLang::Rust, pattern, source.as_bytes(), "the fixture")
            .expect("the fixture matches");
        matched
            .iter()
            .map(|bytes| String::from_utf8_lossy(bytes).into_owned())
            .collect::<Vec<String>>()
            .join("\n")
    }

    /// The headline claim: reformatting a signature is not an edit to it.
    #[test]
    fn whitespace_between_tokens_is_not_an_input() {
        let dense = rendered("pub fn $N($$$A) -> $R", "pub fn foo(a: u8) -> u8 { a }");
        let loose = rendered(
            "pub fn $N($$$A) -> $R",
            "pub   fn\n    foo(\n        a: u8,\n    ) -> u8 { a }",
        );
        assert_eq!(dense, loose);
    }

    /// The trap a named-children-only walk (tree-sitter's own `to_sexp`) falls
    /// into: operators are anonymous nodes, so collapsing them makes `a + b`
    /// and `a - b` one input. That is a missed bust, which is the direction
    /// this tool must never fail in. One pattern over both sources, so the
    /// only difference reaching the renderer is the operator itself.
    #[test]
    fn an_operator_is_part_of_the_rendering() {
        let plus = rendered("let $N = $V;", "fn f() { let x = a + b; }");
        let minus = rendered("let $N = $V;", "fn f() { let x = a - b; }");
        assert_ne!(plus, minus);
        assert!(
            plus.contains(r#"(+ "+")"#),
            "the `+` token survives: {plus}"
        );
    }

    /// Leaf text is exact, so the whitespace *inside* a literal stays content
    /// even though the whitespace around it does not.
    #[test]
    fn whitespace_inside_a_literal_is_an_input() {
        let wide = rendered("let $N = $V;", r#"fn f() { let s = "a   b"; }"#);
        let narrow = rendered("let $N = $V;", r#"fn f() { let s = "a b"; }"#);
        assert_ne!(wide, narrow);
    }

    #[test]
    fn renaming_a_matched_function_changes_the_digest() {
        let foo = rendered("pub fn $N() {}", "pub fn foo() {}");
        let bar = rendered("pub fn $N() {}", "pub fn bar() {}");
        assert_ne!(foo, bar);
    }

    /// What the feature is for: a comment outside every match cannot move the
    /// digest, which a scope naming the file could never promise.
    #[test]
    fn a_comment_outside_the_match_is_not_an_input() {
        let bare = rendered("pub fn $N() {}", "pub fn foo() {}\n");
        let noted = rendered(
            "pub fn $N() {}",
            "// a thought about foo, revised\npub fn foo() {}\n",
        );
        assert_eq!(bare, noted);
    }

    /// Document order is kept rather than sorted, so a reordered file is a
    /// changed file. See the module docs for why that is the safe direction.
    #[test]
    fn match_order_follows_the_document() {
        let one = rendered("pub fn $N() {}", "pub fn a() {}\npub fn b() {}");
        let other = rendered("pub fn $N() {}", "pub fn b() {}\npub fn a() {}");
        assert_ne!(one, other);
        assert_eq!(one.lines().count(), 2, "both functions matched");
    }

    #[test]
    fn a_pattern_matching_nothing_yields_no_matches() {
        let matched = select(
            SupportLang::Rust,
            "pub fn $N() {}",
            b"fn private() {}",
            "the fixture",
        )
        .expect("a pattern that matches nothing still compiles");
        assert!(matched.is_empty());
    }

    /// The refusal `has_error()` buys. tree-sitter recovers an unbalanced
    /// *pattern* into an error node rather than failing, so without this check
    /// a typo would compile fine and match nothing — indistinguishable from a
    /// correct pattern over a file that genuinely has no match, and waived
    /// outright by `allow_empty: true`.
    #[test]
    fn a_pattern_the_grammar_could_not_parse_is_refused() {
        let failed = select(
            SupportLang::Rust,
            "pub fn $N(",
            b"pub fn f() {}",
            "the fixture",
        )
        .expect_err("an unbalanced pattern is refused");
        assert!(matches!(failed, AstFailure::Pattern { .. }), "{failed:?}");
    }

    #[test]
    fn an_empty_pattern_is_refused() {
        let failed = select(SupportLang::Rust, "", b"fn f() {}", "the fixture")
            .expect_err("an empty pattern is refused");
        assert!(matches!(failed, AstFailure::Pattern { .. }), "{failed:?}");
    }

    /// A lone metavariable matches every node, which is a whole-file hash
    /// spelled at length. Deliberately *not* refused: over-declaring is the
    /// safe direction here exactly as it is for a scope, and mmz does not
    /// invent refusals for inputs that are merely wider than they need to be.
    #[test]
    fn a_bare_metavariable_matches_everything() {
        let matched = select(SupportLang::Rust, "$A", b"fn f() {}", "the fixture")
            .expect("a lone metavariable is legal");
        assert!(matched.len() > 1, "it swept the tree: {}", matched.len());
    }

    #[test]
    fn bytes_that_are_not_utf8_are_refused() {
        let failed = select(
            SupportLang::Rust,
            "pub fn $N() {}",
            &[0xff, 0xfe],
            "the fixture",
        )
        .expect_err("invalid utf-8 is refused");
        assert!(matches!(failed, AstFailure::NotText { .. }), "{failed:?}");
    }

    /// A file mid-edit parses into a tree holding `ERROR` nodes rather than
    /// failing, which is deliberate — see the module docs. What catches it is
    /// the match set coming back empty, which `probe_digest` refuses.
    #[test]
    fn a_source_with_a_syntax_error_still_parses() {
        let matched = select(
            SupportLang::Rust,
            "pub fn $N() {}",
            b"pub fn foo() {}\npub fn unfinished(",
            "the fixture",
        )
        .expect("a broken source is recovered rather than refused");
        assert_eq!(matched.len(), 1, "the intact function still matched");
    }

    #[test]
    fn an_extension_infers_the_language() {
        let inferred = resolve_lang(None, Some(Path::new("src/lib.rs")), "`src/lib.rs`")
            .expect("`.rs` infers rust");
        assert_eq!(format!("{inferred:?}"), "Rust");
    }

    #[test]
    fn a_declared_language_beats_the_extension() {
        let declared = resolve_lang(Some("rust"), Some(Path::new("notes.txt")), "`notes.txt`")
            .expect("`lang:` needs no help from the path");
        assert_eq!(format!("{declared:?}"), "Rust");
    }
}

/// A `run:` probe has no path to infer from, so it must say. Inferring a
/// language from a command line would be a guess, and a wrong guess here parses
/// source as the wrong grammar and hashes whatever fell out.
#[test]
fn a_command_probe_without_lang_is_refused() {
    let failed = resolve_lang(None, None, "the output of `cargo expand`")
        .expect_err("a run: probe must declare its language");
    assert!(
        matches!(failed, AstFailure::LanguageUnknown { .. }),
        "{failed:?}"
    );
}

/// An extension no bundled grammar claims is the same refusal, and it is loud:
/// mmz never falls back to parsing an unknown file as something plausible.
#[test]
fn an_unrecognised_extension_without_lang_is_refused() {
    let failed = resolve_lang(None, Some(std::path::Path::new("Justfile")), "`Justfile`")
        .expect_err("an unknown extension is refused");
    assert!(
        matches!(failed, AstFailure::LanguageUnknown { .. }),
        "{failed:?}"
    );
}

/// The distinction the whole feature matrix rests on: a language mmz supports
/// but this build omitted must say so, and name the flag. Telling the reader it
/// is unsupported would send them to the wrong place entirely.
#[test]
fn a_language_this_build_lacks_names_its_feature() {
    let Some(absent) = crate::ast_lang::ALL
        .iter()
        .find(|name| crate::ast_lang::by_name(name).is_none())
    else {
        return; // A `--features lang-all` build has nothing to test here.
    };
    let failed =
        resolve_lang(Some(absent), None, "`src/lib.rs`").expect_err("an absent grammar is refused");
    let AstFailure::LanguageMissing { .. } = failed else {
        panic!("expected a missing-grammar failure for `{absent}`, got {failed:?}");
    };
    let message = failed.to_string();
    assert!(
        message.contains(&format!("--features lang-{absent}")),
        "the message names the flag that fixes it: {message}"
    );
}

/// And a language mmz has no grammar for in any build is a different message,
/// because the answer is a different action.
#[test]
fn a_language_mmz_has_never_had_is_unsupported() {
    let failed = resolve_lang(Some("cobol"), None, "`payroll.cbl`")
        .expect_err("an unsupported language is refused");
    assert!(
        matches!(failed, AstFailure::LanguageUnsupported { .. }),
        "{failed:?}"
    );
}
