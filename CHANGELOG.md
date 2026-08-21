# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `just check-changelog-history` asserts that every `## [x.y.z]` section of this
  file still reads exactly as the `vx.y.z` tag shipped it. A released section is
  a historical record, so an edit to one is either a mistake or a rewrite — and
  the mistake is what happened: the edit repaired in 0.8.0 swallowed the whole
  0.7.0 section into `Unreleased`, and ten green gates ran over it for four days
  and thirteen commits because nothing read this file. `outdatty` couples
  `Cargo.toml` to it as a _dependent_, which asks that a human re-confirmed, not
  that the content survived; `linecop` only counts its lines.

  The comparison is one `git show` per tag and no build, so it is a `just check`
  arm rather than a CI-only one — the check that blocks a merge and the check a
  contributor runs locally should not be different checks. The price is that CI
  must check out with `fetch-depth: 0`: a shallow clone has the tag refs but not
  the trees, and the gate refuses one rather than passing over an empty set.

- `just changelog-waive <version> <reason>` records a deliberate rewrite of a
  released section into `CHANGELOG.waivers`. The shape is `outdatty.lock`'s and
  so is the meaning — a recorded hash says a human looked, not that a tool
  agreed. It covers the rewritten section's own bytes rather than switching the
  check off for a version, so the next edit to a waived section fails exactly as
  the first would have, and a waiver whose section has gone back to matching its
  tag fails as an entry nobody rereads.

## [0.8.0] - 2026-08-21

### Added

- Releases now publish two binaries per target. `mmz-<target>` is unchanged —
  default features, the Rust grammar alone, and byte-for-byte what
  `cargo install mmz` builds. `mmz-full-<target>` is `--features lang-all`:
  every grammar, about eight times the size, and the only build for which a
  manifest naming a language can never be answered with "rebuild it yourself".

  The people who download a prebuilt binary are the people who did not want to
  build one, so shipping only the first left exactly them stuck — a
  `lang: python` probe failed with an error whose fix was a Rust toolchain.

  Two flavours, not three. A curated middle tier was considered and dropped:
  `default` fails predictably and `full` never fails, but a "popular" set fails
  for _some_ of your colleagues and not others, which is the trap the docs
  already warn about wearing a release asset's name. It is also a taste
  judgement that drifts, and a subset anyone actually wants is one
  `cargo install --features` away. `nix build …#full` is the Nix spelling.

- `mmz --version` now reports how many languages the build can parse —
  `mmz 0.7.0 (1 ast lang)`, `mmz 0.7.0 (28 ast langs)`. Which grammars a binary
  carries is a compile-time choice, so one version number describes more than
  one binary, and as of this release it describes two published ones. Without
  the count, "mmz 0.7.0 cannot parse Python" names a version that is true of
  both, and the list was reachable only by provoking the error that prints it.

  A count rather than the twenty-eight names, which would bury the version.
  Twenty-eight against twenty-seven grammar crates: `typescript` and `tsx` are
  two names a manifest may write and one crate on disk.

- `just test-lang-all` runs the suite with every grammar compiled in, on its own
  CI job. `just check` runs on default features, so `ast_lang_tests.rs`'s claim
  that every table entry really parses covered one language out of twenty-eight
  — and a binary whose selling point is the other twenty-seven cannot be the
  first thing to run them. Deliberately not a `check` arm: twenty-seven C
  compiles do not belong in front of the recipe people run in a loop.

  It found its first bug immediately: `a_language_this_build_lacks_names_the_feature_to_rebuild_with`
  hard-coded `kotlin` as the absent grammar, and under `lang-all` asserted a
  refusal that correctly did not happen. It is now gated on the absence of the
  grammar it names.

