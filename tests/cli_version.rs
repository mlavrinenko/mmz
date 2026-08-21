//! What `mmz --version` and `mmz --help` claim about the binary running them.
//!
//! Split out of `cli.rs` when the version line grew a grammar count. The count
//! is the only thing that distinguishes the two binaries a release publishes
//! under one version — a default build parsing Rust alone, and a `lang-all`
//! build parsing every grammar — so it is what a bug report is read against,
//! and it earns tests of its own rather than a shared file's tail.

use assert_cmd::Command;
use predicates::prelude::{PredicateBooleanExt, predicate};

/// `mmz --version`'s stdout, trimmed.
fn version_line() -> String {
    let output = Command::cargo_bin("mmz")
        .expect("binary should build")
        .arg("--version")
        .output()
        .expect("run --version");
    String::from_utf8(output.stdout)
        .expect("utf-8 stdout")
        .trim()
        .to_owned()
}

/// The count out of `(N ast lang…)`, with the noun that followed it.
fn parsed_count(line: &str) -> (usize, String) {
    let inner = line
        .rsplit_once('(')
        .and_then(|(_, tail)| tail.strip_suffix(')'))
        .unwrap_or_else(|| panic!("no parenthesised grammar count in `{line}`"));
    let (count, noun) = inner
        .split_once(' ')
        .unwrap_or_else(|| panic!("grammar count `{inner}` is not `N ast lang…`"));
    let count: usize = count
        .parse()
        .unwrap_or_else(|_| panic!("grammar count `{count}` is not a number"));
    (count, noun.to_owned())
}

#[test]
fn reports_the_version() {
    Command::cargo_bin("mmz")
        .expect("binary should build")
        .arg("--version")
        .assert()
        .success()
        .stdout(
            predicate::str::contains(env!("CARGO_PKG_VERSION"))
                .and(predicate::str::contains(" ast lang")),
        );
}

/// The noun has to agree with the number it follows. Checked against mmz's own
/// count rather than against a constant, because the count legitimately differs
/// per build — but the agreement never does, and the stock one-grammar build is
/// exactly the case a plural would embarrass.
#[test]
fn the_grammar_count_agrees_with_its_noun() {
    let line = version_line();
    let (count, noun) = parsed_count(&line);
    let expected = if count == 1 { "ast lang" } else { "ast langs" };
    assert_eq!(noun, expected, "`{line}` counts {count}");
}

/// The number itself, for the one feature set whose answer is knowable here.
/// Twenty-eight, not the twenty-seven grammar crates: `typescript` and `tsx`
/// share a crate and are two names a manifest may write.
///
/// This is what proves the release matrix wired `--features lang-all` to the
/// binary it labelled `full`, rather than building the default twice.
#[cfg(feature = "lang-all")]
#[test]
fn a_full_build_carries_every_language() {
    let line = version_line();
    let (count, _) = parsed_count(&line);
    assert_eq!(count, 28, "`{line}` is not a full build");
}

#[test]
fn help_shows_version() {
    Command::cargo_bin("mmz")
        .expect("binary should build")
        .arg("--help")
        .assert()
        .success()
        .stdout(
            predicate::str::contains(env!("CARGO_PKG_VERSION"))
                .and(predicate::str::contains("memoized command runner")),
        );
}
