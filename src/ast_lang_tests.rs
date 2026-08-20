//! Tests for the language table.
//!
//! The load-bearing one is [`every_entry_really_parses`]. `ast_grep_language`
//! answers a compiled-out grammar with `unimplemented!()` rather than an error,
//! so a `cfg` typo in `TABLE` — an entry enabled by a feature that does not
//! turn its parser on — would not fail to compile. It would abort the process
//! the first time a manifest named that language. Touching every entry's
//! parser here is what turns "unreachable" from a claim into a check.

use std::path::Path;

use super::{ALL, TABLE, available, by_extension, by_name, is_known};

#[test]
fn every_entry_really_parses() {
    for entry in TABLE {
        // Returning at all is the assertion: compiling a pattern loads the
        // grammar, so an entry whose `cfg` does not match its parser's feature
        // panics here instead of in front of a user. `$A` matches every node,
        // so a grammar that parsed the empty source into a root and nothing
        // else answers with exactly one match.
        if let Ok(matched) = crate::ast::select(entry.grammar, "$A", None, b"", "the empty source")
        {
            assert!(
                matched.len() <= 1,
                "`{}` found {} nodes in an empty source",
                entry.name,
                matched.len()
            );
        }
    }
}

#[test]
fn every_enabled_entry_is_a_known_language() {
    for entry in TABLE {
        assert!(
            is_known(entry.name),
            "`{}` is in TABLE but missing from ALL, so a build without it would report it as \
             unsupported rather than as not-built",
            entry.name
        );
    }
}

#[test]
fn known_names_are_sorted_and_unique() {
    let mut sorted = ALL.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.as_slice(), ALL.as_slice());
}

#[test]
fn table_is_sorted_by_name() {
    let names: Vec<&str> = TABLE.iter().map(|entry| entry.name).collect();
    let mut sorted = names.clone();
    sorted.sort_unstable();
    assert_eq!(names, sorted, "`available()` promises a sorted list");
}

#[test]
fn no_extension_maps_to_two_languages() {
    let mut seen: Vec<(&str, &str)> = Vec::new();
    for entry in TABLE {
        for extension in entry.extensions {
            if let Some((claimed, owner)) = seen.iter().find(|(ext, _)| ext == extension) {
                panic!(
                    "`{claimed}` is claimed by both `{owner}` and `{}`",
                    entry.name
                );
            }
            seen.push((extension, entry.name));
        }
    }
}

#[test]
fn a_name_this_build_lacks_resolves_to_nothing() {
    assert!(by_name("no-such-language").is_none());
}

#[test]
fn an_unknown_extension_infers_nothing() {
    assert!(by_extension(Path::new("Justfile")).is_none());
    assert!(by_extension(Path::new("notes.unheardof")).is_none());
}

#[test]
fn available_names_what_the_table_holds() {
    let listed = available();
    for entry in TABLE {
        assert!(listed.contains(entry.name), "`{}` is listed", entry.name);
    }
}

/// The default build's promise, asserted rather than assumed: `cargo install
/// mmz` with no features can run the `ast:` example the docs lead with.
#[cfg(feature = "lang-rust")]
#[test]
fn the_default_build_parses_rust() {
    assert!(by_name("rust").is_some());
    assert!(by_extension(Path::new("src/lib.rs")).is_some());
}
