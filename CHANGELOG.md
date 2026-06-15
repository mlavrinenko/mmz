# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
