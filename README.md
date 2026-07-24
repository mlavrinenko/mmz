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
mmz <command> [args...]      run a command, skipping it when its inputs are unchanged
mmz --init                   write a starter .mmz/config.yaml in the current directory
mmz --status [--tag t]...    show each rule's freshness as a table
mmz --status=json            the same as JSON, with each rule's inputs and hashes
mmz --status=json-schema     print the JSON Schema for --status=json
mmz --is-fresh [--tag t]... [-- cmd]   exit 0 if cmd's rule (or every/tagged rule) is fresh
mmz --prune                  delete cache records whose rule no longer exists
mmz --schema                 print the config JSON Schema
mmz --version                print version
mmz --help                   print help
mmz -- <command> [args]      run a command whose name begins with a dash
```

Scaffold a manifest, then wrap commands wherever memoization is wanted — a
Justfile recipe line, a shell, a git hook:

```bash
mmz --init                # writes .mmz/config.yaml and .mmz/.gitignore
mmz cargo test            # skipped when the rust inputs are unchanged
mmz --status              # show each rule's freshness
```

`mmz` receives a clean argv vector (not a shell-expanded string), so matching is
robust and there is no nesting blind spot. Wrappers go outside it
(`chronic mmz cargo test`). There is no `just`/runner integration and no
`--no-memo`: a bare command without `mmz` is simply unmemoized.

## Manifest

`.mmz/config.yaml` — the nearest one, searching upward from the working
directory. Everything mmz needs lives under one `.mmz/` directory: the tracked
config plus a `.mmz/.gitignore` (written by `--init`) that ignores the cache,
so a project gains one entry and its root `.gitignore` stays untouched.

```yaml
# yaml-language-server: $schema=https://raw.githubusercontent.com/mlavrinenko/mmz/v0.1.0/schema/mmz.schema.json
scopes:
  rust: ["**/*.rs", "Cargo.toml", "Cargo.lock", "rust-toolchain.toml"]
commands:
  - name: cargo test       # matcher and cache identity
    inputs: [rust]
#   match: exact           # match only the bare command, no trailing args (default prefix)
#   on_hit: "tests fresh"  # per-rule cache-hit note, overriding the global one below
# cache_dir: .mmz/cache    # where throwaway records live (default; must be git-ignored)
# gitignore: true          # skip git-ignored paths when expanding globs (default)
# strict: [no_match, no_inputs]   # the default; list a subset to relax, [] to relax all
on_hit: "mmz: skipped {cache:command} (inputs unchanged)"   # stderr note on a hit
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
- `cache_dir` (default `.mmz/cache`): directory for throwaway records, relative
  to the project root (the directory holding `.mmz`). Keep it git-ignored; set it
  to relocate state (e.g. `.cache/mmz`).
- `strict` (default: all): the runtime cases `mmz` errors on rather than falling
  back — `no_match` (no rule matches) and `no_inputs` (a matched rule resolves to
  zero files). Omit for both; list a subset to relax the rest; `[]` to relax all.
- `on_hit` (default: none): a line printed to stderr when a command is skipped.
  Embed `{cache:<field>}` to pull a field straight from the cache record
  (`command`, `ran_at`, `input_digest`, …). Set it per command to override the
  global note, or to `""` to silence one. `mmz --init` scaffolds a default.

The manifest is validated at load: command names must be non-empty and unique,
every referenced scope must be defined, and `strict` names must be known. Run
`mmz --schema` for the full JSON Schema.

`mmz --init` pins the `$schema` URL to the `v{version}` tag of the mmz that wrote
it, not `main`, so a project keeps validating against the schema its mmz was
built for even when different projects pin different mmz versions. `mmz --help`
and `mmz --version` report the running version.

## Tags

A command rule can carry `tags: [..]`; `mmz --is-fresh --tag <tag>` and `mmz
--status --tag <tag>` then narrow to rules carrying every listed tag, instead
of every rule in the manifest. Repeat `--tag`/`-t` to require more than one —
repeats AND together (there is no OR: call `mmz --is-fresh` once per tag for
that). A rule with no `tags:` never matches a `--tag` filter; a bare `mmz
--is-fresh` (no `--tag`, no command) still gates every rule regardless of tags.

