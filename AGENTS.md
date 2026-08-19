<!-- Generated from docs/src/agents.typ by `just docs md`. Do not edit; edit the source. -->

# mmz — AI Agent Context

How work is done lives in @CONTRIBUTING.md, pulled into your context whole by that reference; a process rule belongs there and is never restated here. This file carries the other half: what mmz _is_, and the handful of rules specific to working on it.

## What is this?

A memoized command runner. `mmz <command>` runs the command, or skips it when the inputs the matched rule declares are byte-for-byte unchanged since that command last succeeded. One question per invocation: is this rule’s work still done?

Repository: [https://github.com/mlavrinenko/mmz](https://github.com/mlavrinenko/mmz)

## Repo map

- `src/` — the library; `src/main.rs` is a thin argv-parsing wrapper over it
- `schema/` — the JSON Schemas for `.mmz/config.yaml` and `--status=json`
- `examples/demo/` — the docs fixture; `www/generate.sh` runs the real binary against a throwaway copy of it, so every transcript on the site is real output
- `www/content/*.typ` — the docs site (tola)
- `docs/src/*.typ` — the sources this file, README.md and CONTRIBUTING.md are rendered from
- `tasks/` — mmz’s own backlog as per-file MindTape task artifacts

Reference material lives on the docs site, not here. Before writing about a feature, read whichever page below already answers it rather than restating it — the page at `/<stem>/` is `www/content/<stem>.typ`.

| Topic | Read |
| --- | --- |
| The model, and the correctness contract | [Concepts](https://mlavrinenko.github.io/mmz/concepts/) |
| Scopes, globs, the gitignore filter, probes | [Inputs: scopes and probes](https://mlavrinenko.github.io/mmz/inputs/) |
| Rule matching, cache identity, parametric rules | [Matching and parametric rules](https://mlavrinenko.github.io/mmz/matching/) |
| Declared outputs and voided records | [Declared outputs](https://mlavrinenko.github.io/mmz/outputs/) |
| Tags and `--is-fresh` gating | [Gating with tags](https://mlavrinenko.github.io/mmz/gating/) |
| Composing one manifest from several files | [Composing a manifest from imports](https://mlavrinenko.github.io/mmz/composition/) |
| Every manifest key | [Manifest reference](https://mlavrinenko.github.io/mmz/manifest/) |
| Every action and exit code | [CLI reference](https://mlavrinenko.github.io/mmz/cli/) |
| Driving `mmz` as an agent | [For AI agents](https://mlavrinenko.github.io/mmz/agents/) |
| Which tool to reach for instead | [Comparison](https://mlavrinenko.github.io/mmz/comparison/) |

## Architecture

- Rust (edition 2024, MSRV 1.85, toolchain pinned in `rust-toolchain.toml`)
- One crate, library plus a thin binary. `src/main.rs` is excluded from coverage, so anything testable belongs in the library.
- `blake3` 1 for content digests; `globset` 0.4 and `ignore` 0.4 for scope resolution and the gitignore filter
- `serde` 1 with `serde_yaml_ng` 0.10 for the manifest and the cache records, `serde_json` 1 for `--status=json`
- `thiserror` 2 for the error enum the binary maps to exit codes

Versions above are read from `www/generated/crate-map.json`, never hand-written.

## Rules specific to this repo

- **mmz dogfoods itself.** `.mmz/config.yaml` declares the rules, and every `just check` arm runs through `just memo <gate>`, which is `mmz just <gate>`. So a no-op `just check` skips the work. Because the arms name RECIPES, adding or renaming a gate means adding or renaming its rule in `.mmz/config.yaml` — the rule name is `just <recipe>`.
- **Gate membership is derived and cross-checked.** A gate carries `[group("gate")]` AND appears in `check`’s dependency list. Tagging one without the other fails the docs build naming both sides, so wire both or neither.
- **Never hand-edit a generated Markdown file.** `README.md`, `AGENTS.md` (this file), `CONTRIBUTING.md` and `docs/contributing/*.md` are rendered from `docs/src/*.typ`. Edit the source, then `just docs md`. `just docs md-check` fails on drift.
- **Closing a task asserts the build passed.** `mt flip done` runs `mmz --is-fresh --tag gate` via `.mindtape/config.toml`. Run `just check` first to record a pass; `--force` waives it and records the waiver.
- **Tests:** inline `#[cfg(test)]` units beside the code, CLI and integration tests in `tests/` driving the real binary with `assert_cmd` and `predicates`. `just fix-check` auto-ejects inline tests from oversized files.
- **Be careful with context.** Omit non-essential command output; the gate runners are quiet by design.

## Where a change belongs

The seam that decides which layer a change goes in:

- **The manifest schema** (`schema/mmz.schema.json`, and the `--schema` output in `src/schema.rs`) declares what a manifest may say. A new key lands here first, or the docs cannot describe it — the manifest reference is generated from this schema.
- **The library** enforces what a manifest means: resolution, hashing, probe execution, freshness verdicts, record I/O.
- **`src/main.rs`** maps argv to a library call and a library error to an exit code. It holds no logic worth testing, and is excluded from coverage on that basis.

A new manifest key therefore touches the schema, the library type that parses it, `www/utils/config-notes.typ` for its prose, and a test. Missing the notes entry fails `just check-doc-facts` by name.
