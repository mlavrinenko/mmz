//! Maps an invoked command to the rule that memoizes it.

use crate::manifest::Command;

/// Returns the first command rule whose `name` is a token-prefix of `argv`.
///
/// Rules are tried in manifest order, so the author orders specific rules
/// before general ones. A rule named `cargo test` matches `cargo test`,
/// `cargo test --workspace`, and so on, but not `cargo build`.
#[must_use]
pub fn first_match<'a>(commands: &'a [Command], argv: &[String]) -> Option<&'a Command> {
    commands
        .iter()
        .find(|command| is_prefix(&command.name, argv))
}

/// True when the whitespace-split tokens of `name` are a leading slice of
/// `argv`. An empty matcher never matches.
fn is_prefix(name: &str, argv: &[String]) -> bool {
    let tokens: Vec<&str> = name.split_whitespace().collect();
    if tokens.is_empty() {
        return false;
    }
    match argv.get(..tokens.len()) {
        Some(prefix) => prefix.iter().map(String::as_str).eq(tokens.iter().copied()),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::first_match;
    use crate::manifest::Command;

    fn rule(name: &str) -> Command {
        Command {
            name: name.to_owned(),
            inputs: Vec::new(),
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
}