```yaml
commands:
  - name: cargo test
    inputs: [rust]
    tags: [gate]
  - name: cargo bench
    inputs: [rust]
    tags: [bench]
```

`mmz --is-fresh --tag gate` here checks only `cargo test`; a bare `mmz
--is-fresh` still checks both. One manifest can then hold a gating subset
(wired into `just check` or a pre-push hook) alongside other memoized commands
a gate should ignore, instead of splitting `.mmz/config.yaml` per concern.

Tags are case-faithful, trimmed, and blank entries are dropped; declaring the
same tag twice on one rule is a manifest error.

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

## Parametric rules (per-file fan)

A single `{scope}` macro in a rule's `name` fans it over that scope's files: one
per-file cache record per matched file, without hand-listing each as its own
rule.

```yaml
scopes:
  lint-targets: ["src/**/*.rs"]
  rust-pins:    ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml"]
commands:
  - name: "ruff check {lint-targets}"   # {scope} macro ⇒ parametric rule
    inputs: [rust-pins]                 # shared pins added to every record
```

`ruff check {lint-targets}` stands for `ruff check src/a.rs`, `ruff check
src/b.rs`, … — each a distinct cache identity. A record's inputs are the
`inputs` pins plus its own file, so editing `src/a.rs` busts only that file's
record. The macro is one whitespace token but may sit inside one
(`--file={lint-targets}`). `mmz` stays a prefix — you drive the loop
(`for f in src/**/*.rs; do mmz ruff check "$f"; done`).

The bound file must be a member of the scope (gitignore-filtered), so an
off-list path falls through to the no-match case rather than inventing a record.
`mmz --status` enumerates one row per expanded file, `mmz --is-fresh` gates one
verdict per expanded file (a bare `--is-fresh` over a parametric rule passes
only when every one of its expansions is fresh; a targeted `--is-fresh --
<command> <file>` gates the one expansion `<file>` binds to), `mmz --prune`
drops a record once its file leaves the tree, and two rules resolving to the
same expanded identity is an error, not a silently picked winner.

Per-file scoping is only honest when the command depends on that one file plus
the pins (a per-file lint/format/typecheck). A whole-crate command like `cargo
mutants -f {scope}` compiles siblings, so a sibling edit can leave a file's
record wrongly fresh — use the fan there knowing you trade correctness for speed.

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

Records live in a git-ignored cache directory (`.mmz/cache` by default), one YAML file
per rule, written atomically (temp file + rename) so a crash or concurrent writer
never leaves a truncated record. Derived, throwaway state — do not commit it. A
record is fresh only when its `status` is `ok` and its content digest, format,
algorithm, and command all still match; anything else re-runs.

`mmz --status` shows each rule's verdict and the age of its record;
`mmz --status=json` adds every resolved input and its content hash so you can
`jq` out what changed, and `mmz --status=json-schema` prints its schema. Renaming
or removing a rule orphans its record; `mmz --prune` sweeps the unclaimed ones.

`mmz --is-fresh -- <command>` gates a command on its cache without running it:
exit 0 when its rule is already fresh, exit 1 otherwise. With no command,
`mmz --is-fresh` gates every rule at once. It is the inverse of wrapping — where
`mmz <command>` runs a stale command, `mmz --is-fresh -- <command>` refuses it —
so a git hook can require that an expensive check was already run (and memoized)
without paying to run it on the spot:

```bash
# pre-push: refuse the push if the VM checks were not run, but never run them here
mmz --is-fresh -- just check-vm || { echo "stale: run 'just check-vm' first" >&2; exit 1; }
```

| Code | Meaning |
| ---- | ------- |
| 0    | fresh, skipped, or the command succeeded |
| 1    | `--is-fresh`: the targeted rule (or some rule) is not fresh |
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

`mmz` memoizes its own checks: [`.mmz/config.yaml`](.mmz/config.yaml) declares the rules and the
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
