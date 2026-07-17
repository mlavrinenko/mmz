//! Maps an invoked command to the rule that memoizes it.

use crate::manifest::{Command, MatchMode};

/// Returns the first command rule that matches `argv` under its match mode.
///
/// Rules are tried in manifest order, so the author orders specific rules
/// before general ones. A `prefix` rule named `cargo test` matches `cargo test`,
/// `cargo test --workspace`, and so on, but not `cargo build`; an `exact` rule
/// matches only the bare `cargo test`.
#[must_use]
pub fn first_match<'a>(commands: &'a [Command], argv: &[String]) -> Option<&'a Command> {
    commands.iter().find(|command| matches(command, argv))
}

/// True when `command` matches `argv` under its [`MatchMode`]. An empty matcher
/// (a name with no tokens) never matches.
pub(crate) fn matches(command: &Command, argv: &[String]) -> bool {
    let tokens: Vec<&str> = command.name.split_whitespace().collect();
    if tokens.is_empty() {
        return false;
    }
    match command.match_mode {
        MatchMode::Prefix => leads(&tokens, argv),
        MatchMode::Exact => argv.len() == tokens.len() && leads(&tokens, argv),
    }
}

/// True when `tokens` equal the leading `tokens.len()` entries of `argv`.
fn leads(tokens: &[&str], argv: &[String]) -> bool {
    match argv.get(..tokens.len()) {
        Some(prefix) => prefix.iter().map(String::as_str).eq(tokens.iter().copied()),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::first_match;
    use crate::manifest::{Command, MatchMode};

    fn rule(name: &str) -> Command {
        Command {
            name: name.to_owned(),
            inputs: Vec::new(),
            match_mode: MatchMode::Prefix,
            on_hit: None,
        }
    }

    fn exact_rule(name: &str) -> Command {
        Command {
            name: name.to_owned(),
            inputs: Vec::new(),
            match_mode: MatchMode::Exact,
            on_hit: None,
        }
    }

    fn argv(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|part| (*part).to_owned()).collect()
    }

    #[test]
    fn matches_token_prefix() {
        let rules = [rule("cargo test")];
        assert!(first_match(&rules, &argv(&["cargo", "test", "--workspace"])).is_some());
        assert!(first_match(&rules, &argv(&["cargo", "build"])).is_none());
        assert!(
            first_match(&rules, &argv(&["cargo"])).is_none(),
            "shorter than matcher"
        );
    }

    #[test]
    fn first_rule_in_order_wins() {
        let rules = [rule("cargo"), rule("cargo test")];
        let hit = first_match(&rules, &argv(&["cargo", "test"])).expect("match");
        assert_eq!(hit.name, "cargo", "earlier rule wins on a tie");
    }

    #[test]
    fn token_boundary_is_respected() {
        let rules = [rule("car")];
        assert!(
            first_match(&rules, &argv(&["cargo"])).is_none(),
            "no partial-token match"
        );
    }

    #[test]
    fn exact_rejects_trailing_args() {
        let rules = [exact_rule("cargo test")];
        assert!(
            first_match(&rules, &argv(&["cargo", "test"])).is_some(),
            "exact matches the bare command"
        );
        assert!(
            first_match(&rules, &argv(&["cargo", "test", "--workspace"])).is_none(),
            "exact rejects extra args a prefix rule would accept"
        );
    }
}
