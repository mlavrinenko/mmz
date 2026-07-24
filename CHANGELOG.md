# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed

- `mmz --is-fresh` now expands parametric (`{scope}`-fanned) rules, the way
  `--status` and `--prune` already did. A bare or `--tag`-filtered gate reports
  one verdict per per-file expansion (keyed on the expanded identity, not the
  literal template), and a targeted `--is-fresh -- <command> <file>` gates the
  single expansion `<file>` binds to. Previously any manifest with a parametric
  rule wedged the gate: it reported the unexpandable `{scope}` template as
  `never` and a targeted invocation returned `no rule matches`.
- Glob input resolution now includes a symlink that resolves to a regular file,
  matching how a literal path is already resolved (symlinked directories are
  still not traversed). Previously a symlinked source matched by a glob was
  silently dropped from the input set, so edits to its target could skip the
  cache undetected.

### Changed

- `mmz --status` and `mmz --is-fresh` resolve a rule's shared `inputs` glob set
  once per rule instead of once per expansion, so enumerating a parametric rule
  over a large tree no longer re-walks the filesystem per fanned file. Output
  is unchanged.

## [0.5.0] - 2026-07-24

### Added

- Tags: a command rule can carry `tags: [..]`, and `mmz --is-fresh --tag <tag>`
  (repeatable `--tag`/`-t`, ANDed across repeats) narrows the gate to rules
  carrying every listed tag instead of the whole manifest; untagged rules
  never match. `mmz --status`/`--status=json` support the same filter. One
  manifest can now hold both a gating subset and other memoized commands a
  gate should ignore. Tags are trimmed, case-faithful, and duplicates within
  one rule are rejected at load.

### Changed

- Breaking (library): `mmz::freshness::evaluate` takes a third `tags: &[String]`
  parameter. Passing `Some(argv)` alongside a non-empty tag filter is now a
  usage error (`Error::TagWithCommand`) — a targeted command already resolves
  to one rule. `mmz::status::report` and `report_json` likewise take a `tags`
  parameter.

## [0.4.0] - 2026-07-18

### Added

- Parametric rules: a single `{scope}` macro in a command `name` fans the rule
  over that scope's files, yielding one per-file cache record per matched file.
  Each record is keyed by the expanded name and scoped to its own file plus the
  rule's shared `inputs` pins, so a file busts only its own record — the
  one-rule-per-file form without the hand-list. The macro is one whitespace
  token (embeddable, e.g. `--file={scope}`); the bound file must be a member of
  the scope. `mmz --status` enumerates the expansions and `mmz --prune` sweeps a
  record once its file is gone. Two rules resolving to the same identity is a
  hard error.
- MindTape task tracking: adopt MindTape for project task tracking
  (`tasks/`, driven via `mt` CLI).

### Changed

- Build QA: replaced ad-hoc Nix dev shell tooling with `qahq` for shared QA
  tools.
- Dropped `sccache` from the build — the `RUSTC_WRAPPER` env var is no longer
  set, making sandboxed and downstream builds simpler without the wrapper.

## [0.3.0] - 2026-06-30

### Added

- `mmz --is-fresh [-- <command>]`: a freshness gate that asserts a command's
  cache is fresh without running it. Exits 0 when the matched rule is fresh, 1
  when it is not (stale, never run, last failed, or no inputs); a no-match is a
  strict refusal (exit 3) and a missing or invalid manifest still exits 4. With
  no command it gates every rule at once. It is the inverse of wrapping — where
  `mmz <command>` runs a stale command, `mmz --is-fresh -- <command>` refuses it
  — so a git hook can require that an expensive check was already run and
  memoized without paying to run it on the spot.

## [0.2.0] - 2026-06-27

### Changed

- Breaking: the manifest moved from `mmz.yaml` to `.mmz/config.yaml`, and the
  default `cache_dir` from `.mmz` to `.mmz/cache`. Everything mmz needs now lives
  under one `.mmz/` directory. `mmz --init` writes the config plus a
  `.mmz/.gitignore` that ignores the cache, so adding mmz costs one tracked entry
  and never touches the project's root `.gitignore`. Input globs and `cache_dir`
  resolve against the project root (the directory holding `.mmz`). Migrate by
  moving `mmz.yaml` to `.mmz/config.yaml`, dropping any root `.gitignore` entry
  for the old cache, and re-running `mmz --init` in a scratch dir to copy the
  generated `.mmz/.gitignore`.

### Added

- `on_hit`: an optional message printed to stderr when a command is skipped (a
  cache hit). Supports `{cache:<field>}` macros that pull a field straight from
  the matched rule's cache record, can be overridden per command (or silenced
  with `""`), and is now scaffolded by `mmz --init`.

## [0.1.1] - 2026-06-15

### Fixed

- Package builds no longer fail when sccache is absent. The
  `rustc-wrapper = "sccache"` dev speedup moved from a committed
  `.cargo/config.toml` — which naersk vendored into the sandboxed `nix build`,
  where cargo then tried and failed to exec a missing `sccache` — to the flake
  dev shell's `RUSTC_WRAPPER`. It still applies under `nix develop` (loaded via
  direnv), but never in the build sandbox or for downstream flake consumers.

## [0.1.0] - 2026-06-15

### Added

- Memoized command runner: prefix a command with `mmz` to skip it when the
  matched rule's declared inputs are unchanged since it last succeeded.
- `mmz.yaml` manifest with named `scopes`, ordered `commands`, and `gitignore`
  (default true).
- Per-rule `match`: `prefix` (default, token-prefix) or `exact` (the whole
  command, no trailing args), so near-identical invocations can be separate
  cache identities.
- `cache_dir` (default `.mmz`): relocate the throwaway cache directory.
- `strict` list (default: all): the runtime cases mmz errors on rather than
  falling back — `no_match` and `no_inputs`. Use a subset, or `[]`, to relax.
- `mmz --init`, `mmz --status`, `mmz --prune`, and `mmz --schema` actions.
  `mmz --status` shows each rule's record age; `mmz --prune` drops records whose
  rule no longer exists.
- `mmz --status=json`: the freshness report as JSON, listing each rule's
  resolved inputs with content hashes (and `ran_at`) for scripting and `jq`.
  `mmz --status=json-schema` prints its JSON Schema.
- JSON Schemas for `mmz.yaml` and the status output under `schema/`.
- Library crate: `mmz::run`, `mmz::status`, `mmz::prune`, and `mmz::Manifest`
  expose the same engine the binary wraps.
- `mmz --init` pins the scaffolded `$schema` URL to the `v{version}` tag of the
  mmz that wrote it (not `main`), so projects pinning different mmz versions each
  validate against the matching schema. `mmz --help` and `mmz --version` report
  the running version.

### Notes

- Fails closed by default: a missing or invalid manifest always errors, as do
  unmatched commands and matched rules with no inputs (relaxable via `strict`).
- Cache records are written atomically (temp file + rename), so a crash or a
  concurrent writer can never leave a truncated record.
