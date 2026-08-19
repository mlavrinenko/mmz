// docs/src/agents.typ — source for the repo-root AGENTS.md. Rendered to
// Markdown by `docs/generate-md.sh` via typlite; see docs/src/lib.typ for what
// that render path can and cannot do.
//
// Facts this file used to hand-list are read from www/generated/ instead: the
// crate version and its dependency versions from crate-map.json, the recipe
// names through the generated `just` module (so a rename fails the render rather
// than rotting in prose).
//
// Every docs-site pointer goes through `www-link` (docs/src/lib.typ), which
// resolves the route against the generated page manifest and panics on an
// unknown one. A hand-typed site URL here would rot unnoticed — nothing in this
// repo resolves a URL sitting in generated Markdown.
//
// This file is on the always-loaded path, so its budget is a real cost, not an
// aesthetic one: prose that motivates a rule an agent follows anyway, a rule
// stated twice, and a pointer that restates the page it points at are all
// deleted on sight. Every rule, command shape and field name stays.
#import "lib.typ": doc-title, fact, just, www-link

#metadata((
  output: "AGENTS.md",
)) <mmz-md>

// The root package's own manifest facts, read from the generated crate map
// rather than retyped: the repository URL below and the versions under
// Architecture are all Cargo.toml's to state, and a fork or a move should not
// need a human to find the copy that lied about it.
#let pkg = fact("crate-map").package

#doc-title[mmz — AI Agent Context]

How work is done lives in \@CONTRIBUTING.md, pulled into your context whole by
that reference; a process rule belongs there and is never restated here. This
file carries the other half: what mmz _is_, and the handful of rules specific to
working on it.

= What is this?

A memoized command runner. `mmz <command>` runs the command, or skips it when the
inputs the matched rule declares are byte-for-byte unchanged since that command
last succeeded. One question per invocation: is this rule's work still done?

Repository: #link(pkg.repository)

= Repo map

- `src/` — the library; `src/main.rs` is a thin argv-parsing wrapper over it
- `schema/` — the JSON Schemas for `.mmz/config.yaml` and `--status=json`
- `examples/demo/` — the docs fixture; `www/generate.sh` runs the real binary
  against a throwaway copy of it, so every transcript on the site is real output
- `www/content/*.typ` — the docs site (tola)
- `docs/src/*.typ` — the sources this file, README.md and CONTRIBUTING.md are
  rendered from
- `tasks/` — mmz's own backlog as per-file MindTape task artifacts

Reference material lives on the docs site, not here. Before writing about a
feature, read whichever page below already answers it rather than restating it —
the page at `/<stem>/` is `www/content/<stem>.typ`.

#table(
  columns: 2,
  table.header([Topic], [Read]),
  [The model, and the correctness contract], www-link("/concepts/"),
  [Scopes, globs, the gitignore filter, probes], www-link("/inputs/"),
  [Rule matching, cache identity, parametric rules], www-link("/matching/"),
  [Declared outputs and voided records], www-link("/outputs/"),
  [Tags and `--is-fresh` gating], www-link("/gating/"),
  [Composing one manifest from several files], www-link("/composition/"),
  [Every manifest key], www-link("/manifest/"),
  [Every action and exit code], www-link("/cli/"),
  [Driving `mmz` as an agent], www-link("/agents/"),
  [Which tool to reach for instead], www-link("/comparison/"),
)

= Architecture

#let deps = fact("crate-map").deps
#let major(v) = {
  let parts = v.split(".")
  if parts.at(0) == "0" { "0." + parts.at(1) } else { parts.at(0) }
}

- Rust (edition #pkg.edition, MSRV #pkg.rust_version, toolchain pinned in
  `rust-toolchain.toml`)
- One crate, library plus a thin binary. `src/main.rs` is excluded from coverage,
  so anything testable belongs in the library.
- `blake3` #major(deps.blake3) for content digests; `globset` #major(deps.globset)
  and `ignore` #major(deps.ignore) for scope resolution and the gitignore filter
- `serde` #major(deps.serde) with `serde_yaml_ng` #major(deps.serde_yaml_ng) for
  the manifest and the cache records, `serde_json` #major(deps.serde_json) for
  `--status=json`
- `thiserror` #major(deps.thiserror) for the error enum the binary maps to exit
  codes

Versions above are read from `www/generated/crate-map.json`, never hand-written.

= Rules specific to this repo

- #strong[mmz dogfoods itself.] `.mmz/config.yaml` declares the rules, and every
  #just.check() arm runs through #just.memo("<gate>"), which is
  `mmz just <gate>`. So a no-op #just.check() skips the work. Because the arms
  name RECIPES, adding or renaming a gate means adding or renaming its rule in
  `.mmz/config.yaml` — the rule name is `just <recipe>`.
- #strong[Gate membership is derived and cross-checked.] A gate carries
  `[group("gate")]` AND appears in `check`'s dependency list. Tagging one without
  the other fails the docs build naming both sides, so wire both or neither.
- #strong[Never hand-edit a generated Markdown file.] `README.md`, `AGENTS.md`
  (this file), `CONTRIBUTING.md` and `docs/contributing/*.md` are rendered from
  `docs/src/*.typ`. Edit the source, then #just.docs-md(). #just.docs-md-check()
  fails on drift.
- #strong[Closing a task asserts the build passed.] `mt flip done` runs
  `mmz --is-fresh --tag gate` via `.mindtape/config.toml`. Run #just.check()
  first to record a pass; `--force` waives it and records the waiver.
- #strong[Tests:] inline `#[cfg(test)]` units beside the code, CLI and
  integration tests in `tests/` driving the real binary with `assert_cmd` and
  `predicates`. #just.fix-check() auto-ejects inline tests from oversized files.
- #strong[Be careful with context.] Omit non-essential command output; the gate
  runners are quiet by design.

= Where a change belongs

The seam that decides which layer a change goes in:

- #strong[The manifest schema] (`schema/mmz.schema.json`, and the `--schema`
  output in `src/schema.rs`) declares what a manifest may say. A new key lands
  here first, or the docs cannot describe it — the manifest reference is
  generated from this schema.
- #strong[The library] enforces what a manifest means: resolution, hashing, probe
  execution, freshness verdicts, record I/O.
- #strong[`src/main.rs`] maps argv to a library call and a library error to an
  exit code. It holds no logic worth testing, and is excluded from coverage on
  that basis.

A new manifest key therefore touches the schema, the library type that parses it,
`www/utils/config-notes.typ` for its prose, and a test. Missing the notes entry
fails #just.check-doc-facts() by name.
