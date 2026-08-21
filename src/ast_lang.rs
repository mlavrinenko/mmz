//! Which languages this build of mmz can parse, and how a probe reaches one.
//!
//! Every grammar is a compile-time choice, because a tree-sitter grammar is
//! not small: the set ast-grep ships dwarfs the binary that would carry it.
//! What each one costs is measured rather than remembered — `just
//! measure-sizes` writes `www/sizes.yaml` and the docs read it, while a figure
//! repeated into this comment would have nothing to keep it true. Shipping all
//! of them would charge every user who never writes an `ast:` probe for the
//! ones they will never use, and shipping one fixed small set would permanently
//! exclude every language outside it. So each is a cargo feature, a documented
//! default set is on, and [`ALL`] names the rest so a manifest asking for one
//! gets told how to get it rather than told it does not exist.
//!
//! # Why this table exists at all
//!
//! `ast_grep_language` has `SupportLang::from_str` and `from_path` already, and
//! neither is usable here: `SupportLang`'s variants are *not* feature-gated,
//! only the parsers behind them are, and a variant whose parser was compiled
//! out answers `get_ts_language()` with `unimplemented!()`. Reaching one is an
//! abort, not an error. [`TABLE`] is gated element by element and is the only
//! way a language is looked up, so a parser mmz was not built with is refused
//! by name before anything asks it to parse. `ast_lang_tests.rs` asserts every
//! entry really parses, which is what makes that claim more than a comment.

use std::path::Path;

use ast_grep_language::SupportLang;

/// One language this build can parse: the name a manifest writes under `lang:`,
/// the extensions that infer it, and the grammar behind both.
struct Entry {
    /// The manifest spelling, lowercase.
    name: &'static str,
    /// Extensions that infer this language from a `file:` path, without the
    /// dot. Each extension belongs to exactly one entry; `ast_lang_tests.rs`
    /// fails on an overlap rather than letting resolution depend on order.
    extensions: &'static [&'static str],
    /// The ast-grep grammar. Only ever named inside a `cfg`-enabled element,
    /// so this field can never hold a compiled-out parser.
    grammar: SupportLang,
}

/// Every language mmz has a `lang-*` cargo feature for, whether or not this
/// build enabled it.
///
/// Kept separate from [`TABLE`] and deliberately *not* gated: it is what lets
/// a miss distinguish "mmz was built without this grammar, here is the flag"
/// from "mmz has never heard of this language". Those want different answers,
/// and a reader who gets the wrong one goes looking in the wrong place.
pub(crate) const ALL: [&str; 28] = [
    "bash",
    "c",
    "cpp",
    "csharp",
    "css",
    "dart",
    "elixir",
    "go",
    "haskell",
    "hcl",
    "html",
    "java",
    "javascript",
    "json",
    "kotlin",
    "lua",
    "markdown",
    "nix",
    "php",
    "python",
    "ruby",
    "rust",
    "scala",
    "solidity",
    "swift",
    "tsx",
    "typescript",
    "yaml",
];