- `just measure-sizes` measures what each grammar costs a linked binary and
  writes `www/sizes.yaml`, which every binary-size figure in the docs is now
  read from. Thirty release builds — the grammar-free baseline, `default`,
  `lang-all`, and each grammar as its own delta against the baseline — so it is
  a recipe you run on purpose, not a gate.

  `www/generate-facts.sh` republishes the measurement, cross-checking its
  grammar set against the crate's own `lang-` features: a grammar added without
  a re-measure fails the docs build naming it, rather than rendering a table
  that quietly omits a language mmz can parse. `outdatty`'s `binary-size` group
  asks for the re-run when `Cargo.toml` or `Cargo.lock` moves under a recorded
  measurement — a review, not a rebuild, because whether a dependency bump moved
  the number enough to matter is a judgement.

- `ast:` and `lang:` on a probe, so a rule can depend on a *structural* slice of
  a source file — one function, one type, one impl block — by parsing it in
  process and matching an [ast-grep](https://ast-grep.github.io/) pattern:

  ```yaml
  probes:
    wire-types:
      file: src/types.rs
      ast: 'pub struct $NAME { $$$FIELDS }'
  ```

  Nothing is spawned, nothing has to be on `PATH`, and no regex is asked to
  pretend it is a parser. Everything the pattern did not match — comments,
  imports, private items — is free to move without re-running the rule, which
  is the input a scope naming the file cannot express.

  What is hashed is mmz's own rendering of the matched *tree*, not its text:
  every token exactly, the whitespace between them dropped. Reflowing a
  signature is not an edit to it; renaming one is, and so is `a + b` becoming
  `a - b`, because operators are nodes too. Matches join in document order,
  which is kept rather than sorted for the reason `json:` keeps array order —
  order a document chose is content, and sorting it would hide a real edit.

  A match is a whole node, so a pattern spanning a function's body depends on
  that body. `capture:` below narrows that.

  Grammars are not small: all twenty-seven ast-grep ships weigh about 40 MB
  linked against an mmz binary of 3.5 MB, so each is a cargo feature and a
  stock build carries `lang-rust` alone (`--features lang-all` for the lot).
  Naming a language this build lacks is a hard error that quotes the flag to
  rebuild with; naming one mmz has no grammar for at all is a different error,
  because it needs a different answer. mmz never falls back to parsing an
  unknown file as something plausible, and a pattern the grammar could only
  recover into an error node is refused rather than left to match nothing.

- `capture:` on an `ast:` probe, naming which of the pattern's metavariables
  are the input:

  ```yaml
  probes:
    public-api:
      file: src/lib.rs
      ast: 'pub fn $NAME($$$ARGS) -> $RET { $$$BODY }'
      capture: [NAME, ARGS, RET]
  ```

  Without it, "this gate depends on the public API of `lib.rs`, not on its
  bodies" had no spelling: a Rust signature stops being a node of its own once
  a body follows it, so the only pattern that reaches a real function spans the
  body too, and a match is a whole node.

  The pattern and the list answer different questions, which is the one way to
  get this wrong. The pattern decides which constructs match — `$$$BODY` is why
  a function *with* a body matches at all — and the list decides which parts of
  each are hashed. Dropping `$$$BODY` from the pattern would not narrow the
  input; it would stop matching the functions you meant.

  A capture renders as `($NAME …)` around the rendering of every node it bound,
  and the list is sorted before hashing: it is the *set* of parts that matter,
  so retyping it in another order is not an edit. That sort cannot hide one,
  unlike sorting match order would — only two spellings of one set ever
  normalise together. A multi capture that bound nothing renders as a bare
  `($ARGS)`, distinct from every count above it.

  A name the pattern does not define is a hard error naming what it does
  define, and this is the refusal the key could not ship without: an undefined
  name binds nothing, so it would render empty in every match and narrow the
  probe silently — with every match still present, so `allow_empty: true` would
  find nothing to complain about. An anonymous `$$$` or a `$_` binds nothing in
  ast-grep and cannot be named at all. Three more refusals land at load, where
  the manifest alone settles them: an empty list, a name that could never be a
  metavariable (`$NAME` copied straight out of the pattern), and a duplicate.

  The default is unchanged and is still the answer most of the time. Where a
  pattern can stop at the boundary you care about, let it; this is for the
  constructs whose grammar will not let it.

- `file:` and `json:` on a probe, so a rule can depend on one field of a JSON
  file with no subprocess at all:

  ```yaml
  probes:
    nixpkgs-input:
      file: flake.lock
      json: '.nodes["nixpkgs"]["locked"]["narHash"]'
  ```

  Every probe until now was a spawn. This one is not: mmz opens the file,
  parses it, selects, and hashes the result. No shell, nothing required on
  `PATH`, no shell quoting to get wrong, and no process per probe on an
  `mmz --is-fresh` that gates every rule at once — which is the operation whose
  entire value is being cheap enough to run in a hook.

  It also reaches inputs that were previously inexpressible. A scope names whole
  files, so a rule depending on one lockfile node had to hash a file with a
  hundred of them; bumping any input busted every rule pinning it. That is not a
  coarser version of the rule's dependency, it is a different dependency, and
  there was no way to write the real one.

  `run:` may carry a `json:` too, selecting out of a command's stdout instead of
  a file. That halves the spawns rather than removing them, and it is the weaker
  case — it earns its place by taking `jq` off the ambient-tool surface and by
  making canonical hashing structural rather than conventional.

  Which is the second thing this changes. mmz hashes its own rendering of the
  selected value — object keys sorted at every depth, array order preserved —
  never the bytes it read, so key order is not an input by construction. The
  `jq -S` on every shelled-out probe in this repo is that same property
  maintained by everyone remembering it, and forgetting once made a `just`
  upgrade read as ten stale gates.

  `json:` is jq, run in-process by an embedded engine (jaq), not a narrower
  path syntax. The reason is compatibility over time rather than power: the
  probes here already use `,` and `with_entries(select(…))`, so a path-only
  spelling would have had to change meaning in a later version, and a manifest
  key's semantics must not break under a reader.

  Everything about it fails closed, exit 6 with nothing recorded: an unreadable
  `file:`, bytes that are not exactly one JSON value, a program that does not
  compile or that raises against the document, and — the load-bearing one — a
  selection that measured nothing. `.a.b.c` against a document lacking them is
  not a failure in jq but a successful selection of `null`, and a probe tracking
  `null` reports one digest whatever the document does, leaving the rule fresh
  forever against an input nobody is measuring. That is exactly what `jq -e`
  exists to prevent, so `json:` refuses it too. `false` is a value and passes:
  jq conflates it with `null` only because a shell exit code cannot tell them
  apart, and mmz is under no such constraint. `allow_empty: true` opts into an
  empty selection, the same key and the same meaning it already had for stdout.

  `file:` and `run:` are mutually exclusive — a probe declaring both is a
  manifest error (exit 4) naming the probe, not a precedence rule for a reader
  to memorise. A `file:` with no `json:` is refused as well: hashing a whole
  file is what a scope is for, and a scope keeps the gitignore filter and
  reports which file moved, so a second spelling of it here would only be a
  quieter one. The probes in this repo's own manifest are unchanged; migrating
  them moves every gate's digest and belongs in its own reviewable change.

- `probe_shell`, a root-manifest key naming the argv every probe's `run` line
  is executed by, with the line appended as one final argument. It defaults to
  `["sh", "-c"]`, which is what every probe ran under before, so nothing
  changes for a manifest that omits it.

  A probe resolves its commands through whatever `PATH` the caller had, which
  quietly makes the caller's shell part of what the probe reports — and a probe
  is supposed to report the project. The same probe run inside a project shell
  and outside it can disagree about a tool's version, and the disagreement
  surfaces as an unexplained stale rule rather than as an error, because a
  digest that moved is indistinguishable from one that should have.
  `["direnv", "exec", ".", "sh", "-c"]` or
  `["nix", "develop", "--command", "sh", "-c"]` pins the answer.

  Root-manifest-only, like `cache_dir`, `gitignore`, `strict` and `on_hit`: a
  fragment setting it would leave undecidable which one governs a probe
  declared in a third file. An empty list is a load error (exit 4), since there
  would be nothing to spawn, and it is caught at load rather than in the spawn
  path. `mmz --dump-config` reports it alongside the other four, marked
  `(default)` when unwritten.

  This pins the environment; it does not make mmz aware of it. A probe measured
  under the wrong shell is still one mmz will trust. What the key buys is that
  there need no longer be a wrong shell to be measured under.

### Changed

- Release assets are archives rather than raw binaries: `.tar.gz` per unix
  target, `.zip` for Windows, each holding the binary under its plain name plus
  `LICENSE-MIT`, with a `SHA256SUMS` over the set. A tree-sitter parse table is
  a large array of small integers, and the `full` flavour compresses eight to
  one: 45.4 MB becomes 5.7 MB, across five targets. The default flavour, mostly
  code rather than tables, manages 5.5 MB to 2.0 MB.

  **This renames every asset**, which is why it did not ship with the flavour
  split. Nothing is pinned to the old names: every asset of every release from
  v0.1.1 to v0.7.0 sits at 0-1 downloads, there is no install script, and the
  docs link the releases page rather than any file. Releases up to v0.7.0 keep
  their raw binaries; a script pinned to `releases/latest/download/mmz-<target>`
  wants `mmz-<target>.tar.gz` from here on.

- The binary sizes the docs quote are measured rather than typed. The claim that
  a grammar is not small carried four hand-written figures, and the first had
  already rotted: no build had produced the 3.5 MB binary the page claimed since
  the jq and ast-grep engines landed. `Cargo.toml`, `src/ast_lang.rs` and both
  JSON Schemas now make the argument without quoting a number, because none of
  them can read one.

### Fixed

- Each gate now declares the tools it runs, by flake input rather than by the
  whole lockfile. `flake.lock` sat in the `rust` scope as a blanket stand-in,
  which was wrong in both directions: too coarse, since bumping a transitive
  input like `nixpkgs-lib` re-ran clippy and the full suite, and absent from
  `just machete` entirely until the fix below.

  Three probes replace it, one per flake input the dev shell draws binaries
  from — `qahq-tools`, `tola-tools`, `nixpkgs-tools` — each reading one node's
  `narHash` out of `flake.lock` with no subprocess. The blast radius of a
  `nix flake update` drops accordingly: bumping `tola` re-runs one gate,
  `qahq` two, where every one of them previously re-ran nine.

  The two spellings are not the same mechanism and look identical from
  `inputs:`. A tool that is its own flake input is pinned exactly. A tool out of
  nixpkgs has no node of its own, so `nixpkgs-tools` is a proxy that over-busts
  — still strictly finer than the whole file, because it does not move when
  qahq or tola do. The manifest says which is which where a reader meets it.

  `tests/gate_inputs_pin_the_toolchain.rs` (renamed from
  `gate_inputs_close_over_flake_lock.rs`) now resolves probes as well as scopes,
  and deliberately does not accept `rust-toolchain.toml` as satisfying it: nine
  of ten gates name `rust`, so accepting it would pass almost everything for
  free. Nothing here installs a compiler from that file — the dev shell does.

- `just machete` now busts when the dev shell moves.
 It was the one gate rule
  naming no toolchain pin — `[manifests, recipe-machete]`, where `manifests` is
  the two Cargo files — while `cargo-machete` itself comes from the dev shell.
  A `nix flake update` that changed which binary runs left its recorded pass
  looking fresh, which is the dangerous direction: a wrongly-*fresh* gate is a
  green build that proved nothing.

  It takes a new `toolchain` scope (`rust-toolchain.toml`, `flake.lock`) rather
  than having `flake.lock` folded into `manifests`, whose name means the Cargo
  manifests, or naming `rust`, which would drag in every source file
  `cargo-machete` never opens and destroy the property that made the original
  declaration right — a source-only edit still must not bust it.

  `tests/gate_inputs_close_over_flake_lock.rs` now asserts the general property
  for every `gate`-tagged rule, resolving each rule's `inputs` through the
  declared scopes via `mmz --dump-config=json` rather than re-parsing the
  fragments. `docs/contributing/gates.md` has claimed this under "Getting the
  scopes right" since it was written and nothing enforced it; `just machete` was
  the sole rule failing it.

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

## [0.7.0] - 2026-08-17

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
