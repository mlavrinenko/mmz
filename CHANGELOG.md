# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed

- The gate probes in this repo's own manifest now sort their JSON
  (`jq -S`). `just --dump --dump-format json` renders the same recipe with its
  object keys in different orders across `just` versions, so hashing the
  unsorted selection made every gate read stale whenever the `just` resolving
  on PATH was not the dev shell's — a `mt done` outside `nix develop` reported
  ten stale gates against a worktree whose checks had just passed. This removes
  the accidental version dependency; the deliberate one, a probe measuring
  whatever tool PATH offers rather than the declared toolchain, is unchanged
  and tracked separately.
- `mmz --is-fresh` no longer exits 0 over a selection that holds no rule. A
  `--tag` no rule carries — a typo, a rename, a rule that quietly lost its
  `tags:` entry — used to be indistinguishable from a passing build, which is
  the false green the tool exists to refuse. It is now an error (exit 7,
  never relaxable) naming the tags it filtered on and listing the ones the
  manifest declares. Two more ways to gate nothing are refused with it: a
  manifest that declares no `commands:` at all, and a selected rule that fans
  over a scope resolving to no files. The code is distinct from `1` so a hook
  branching on `$?` can tell a stale build from a gate pointed at nothing.
- `mmz --status --tag <tag>` no longer answers a filter that kept no rule with
  "no rules defined in …" — a sentence written for a manifest with no
  `commands:` at all, and untrue of one whose rules simply do not carry that
  tag. It still exits 0, because a report asserts nothing, and now names which
  emptiness it is along with the tags the manifest declares.
- The project root a manifest is anchored to is now canonicalized, so it agrees
  with the canonical paths provenance records. A root reached through a symlink
  — a library caller passing `mmz::run(&argv, path)` its own path, never having
  touched `current_dir()` — made every source path in `--status` and
  `--dump-config` render absolute instead of root-relative, because stripping
  one representation off the other could not match. Globs, `outputs` and
  `cache_dir` resolve against the same root, so they are now consistent too.
- A composition error names a file the way a report does: relative to the
  project root when it sits under it, absolute otherwise. Previously
  `scope \`rust\` is declared in both …` printed two absolute paths while
  `--status` and `--dump-config` printed the same files root-relative, because
  the loader was never handed the root. A fragment outside the tree — a store
  path — still prints in full, which is the only form of it a reader can act
  on.

### Added