/// The languages this build actually carries a parser for.
///
/// One element per enabled feature. `tsx` and `typescript` share the
/// `lang-typescript` feature because they share a grammar crate, and are
/// separate entries because they are separate parsers: a `.tsx` file's `<div/>`
/// is a type assertion to the other one.
const TABLE: &[Entry] = &[
    #[cfg(feature = "lang-bash")]
    Entry {
        name: "bash",
        extensions: &["sh", "bash", "zsh"],
        grammar: SupportLang::Bash,
    },
    #[cfg(feature = "lang-c")]
    Entry {
        name: "c",
        extensions: &["c", "h"],
        grammar: SupportLang::C,
    },
    #[cfg(feature = "lang-cpp")]
    Entry {
        name: "cpp",
        extensions: &["cc", "cpp", "cxx", "hpp", "hxx"],
        grammar: SupportLang::Cpp,
    },
    #[cfg(feature = "lang-csharp")]
    Entry {
        name: "csharp",
        extensions: &["cs"],
        grammar: SupportLang::CSharp,
    },
    #[cfg(feature = "lang-css")]
    Entry {
        name: "css",
        extensions: &["css", "scss"],
        grammar: SupportLang::Css,
    },
    #[cfg(feature = "lang-dart")]
    Entry {
        name: "dart",
        extensions: &["dart"],
        grammar: SupportLang::Dart,
    },
    #[cfg(feature = "lang-elixir")]
    Entry {
        name: "elixir",
        extensions: &["ex", "exs"],
        grammar: SupportLang::Elixir,
    },
    #[cfg(feature = "lang-go")]
    Entry {
        name: "go",
        extensions: &["go"],
        grammar: SupportLang::Go,
    },
    #[cfg(feature = "lang-haskell")]
    Entry {
        name: "haskell",
        extensions: &["hs"],
        grammar: SupportLang::Haskell,
    },
    #[cfg(feature = "lang-hcl")]
    Entry {
        name: "hcl",
        extensions: &["hcl", "tf", "tfvars"],
        grammar: SupportLang::Hcl,
    },
    #[cfg(feature = "lang-html")]
    Entry {
        name: "html",
        extensions: &["html", "htm"],
        grammar: SupportLang::Html,
    },
    #[cfg(feature = "lang-java")]
    Entry {
        name: "java",
        extensions: &["java"],
        grammar: SupportLang::Java,
    },
    #[cfg(feature = "lang-javascript")]
    Entry {
        name: "javascript",
        extensions: &["js", "cjs", "mjs", "jsx"],
        grammar: SupportLang::JavaScript,
    },
    #[cfg(feature = "lang-json")]
    Entry {
        name: "json",
        extensions: &["json"],
        grammar: SupportLang::Json,
    },
    #[cfg(feature = "lang-kotlin")]
    Entry {
        name: "kotlin",
        extensions: &["kt", "kts"],
        grammar: SupportLang::Kotlin,
    },
    #[cfg(feature = "lang-lua")]
    Entry {
        name: "lua",
        extensions: &["lua"],
        grammar: SupportLang::Lua,
    },
    #[cfg(feature = "lang-markdown")]
    Entry {
        name: "markdown",
        extensions: &["md", "markdown"],
        grammar: SupportLang::Markdown,
    },
    #[cfg(feature = "lang-nix")]
    Entry {
        name: "nix",
        extensions: &["nix"],
        grammar: SupportLang::Nix,
    },
    #[cfg(feature = "lang-php")]
    Entry {
        name: "php",
        extensions: &["php"],
        grammar: SupportLang::Php,
    },
    #[cfg(feature = "lang-python")]
    Entry {
        name: "python",
        extensions: &["py", "pyi"],
        grammar: SupportLang::Python,
    },
    #[cfg(feature = "lang-ruby")]
    Entry {
        name: "ruby",
        extensions: &["rb"],
        grammar: SupportLang::Ruby,
    },
    #[cfg(feature = "lang-rust")]
    Entry {
        name: "rust",
        extensions: &["rs"],
        grammar: SupportLang::Rust,
    },
    #[cfg(feature = "lang-scala")]
    Entry {
        name: "scala",
        extensions: &["scala", "sc"],
        grammar: SupportLang::Scala,
    },
    #[cfg(feature = "lang-solidity")]
    Entry {
        name: "solidity",
        extensions: &["sol"],
        grammar: SupportLang::Solidity,
    },
    #[cfg(feature = "lang-swift")]
    Entry {
        name: "swift",
        extensions: &["swift"],
        grammar: SupportLang::Swift,
    },
    #[cfg(feature = "lang-typescript")]
    Entry {
        name: "tsx",
        extensions: &["tsx"],
        grammar: SupportLang::Tsx,
    },
    #[cfg(feature = "lang-typescript")]
    Entry {
        name: "typescript",
        extensions: &["ts", "mts", "cts"],
        grammar: SupportLang::TypeScript,
    },
    #[cfg(feature = "lang-yaml")]
    Entry {
        name: "yaml",
        extensions: &["yaml", "yml"],
        grammar: SupportLang::Yaml,
    },
];

/// The grammar a manifest's `lang:` names, or `None` when this build has none.
///
/// The comparison is exact rather than case-folded: a manifest key's value is
/// data, and `lang: Rust` quietly meaning `rust` is one more thing a reader has
/// to know. The miss is loud, so getting the case wrong costs a message rather
/// than a wrong answer.
pub(crate) fn by_name(name: &str) -> Option<SupportLang> {
    TABLE
        .iter()
        .find(|entry| entry.name == name)
        .map(|entry| entry.grammar)
}

/// The grammar `path`'s extension implies, or `None` when the extension maps to
/// nothing this build carries.
///
/// Only ever a *default*: a probe that sets `lang:` never reaches this, so an
/// extension mmz reads differently than the project does is corrected in the
/// manifest rather than worked around.
pub(crate) fn by_extension(path: &Path) -> Option<SupportLang> {
    let extension = path.extension()?.to_str()?;
    TABLE
        .iter()
        .find(|entry| entry.extensions.contains(&extension))
        .map(|entry| entry.grammar)
}

/// Every language name this build parses, comma-joined for an error message.
/// Sorted, because [`TABLE`] is sorted and a message that reorders itself
/// between builds is a message nobody trusts.
pub(crate) fn available() -> String {
    let names: Vec<&str> = TABLE.iter().map(|entry| entry.name).collect();
    if names.is_empty() {
        return "(none — this mmz was built with no `lang-*` feature)".to_owned();
    }
    names.join(", ")
}

/// How many languages this build parses — the count `mmz --version` reports,
/// so two binaries carrying one version number can be told apart without
/// having to provoke an error to find out which is which.
///
/// Counts [`TABLE`] entries rather than grammar crates. `typescript` and `tsx`
/// are one crate and two names a manifest may write, and the number worth
/// printing beside a version is the one a reader compares against their own
/// `lang:`.
pub(crate) fn count() -> usize {
    TABLE.len()
}

/// Whether `name` is a language mmz has a feature for, enabled here or not.
/// What separates "rebuild with this flag" from "no such language".
pub(crate) fn is_known(name: &str) -> bool {
    ALL.contains(&name)
}

#[cfg(test)]
#[path = "ast_lang_tests.rs"]
mod tests;
