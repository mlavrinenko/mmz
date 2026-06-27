# mmz

## Agent Rules

- See and use [Justfile](Justfile). Add any repeatable and regular operations there.
- At the end ensure that `just fix-check` is green.
- `mmz` dogfoods itself: [.mmz/config.yaml](.mmz/config.yaml) declares the rules
  and `just check` wraps `cargo test`/`clippy`/`fmt`/`machete` with the locally
  built binary (`_mmz` recipe builds it first), so a no-op `just check` skips
  them. When you add or rename a check command in the recipe, update
  .mmz/config.yaml's rules to match.
- Tests: inline `#[cfg(test)]` units; CLI/integration in `tests/` (`assert_cmd` + `predicates`). `just fix-check` auto-ejects inline tests from oversized files via `ejectest`.
- Coverage and CRAP gates run separately (CI + locally): `just cover` then
  `just crap`. If `just crap` flags a function, add tests or reduce its
  branching — don't raise the threshold to dodge it.
- Be careful with the context. Omit non-necessary command outputs using `chronic` or `grep`.
- [outdatty.yaml](outdatty.yaml) couples sources to dependents. When `just check`
  reports drift, update the listed dependents, then run `just outdatty-update`
  to re-confirm. Add a group whenever you introduce files that must stay in sync.
- Commits: Conventional Commits, English, no `Refs:` footer (no external tracker
  by policy). See [CONTRIBUTING.md](CONTRIBUTING.md#commits).

See [CONTRIBUTING.md](CONTRIBUTING.md) for project conventions and code standards.
