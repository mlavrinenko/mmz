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
        captured(pattern, None, source)
    }

    /// The same, under a `capture:` list. Takes the names as the manifest would
    /// write them, so a test reads as the YAML it stands for.
    pub(super) fn captured(pattern: &str, capture: Option<&[&str]>, source: &str) -> String {
        let names: Option<Vec<String>> =
            capture.map(|listed| listed.iter().map(|name| (*name).to_owned()).collect());
        let matched = select(
            SupportLang::Rust,
            pattern,
            names.as_deref(),
            source.as_bytes(),
            "the fixture",
        )
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
            None,
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
            None,
            b"pub fn f() {}",
            "the fixture",
        )
        .expect_err("an unbalanced pattern is refused");
        assert!(matches!(failed, AstFailure::Pattern { .. }), "{failed:?}");
    }

    #[test]
    fn an_empty_pattern_is_refused() {
        let failed = select(SupportLang::Rust, "", None, b"fn f() {}", "the fixture")
            .expect_err("an empty pattern is refused");
        assert!(matches!(failed, AstFailure::Pattern { .. }), "{failed:?}");
    }

    /// A lone metavariable matches every node, which is a whole-file hash
    /// spelled at length. Deliberately *not* refused: over-declaring is the
    /// safe direction here exactly as it is for a scope, and mmz does not
    /// invent refusals for inputs that are merely wider than they need to be.
    #[test]
    fn a_bare_metavariable_matches_everything() {
        let matched = select(SupportLang::Rust, "$A", None, b"fn f() {}", "the fixture")
            .expect("a lone metavariable is legal");
        assert!(matched.len() > 1, "it swept the tree: {}", matched.len());
    }

    #[test]
    fn bytes_that_are_not_utf8_are_refused() {
        let failed = select(
            SupportLang::Rust,
            "pub fn $N() {}",
            None,
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
            None,
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

/// The `capture:` list, which is what makes "the public API but not the
/// bodies" expressible: the pattern still has to span the body to match a
/// function that has one, and the list is what keeps that body out of the
/// digest.
#[cfg(feature = "lang-rust")]
mod captures {
    use ast_grep_language::SupportLang;

    use super::super::{AstFailure, select};
    use super::rust::captured;

    /// The task's motivating example, spelled as the manifest would.
    const SIGNATURE: &str = "pub fn $NAME($$$ARGS) -> $RET { $$$BODY }";
    const SIGNATURE_PARTS: [&str; 3] = ["NAME", "ARGS", "RET"];

    fn names(listed: &[&str]) -> Vec<String> {
        listed.iter().map(|name| (*name).to_owned()).collect()
    }

    /// The headline. `$$$BODY` is in the pattern because a Rust signature stops
    /// being a node of its own once a body follows it — so the only way to
    /// reach the function at all is to span the body, and the only way to drop
    /// the body from the input is to leave it out of `capture:`.
    #[test]
    fn a_body_the_pattern_spans_is_not_an_input_when_it_is_not_captured() {
        let before = captured(
            SIGNATURE,
            Some(&SIGNATURE_PARTS),
            "pub fn one(a: u8) -> u8 { a }",
        );
        let after = captured(
            SIGNATURE,
            Some(&SIGNATURE_PARTS),
            "pub fn one(a: u8) -> u8 { a + 0 }",
        );
        assert_eq!(before, after, "the body was matched but not captured");
    }

    /// The other half, without which the first would be a probe that measures
    /// nothing: every captured part is still an input.
    #[test]
    fn each_captured_part_is_still_an_input() {
        let base = captured(
            SIGNATURE,
            Some(&SIGNATURE_PARTS),
            "pub fn one(a: u8) -> u8 { a }",
        );
        for changed in [
            "pub fn renamed(a: u8) -> u8 { a }",
            "pub fn one(a: u16) -> u8 { a as u8 }",
            "pub fn one(a: u8, b: u8) -> u8 { a }",
            "pub fn one(a: u8) -> u16 { a.into() }",
        ] {
            assert_ne!(
                base,
                captured(SIGNATURE, Some(&SIGNATURE_PARTS), changed),
                "`{changed}` moved a captured part"
            );
        }
    }

    /// A capture list is the *set* of parts that matter, so the order it was
    /// typed in is presentation — the same call `json:` makes on object keys.
    /// Sorting cannot hide an edit here, because only two spellings of one set
    /// normalise together.
    #[test]
    fn the_order_of_the_capture_list_is_not_an_input() {
        let source = "pub fn one(a: u8) -> u8 { a }";
        let listed = captured(SIGNATURE, Some(&SIGNATURE_PARTS), source);
        let shuffled = captured(SIGNATURE, Some(&["RET", "NAME", "ARGS"]), source);
        assert_eq!(listed, shuffled);
    }

    /// Dropping a name from the list is an edit to what is measured, which is
    /// what stops the sort above from being a narrowing.
    #[test]
    fn dropping_a_name_from_the_list_changes_the_digest() {
        let source = "pub fn one(a: u8) -> u8 { a }";
        assert_ne!(
            captured(SIGNATURE, Some(&SIGNATURE_PARTS), source),
            captured(SIGNATURE, Some(&["NAME", "ARGS"]), source),
        );
    }

    /// A multi capture that bound nothing renders as a bare `($ARGS)`, distinct
    /// from every count above it — so emptying an argument list stays an edit
    /// rather than collapsing into "no arguments were ever there".
    #[test]
    fn a_multi_capture_that_bound_nothing_is_distinct() {
        let empty = captured(SIGNATURE, Some(&["ARGS"]), "pub fn one() -> u8 { 1 }");
        let one = captured(SIGNATURE, Some(&["ARGS"]), "pub fn one(a: u8) -> u8 { a }");
        assert_eq!(empty, "($ARGS)", "the rendering names the empty capture");
        assert_ne!(empty, one);
    }

    /// Still document order, still one line per match: `capture:` narrows what
    /// each match contributes and changes nothing about which matched.
    #[test]
    fn matches_stay_one_per_line_in_document_order() {
        let one = captured(
            SIGNATURE,
            Some(&["NAME"]),
            "pub fn a() -> u8 { 1 }\npub fn b() -> u8 { 2 }",
        );
        let other = captured(
            SIGNATURE,
            Some(&["NAME"]),
            "pub fn b() -> u8 { 2 }\npub fn a() -> u8 { 1 }",
        );
        assert_eq!(one.lines().count(), 2, "both functions matched");
        assert_ne!(one, other, "reordering the file is still an edit");
    }

    /// The refusal this feature could not ship without: an undefined name binds
    /// no node, so it would render as an empty `($TYPO)` in every match and
    /// narrow the probe silently — and `allow_empty` could not even be blamed,
    /// because the matches are all there.
    #[test]
    fn a_name_the_pattern_does_not_define_is_refused() {
        let failed = select(
            SupportLang::Rust,
            SIGNATURE,
            Some(&names(&["NAME", "TYPO"])),
            b"pub fn one(a: u8) -> u8 { a }",
            "the fixture",
        )
        .expect_err("an undefined capture is refused");
        let AstFailure::CaptureUndefined { .. } = failed else {
            panic!("expected an undefined-capture failure, got {failed:?}");
        };
        let message = failed.to_string();
        assert!(message.contains("TYPO"), "names the miss: {message}");
        for defined in SIGNATURE_PARTS {
            assert!(
                message.contains(defined),
                "names what the pattern does define: {message}"
            );
        }
    }

    /// It is raised from the compiled pattern, so a source with no matches at
    /// all still gets told about the list rather than about the emptiness —
    /// which is the error a reader can act on.
    #[test]
    fn an_undefined_name_is_refused_even_when_nothing_matched() {
        let failed = select(
            SupportLang::Rust,
            SIGNATURE,
            Some(&names(&["TYPO"])),
            b"fn private() {}",
            "the fixture",
        )
        .expect_err("the list is checked before the source is");
        assert!(
            matches!(failed, AstFailure::CaptureUndefined { .. }),
            "{failed:?}"
        );
    }

    /// An anonymous `$$$` binds nothing in ast-grep, so it is not nameable at
    /// all — and the answer a reader gets is the list of what is, rather than
    /// silence.
    #[test]
    fn an_anonymous_multi_capture_is_not_nameable() {
        let failed = select(
            SupportLang::Rust,
            "pub fn $NAME($$$) -> $RET { $$$ }",
            Some(&names(&["ARGS"])),
            b"pub fn one(a: u8) -> u8 { a }",
            "the fixture",
        )
        .expect_err("`$$$` captures nothing");
        let message = failed.to_string();
        assert!(message.contains("NAME"), "names what is defined: {message}");
        assert!(message.contains("RET"), "names what is defined: {message}");
    }

    /// A pattern with no metavariables at all says so in words rather than
    /// trailing off into an empty list.
    #[test]
    fn a_pattern_that_captures_nothing_says_so() {
        let failed = select(
            SupportLang::Rust,
            "pub fn one() {}",
            Some(&names(&["NAME"])),
            b"pub fn one() {}",
            "the fixture",
        )
        .expect_err("there is nothing to capture");
        assert!(
            failed.to_string().contains("no metavariables at all"),
            "{failed}"
        );
    }
}
