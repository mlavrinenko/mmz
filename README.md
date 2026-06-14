# mmz

[![CI](https://github.com/mlavrinenko/mmz/actions/workflows/ci.yml/badge.svg)](https://github.com/mlavrinenko/mmz/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/mmz.svg)](https://crates.io/crates/mmz)
[![License: MIT](https://img.shields.io/crates/l/mmz.svg)](LICENSE-MIT)

memoized command runner

Prefix any command with `mmz`. When the matched rule's declared inputs are
byte-for-byte unchanged since the command last succeeded, `mmz` skips it and
exits 0. Otherwise it runs the command, streams its output, and records the
result on success.

It is not a build system: no task ordering, no dependency graph, no
output/artifact tracking, no remote cache. It answers one question per
invocation — are this rule's inputs unchanged since it last passed? See
[Non-goals](#non-goals).

## Install

```bash
cargo install mmz
```

Or download a pre-built binary from the
[latest release](https://github.com/mlavrinenko/mmz/releases/latest).

## Usage

```
mmz <command> [args...]   run a command, skipping it when its inputs are unchanged
mmz --init                write a starter mmz.yaml in the current directory
mmz --status              show each rule's freshness as a table
mmz --status=json         the same as JSON, with each rule's inputs and hashes
mmz --status=json-schema  print the JSON Schema for --status=json
mmz --prune               delete cache records whose rule no longer exists
mmz --schema              print the mmz.yaml JSON Schema
mmz --version             print version
mmz --help                print help
mmz -- <command> [args]   run a command whose name begins with a dash
```

Scaffold a manifest, then wrap commands wherever memoization is wanted — a
Justfile recipe line, a shell, a git hook:

```bash
mmz --init                # writes a starter mmz.yaml
mmz cargo test            # skipped when the rust inputs are unchanged
mmz --status              # show each rule's freshness
```

`mmz` receives a clean argv vector (not a shell-expanded string), so matching is
robust and there is no nesting blind spot. Wrappers go outside it
(`chronic mmz cargo test`). There is no `just`/runner integration and no
`--no-memo`: a bare command without `mmz` is simply unmemoized.

## Manifest

`mmz.yaml` — the nearest one, searching upward from the working directory.

```yaml
# yaml-language-server: $schema=https://raw.githubusercontent.com/mlavrinenko/mmz/main/schema/mmz.schema.json
scopes:
  rust: ["**/*.rs", "Cargo.toml", "Cargo.lock", "rust-toolchain.toml"]
commands:
  - name: cargo test       # matcher and cache identity
    inputs: [rust]
#   match: exact           # match only the bare command, no trailing args (default prefix)
# cache_dir: .mmz          # where throwaway records live (default; must be git-ignored)
# gitignore: true          # skip git-ignored paths when expanding globs (default)
# strict: [no_match, no_inputs]   # the default; list a subset to relax, [] to relax all
```

- `scopes`: named glob sets, declared once and referenced by many commands, so a
  shared input path lives in one place. Globs follow the common convention — `*`
  stays within a directory, `**` crosses directories.
- `commands`: ordered rules. Each has a `name` (the matcher and cache identity),
  `inputs` (scope names whose globs, unioned, are the rule's input set), and an
  optional `match` (`prefix`, the default, or `exact`; see [Matching](#matching)).
- `gitignore` (default `true`): glob expansion skips git-ignored paths, so build
  artifacts never enter an input set. Explicitly listed literal paths are always
  kept. The `.git` directory is never traversed; symlinks are not followed.
- `cache_dir` (default `.mmz`): directory for throwaway records, relative to the
  manifest. Keep it git-ignored; set it to relocate state (e.g. `.cache/mmz`).
- `strict` (default: all): the runtime cases `mmz` errors on rather than falling
  back — `no_match` (no rule matches) and `no_inputs` (a matched rule resolves to
  zero files). Omit for both; list a subset to relax the rest; `[]` to relax all.

The manifest is validated at load: command names must be non-empty and unique,
every referenced scope must be defined, and `strict` names must be known. Run
`mmz --schema` for the full JSON Schema.

## Matching

A rule's `name` is split on whitespace into tokens, and it matches when those
tokens are a prefix of the invoked argv: `cargo test` matches `cargo test` and
`cargo test --workspace`, but not `cargo build` and not the bare `cargo`.
Matching is on whole tokens, so `car` does not match `cargo`.

Rules are tried in manifest order; the first match wins. Order specific rules
before general ones. The cache identity is the matched rule (its `name`), not
the full argv, so you control granularity by how specifically rules are written:
split a rule or narrow its matcher when one rule conflates commands with
different real inputs.

Set `match: exact` on a rule to match only the bare command, no trailing args —
so `cargo test` and `cargo test --release` become separate cache identities. It
only narrows a rule, never causing a wrongful skip: an unmatched invocation
falls to the `no_match` case (error, or passthrough when relaxed).

## Correctness contract

The governing asymmetry, because the failure is silent:

- Under-declaring a rule's inputs → `mmz` skips a command that should have run →
  false green. Dangerous.
- Over-declaring inputs → an unnecessary re-run. Harmless.

So a rule's scopes must be a superset of every file any matching invocation
could depend on. When in doubt, broaden the scope. Toolchain sensitivity is
modeled as ordinary inputs: add `rust-toolchain.toml` or `flake.lock` to a scope
and a toolchain bump busts the cache. `mmz` trusts file content, not the ambient
environment.

`mmz` fails closed: a missing or invalid manifest always errors, and unmatched
commands or matched rules with no inputs error too unless `strict` relaxes them
(then they run unmemoized). What `mmz` never does is skip a command whose inputs
it has not confirmed unchanged.

## State and exit codes

Records live in a git-ignored cache directory (`.mmz/` by default), one YAML file
per rule, written atomically (temp file + rename) so a crash or concurrent writer
never leaves a truncated record. Derived, throwaway state — do not commit it. A
record is fresh only when its `status` is `ok` and its content digest, format,
algorithm, and command all still match; anything else re-runs.

`mmz --status` shows each rule's verdict and the age of its record;
`mmz --status=json` adds every resolved input and its content hash so you can
`jq` out what changed, and `mmz --status=json-schema` prints its schema. Renaming
or removing a rule orphans its record; `mmz --prune` sweeps the unclaimed ones.

| Code | Meaning |
| ---- | ------- |
| 0    | skipped (fresh) or the command succeeded |
| *n*  | the wrapped command's own exit code |
| 2    | usage error (empty invocation, unknown option, `--init` over an existing manifest) |
| 3    | strict refusal (no matching rule, or a matched rule with no inputs) |
| 4    | manifest missing or invalid |
| 70   | internal error |
| 127  | command could not be spawned |

## Non-goals

`mmz` follows the Unix philosophy — one thing, done right — so these stay out of
scope:

- Task orchestration: no execution order or dependency graph; use a task runner.
- Output replay: only the exit code is cached, never stdout, stderr, or artifacts.
- Automatic dependency tracing: no strace; scopes are declared explicitly.
- Remote caching: state is strictly local and throwaway.
- Deep runner integration: no plugins or hooks; `mmz` is a dumb CLI prefix.

## Library

`mmz` is published as a library crate too; the binary is a thin wrapper.
`mmz::run(&argv, cwd)` memoizes one invocation and returns its exit code;
`mmz::status`, `mmz::prune`, and `mmz::Manifest` cover the rest.

```rust
let argv = vec!["cargo".to_owned(), "test".to_owned()];
std::process::exit(mmz::run(&argv, std::path::Path::new("."))?.into());
```

## Dogfooding

`mmz` memoizes its own checks: [`mmz.yaml`](mmz.yaml) declares the rules and the
[`Justfile`](Justfile) `check` recipe wraps `cargo test`, `cargo clippy`,
`cargo fmt`, and `cargo machete` with the locally built binary, so a no-op
`just check` skips them.

## Development

Prerequisites: [Nix](https://nixos.org/) with flakes enabled.

```bash
direnv allow         # or: nix develop

just check           # fmt + clippy + tests + file-size + drift check (memoized)
just build
just test
just cover           # code coverage (70% minimum)
just fmt             # format code
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for coding conventions.

## License

MIT