- A documentation site under `www/`, built from Typst sources with
  [tola](https://github.com/tola-rs/tola-ssg) and searchable via Pagefind,
  replacing the single hand-written `index.html` that restated the README from
  memory. Twelve pages, and the reference ones are generated rather than
  written: the manifest reference from `mmz --schema`, the CLI reference and the
  exit-code table from `mmz --help`, the rule-state vocabulary from
  `mmz --status=json-schema`. Every transcript is captured from the real binary
  run against a new `examples/demo` fixture, so no example on the site can drift
  from what the binary does.
- `README.md`, `AGENTS.md`, `CONTRIBUTING.md` and `docs/contributing/*.md` are
  now rendered from `docs/src/*.typ` by typlite (`just docs md`), and
  `just docs::md-check` fails when a committed file has drifted from its source.
  The README drops from 500 lines to 76: it is a front door now, and the manual
  it used to carry lives on the site where it can be generated.
- Four documentation gates in `just check`: `docs::check` builds the site and
  validates every internal link, `docs::md-check` catches Markdown drift,
  `check-doc-coverage` fails when `mmz --help` advertises an action with no
  hand-written note, and `check-doc-facts` does the same for every manifest key,
  exit code, gate and page. Each compares in both directions, so a note for
  something that no longer exists fails too.
- `MMZ_NOW` pins "now" to a Unix epoch in seconds, so output carrying a time is
  reproducible. It is resolved once per invocation and threaded through both
  surfaces that carry one — the `ran_at` a cache record stamps, and the `AGE`
  column `mmz --status` renders — so two stamps in one run can never disagree.
  A value that is not an epoch is refused with exit 2 by every action that reads
  the clock, never ignored: a silent fall-back to the system clock would hide
  the misconfiguration and restore the non-determinism the pin exists to remove.
  Deliberately its own variable rather than `SOURCE_DATE_EPOCH`, which dev
  shells and CI export at the 1980 zip-epoch floor. Freshness is untouched — mmz
  compares digests, not times.

### Changed

- `just check` runs its arms as `just memo <gate>` — that is, `mmz just <gate>` —
  so `.mmz/config.yaml` names RECIPES rather than mirroring their command lines.
  The mirror it replaces was hand-maintained and desynchronised silently: a
  recipe growing a flag left the rule matching, and memoizing, the old command's
  identity.
- Each gate rule now pins its own recipe body through a probe
  (`just --dump --dump-format json | jq -e -c '.recipes["<name>"]'`) instead of
  sharing one scope over the whole `Justfile`. Editing an unrelated recipe used
  to bust every rule and re-run clippy and the full test suite; it now busts
  nothing. Verified both directions: a non-gate recipe edit leaves all ten rules
  fresh, and a `clippy` body edit makes exactly one stale.
- Gate membership is derived from `[group("gate")]` and cross-checked against
  `check`'s own dependency list, which is also the gate table's row order in
  CONTRIBUTING.md. A gate tagged but not wired — or wired but not tagged — fails
  the docs build naming both sides.
- `.linecop.yaml` caps Typst, Shell, jq and CSS, and exempts the generated
  Markdown: typlite emits one line per paragraph, so those line counts were never
  a meaningful size proxy. The cap that matters is on the Typst source. The
  README's 500-line override is gone with the 500-line README.
- `outdatty.yaml`'s groups now couple sources to the hand-written prose a
  generator cannot check — the notes files — rather than to whole documents that
  are no longer hand-edited.
- The Pages workflow builds the site through the dev shell instead of uploading
  `www/` verbatim.
- `www/generate.sh` pins its captures with `MMZ_NOW` rather than rewriting a
  record's `ran_at` afterwards with `sed`, so the cache record shown on the site
  is the file the binary wrote, byte for byte. One normalization is left, on the
  fixture's absolute temp path.
- `mmz::cache::write` takes the resolved `Clock` the record is stamped from.
  Library callers that build an `Outcome` by hand pass `Clock::resolve()?` (or
  `Clock::pinned(secs)`) alongside it.

## [0.6.0] - 2026-08-16

### Added

- Command-driven inputs: a top-level `probes:` map declares named commands whose
  stdout is hashed into the input digest of every rule whose `inputs:` names
  them — one namespace with scopes, so the two cannot share a name. That is how
  a rule depends on part of a file (one recipe body out of a `Justfile`) or on
  something that is not a file (`rustc -vV`). A probe runs under `sh -c` from the
  project root with stdin closed, once per invocation however many rules name it.
  A non-zero exit, a failed spawn, or — without `allow_empty: true` — empty
  stdout exits 6 naming the probe, consuming no output and writing no record.
  A wrong scope costs time, a wrong probe can lie, so correctness stays yours:
  assert the shape in the probe (`jq -e`) and a bad shape becomes a failed exit.
  A stale gate names the probe that moved; `--status=json` reports each digest.
- A command rule may declare `outputs:` — literal artifact paths relative to the
  project root — and is fresh only when its inputs still hash the same AND every
  declared output exists. A record claims a command exited 0 while its inputs
  hashed to H, and for a producer command that claim can be undone without
  touching an input (`cargo clean`, a fresh clone, a pruned `target/`): such a
  record is not stale, it is void, and it used to read fresh forever. Existence
  only, stat-ed never walked, so `gitignore` never applies and a glob is an
  error. A voided record reports `missing-output` and names the path in the
  `--is-fresh` reason, the `--status` table, and `--status=json`.
- Exit code `5`: a wrapped command exited 0 without producing a declared output.
  mmz names the missing path and writes no record — skipping it silently would
  leave a rule that quietly never hits again. A failing run is unaffected.
- A scope value may now be an object — `globs:` plus an optional `gitignore:` —
  overriding the manifest-level `gitignore` for that scope alone; the array form
  is unchanged and inherits it. That is what lets a rule depend on a build
  artifact, which lives in a git-ignored path by definition and so resolved to
  nothing, leaving its rule fresh forever. A rule may mix both kinds. An object
  without `globs`, or with an empty `globs`, is a manifest error.

### Changed

- Library API: `Manifest::scopes` maps to `manifest::Scope`, `Manifest::globs_for`
  is replaced by `Manifest::glob_groups` (feeding `resolve::expand_groups`), and
  `cache::write` takes a `cache::Outcome`. The `mmz` CLI surface is unaffected.

## [0.5.1] - 2026-07-24

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

- A non-fresh `mmz --is-fresh` gate now prints one remediation line after the
  per-rule reasons: `re-run each listed command under mmz (e.g. \`mmz just
  check\`) to record a pass — a standalone run is not tracked`. mmz only
  observes a command it wraps, so a pass run standalone is invisible to the
  cache and the rule stays stale or `never`; the new hint names the cure once,
  not per rule. It is suppressed when every offending rule is `no-inputs`,
  whose fix is in the manifest, not a re-run.
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
