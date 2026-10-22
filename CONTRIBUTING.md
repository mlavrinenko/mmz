<!-- Generated from docs/src/contributing.typ by `just docs md`. Do not edit; edit the source. -->

# Contributing to mmz

## Setup

Prerequisites: [Nix](https://nixos.org/) with flakes enabled. The dev shell pins every tool the gates use, so a green run locally is a green run in CI.

```bash
direnv allow    # or: nix develop
just            # list every recipe
just check      # the full gate, memoized
```

## The gate

`just check` runs 11 gates in parallel. Each arm goes through `just memo <gate>`, which is `mmz just <gate>` — so a gate whose declared inputs have not moved since it last passed is skipped rather than re-run. [docs/contributing/gates.md](docs/contributing/gates.md) covers the memoization itself: what it buys, what it costs, and how to record a pass.

| Gate | Command | What it prevents |
| --- | --- | --- |
| `fmt-check` | Check formatting without writing changes (CI-friendly) | Formatting is settled by a tool, not in review. Covers Rust, the Typst docs sources, and the Justfiles — one gate, so a formatted-by-one-tool tree cannot pass while another tool’s files drift. |
| `clippy` | `cargo clippy --workspace --all-targets -q -- -D warnings` | Every lint in `Cargo.toml`’s `[workspace.lints]` is `deny`, so this is not a style pass: it is the compile-time half of the correctness contract, and it fails the build rather than warning into a log nobody reads. |
| `test` | `cargo test --workspace "$@"` | The whole suite — inline unit tests plus the CLI integration tests that drive the real binary through `assert_cmd`. |
| `machete` | `cargo machete` | A dependency nobody imports is still a dependency somebody audits, builds and ships. Catches the ones a refactor orphaned. |
| `check-file-size` | `linecop` | Caps every file at the limit `.linecop.yaml` sets for its language. The point is not tidiness: a file nobody can hold in their head is where the untested branch hides. |
| `outdatty-check` | `outdatty check` | Fails when a source changed and the dependents `outdatty.yaml` couples to it were not re-confirmed. It cannot check that a doc is _correct_ — only that a human looked since the code moved. |
| `check-changelog-history` | Fail if a released changelog section drifted from its tag | Fails when a `## [x.y.z]` section stops saying what its `vx.y.z` tag shipped. A released section is a historical record, and the edit that swallowed one here went unnoticed for thirteen commits. A deliberate rewrite is recorded in `CHANGELOG.waivers` by `just changelog-waive`. |
| `check-doc-coverage` | Fail if a CLI action has no hand-written note | Fails when `mmz --help` advertises an action with no hand-written note, or a note names an action the binary no longer has. The list is parsed out of the binary, so it cannot be satisfied by editing a list. |
| `check-doc-facts` | Fail if a derived doc fact has no hand-written prose | The same set-difference over the derived facts: a manifest key with no prose, a gate with no prose, a page unreachable from the sidebar, a sidebar entry pointing at no page. |
| `docs-check` | `tola build && pagefind --site public/mmz --silent && tola validate` | Builds the docs site and validates every internal link and asset reference. A cross-page link is a string until something resolves it; this is the something. |
| `docs-md-check` | Fail if generated Markdown has drifted from docs/src | Fails when a committed `README.md`, `AGENTS.md`, `CONTRIBUTING.md` or `docs/contributing/*.md` has drifted from the `docs/src/*.typ` source it is rendered from — which is what a hand-edit of a generated file looks like. The regenerate goes to a temp directory precisely so a hand-corrupted committed file cannot be healed by the check that is supposed to catch it. |

Gate membership is derived: a gate carries `[group("gate")]` AND appears in `check`’s dependency list, or the docs build fails naming both sides. That list is also this table’s row order, so reordering rows is an edit to the `check` line.

Individual gates run on their own, unmemoized, when you want the output:

```bash
just clippy
just test
just docs::check
```

## Code style

Every clippy lint in `Cargo.toml`’s `[workspace.lints]` is `deny`, so the project does not compile with a violation. The ones that shape the code most:

- No `unwrap()` or `expect()` — propagate with `?`.
- No `todo!()`, `unimplemented!()`, `unreachable!()` — handle the case.
- No `unsafe`, no wildcard imports, no single-character names.
- Bounded functions: `too_many_lines`, `cognitive_complexity` and `too_many_arguments` are all denied rather than warned.

Errors are a `thiserror` enum in `src/error.rs`; a family of related cases goes in a sub-enum beside the code that raises it (`ast::AstFailure`, `probe::ProbeFailure`), since no text boundary splits an enum. A new case needs an exit code in `src/main.rs`’s `exit_for` and an entry in `www/utils/exit-code-notes.typ`, or `just check-doc-facts` fails naming the code.

## Project structure

Keep `src/main.rs` a thin entry point: argv parsing, logger init, and a call into the library. It is excluded from coverage, so logic there goes untested; keep the logic out rather than lower the bar.

File size is capped per language, and a cap is a design constraint rather than a nag: a file nobody can hold in their head is where the untested branch hides.

| Language | Cap |
| --- | --- |
| Rust | `500 lines` |
| Markdown | `200 lines` |
| Typst | `250 lines` |
| Shell | `200 lines` |
| jq | `150 lines` |
| CSS | `250 lines` |

Raised, each argued in place in `.linecop.yaml`:

| Path | Cap |
| --- | --- |
| `./CHANGELOG.md` | `1000 lines` |

Exempt, because a line count says nothing useful about them — the generated Markdown (capped at its Typst source instead) and the files vendored verbatim from upstream:

- `./README.md`
- `./AGENTS.md`
- `./CONTRIBUTING.md`
- `./docs/contributing/*.md`
- `./www/templates/tola.typ`
- `./www/utils/tola.typ`

As a Rust file nears its cap, `just eject` (part of `just fix-check`) moves its inline `#[cfg(test)]` module into a sibling `_tests.rs` via [ejectest](https://github.com/mlavrinenko/ejectest), so sources stay under the cap and keep their inline tests.

## Testing

- Unit tests live inline in a `#[cfg(test)] mod tests` block beside their code.
- CLI and integration tests live in `tests/it/` and drive the built binary with [assert\_cmd](https://docs.rs/assert_cmd) and [predicates](https://docs.rs/predicates). The suite is one binary: a new file is `tests/it/<name>.rs` plus a `mod <name>;` line in `tests/it/main.rs`.
- Every bug fix gets a regression test. The bug is evidence that the case was reachable; the test is what stops it being reachable twice.

Coverage is enforced separately from `just check`, in CI and on demand, and so is the grammar set:

```bash
just cover          # tarpaulin, fails under 70%
just crap           # CRAP metric, fails above 30 — needs the lcov `just cover` writes
just test-lang-all  # the suite with every tree-sitter grammar compiled in
```

`just check` runs on default features, the Rust grammar alone, so `ast_lang_tests.rs` proves one of twenty-eight languages parses. `just test-lang-all` covers the rest on its own CI job, since the release ships a `lang-all` binary. It is no gate arm: twenty-seven C compiles do not belong in the loop you rerun. Run it after touching `src/ast_lang.rs` or a `lang-*` feature.

`just crap` exists because a global coverage threshold can stay green while one branchy, untested function rots. When it flags a function, add tests or reduce its branching — never raise the threshold to dodge it.

## Documentation

`README.md`, `AGENTS.md`, this file, and everything under `docs/contributing/` are **generated**. Editing one directly is wasted work: `just docs md-check` fails on the drift, and the next `just docs md` overwrites it.

```bash
just docs md      # render docs/src/*.typ -> the Markdown each source declares
just docs serve   # the docs site, locally, with hot reload
just docs check   # build, index, and validate the site
```

[docs/contributing/generated-docs.md](docs/contributing/generated-docs.md) covers the pipeline: which facts are derived from where, how to add a source, and the typlite constraints a source has to write within.

The rule that matters: **a doc states a fact by reading it**. The manifest reference is generated from `mmz --schema`, the CLI reference from `mmz --help`, every transcript from a real run against `examples/demo`, and the gate table above from `just --dump`. If you find yourself typing a fact that already exists in a file the build can read, derive it instead.

## Adding a manifest key

`Manifest` carries `deny_unknown_fields`, so the struct in `src/manifest.rs` is the manifest’s whole surface — and the JSON Schema, the reference page’s prose notes and the composition layer’s policy-key handling are all derived from it rather than discovering it. A key added in one place and nowhere else fails a gate rather than shipping half-wired.

The step list lives on `Manifest`’s own doc comment, next to the field you are adding, so it cannot drift away from the struct it describes. [docs/contributing/manifest-keys.md](docs/contributing/manifest-keys.md) covers why each coupling exists, which of them will catch you, and what makes a key a root-only policy key.

## Dependency drift

`outdatty.yaml` declares groups coupling `source` files to the `dependents` that must stay in sync with them. `just outdatty-check` (part of the gate) fails when a source changed but its dependents were not re-confirmed.

After editing a source, review the listed dependents, update them as needed, run `just outdatty-update` and commit `outdatty.lock`. A recorded hash is a review watermark: a human looked, no tool verified.

## Worktrees

`just fork <slug>` makes a sibling worktree, `just land` merges it back; see [docs/contributing/worktrees.md](docs/contributing/worktrees.md).

## Commits

- [Conventional Commits](https://www.conventionalcommits.org/) for the subject line, in English, matching this repo’s history.
- Add a `Refs: <stem>` footer naming the MindTape task the commit closes or advances, the bare stem: `mt check --commits` refuses a path. File a task first if none exists; never guess a stem. Add no other trailers.

`git commit` enforces both: `.githooks/commit-msg` runs `mt check --commit-msg`, and `.githooks/pre-commit` refuses a tree `just check` has not passed or an unstaged `outdatty.lock`. The dev shell installs them on entry, through `just install-hooks`.

Task tracking is [MindTape](https://github.com/mlavrinenko/mindtape): one Typst file per task under `tasks/`, ruled by `.mindtape/config.toml` and driven by the `mt` CLI (`mt ls`, `mt add`, `mt flip`, `mt check`), never by hand. `.mmz/` is unrelated: it is mmz’s own command cache.

Closing a task runs `mmz --is-fresh --tag gate` too, so run `just check` to record a pass, then flip. `--force` waives the gate and records the waiver in the note. That is mmz dogfooding its headline feature on its own backlog, see [Gating with tags](https://mlavrinenko.github.io/mmz/gating/).

## Submitting

1. `just fix-check` — formats, applies clippy fixes, then runs the full gate.
2. If you touched a doc source, `just docs md` and commit the regenerated Markdown alongside it.
3. If you touched a coupled source, `just outdatty-update` and commit the lock.
