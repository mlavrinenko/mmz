// docs/src/contributing.typ — source for the repo-root CONTRIBUTING.md.
// Rendered to Markdown by `docs/generate-md.sh` via typlite; see docs/src/lib.typ
// for what that render path can and cannot do.
//
// This file is the process document: how to run the gates, what each one is for,
// and what a commit owes. Every fact with a machine source is READ, not retyped
// — the gate table from `just --dump` (www/gates.jq), the caps from
// .linecop.yaml, the recipe names through the generated `just` module. The
// hand-written half is the reasoning, which has no machine source and belongs
// nowhere else.
//
// Material too long for this document lives in docs/contributing/*.md, each
// rendered from its own docs/src/contributing/*.typ source, and is linked from
// here rather than inlined.
#import "lib.typ": doc-title, fact, just, num-word, www-link
// The per-gate prose the docs site already requires for every gate (see
// .just/scripts/check-doc-facts.sh) — imported rather than duplicated, so one
// entry serves both the site and this document. gate-notes.typ is a bare dict
// with no tola imports, which is what lets this render path read it.
#import "/www/utils/gate-notes.typ": gate-notes

#metadata((
  output: "CONTRIBUTING.md",
)) <mmz-md>

#doc-title[Contributing to mmz]

= Setup

Prerequisites: #link("https://nixos.org/")[Nix] with flakes enabled. The dev
shell pins every tool the gates use, so a green run locally is a green run in CI.

```bash
direnv allow    # or: nix develop
just            # list every recipe
just check      # the full gate, memoized
```

= The gate

#let gates = fact("gates").gates

#just.check() runs #num-word(gates.len()) gates in parallel. Each arm goes
through #just.memo("<gate>"), which is `mmz just <gate>` — so a gate whose
declared inputs have not moved since it last passed is skipped rather than
re-run. #link("docs/contributing/gates.md")[docs/contributing/gates.md] covers
the memoization itself: what it buys, what it costs, and how to record a pass.

#table(
  columns: 3,
  table.header([Gate], [Command], [What it prevents]),
  ..gates
    .map(g => (
      raw(g.name),
      if g.command != none { raw(g.command) } else { g.doc },
      gate-notes.at(g.name),
    ))
    .flatten(),
)

Gate membership is not a list anyone maintains: a gate carries `[group("gate")]`
in the Justfile AND appears in `check`'s own dependency list, and the docs build
fails naming both sides when those two disagree. That dependency list is also
this table's row order, so reordering the table is an edit to the `check` line
rather than to any document.

Individual gates run on their own, unmemoized, when you want the output:

```bash
just clippy
just test
just docs::check
```

= Code style

Every clippy lint in `Cargo.toml`'s `[workspace.lints]` is `deny`, so the project
does not compile with a violation. The ones that shape the code most:

- No `unwrap()` or `expect()` — propagate with `?`.
- No `todo!()`, `unimplemented!()`, `unreachable!()` — handle the case.
- No `unsafe`, no wildcard imports, no single-character names.
- Bounded functions: `too_many_lines`, `cognitive_complexity` and
  `too_many_arguments` are all denied rather than warned.

Errors are a `thiserror` enum in `src/error.rs`. A new error case needs an exit
code in `src/main.rs`'s `exit_for` and an entry in
`www/utils/exit-code-notes.typ`, or #just.check-doc-facts() fails naming the
code.

= Project structure

Keep `src/main.rs` a thin entry point: argv parsing, logger init, and a call into
the library. `main.rs` is excluded from coverage, so anything that lands there is
untested by default — which is the argument for keeping it empty of logic, not
for lowering the bar.

#let caps = fact("linecop-caps")

File size is capped per language, and a cap is a design constraint rather than a
nag: a file nobody can hold in their head is where the untested branch hides.

#table(
  columns: 2,
  table.header([Language], [Cap]),
  ..caps
    .limits
    .keys()
    .map(k => (k, raw(str(caps.limits.at(k)) + " lines")))
    .flatten(),
)

// An override either raises a path's cap or exempts it entirely, and the two
// deserve different presentation: a different number is a budget decision, while
// an exemption is a claim that the metric does not apply to that file at all.
// Both are argued in place in .linecop.yaml; splitting them here keeps the table
// meaning one thing.
#let capped = caps.overrides.filter(o => o.at("limit", default: none) != none)
#let exempt = caps.overrides.filter(o => o.at("exclude", default: false))

#if capped.len() > 0 [
  Raised, each argued in place in `.linecop.yaml`:

  #table(
    columns: 2,
    table.header([Path], [Cap]),
    ..capped.map(o => (raw(o.pattern), raw(str(o.limit) + " lines"))).flatten(),
  )
]

#if exempt.len() > 0 [
  Exempt, because a line count says nothing useful about them — the generated
  Markdown (capped at its Typst source instead) and the files vendored verbatim
  from upstream:

  #list(..exempt.map(o => raw(o.pattern)))
]

As a Rust file approaches its cap, #just.eject() moves its inline
`#[cfg(test)]` module into a sibling `_tests.rs` file via
#link("https://github.com/mlavrinenko/ejectest")[ejectest], driven by
`linecop --baseline`. That keeps sources under the cap without giving up the
inline-test workflow. It runs as part of #just.fix-check().

= Testing

- Unit tests live inline in a `#[cfg(test)] mod tests` block next to
  the code they exercise.
- CLI and integration tests live in `tests/` and drive the built binary with
  #link("https://docs.rs/assert_cmd")[assert_cmd] and
  #link("https://docs.rs/predicates")[predicates].
- Every bug fix gets a regression test. The bug is evidence that the case was
  reachable; the test is what stops it being reachable twice.

Coverage is enforced separately from #just.check(), in CI and on demand:

```bash
just cover   # tarpaulin, fails under 70%
just crap    # CRAP metric, fails above 30 — needs the lcov `just cover` writes
```

#just.crap() exists because a global coverage threshold can stay green while one
branchy, untested function rots. When it flags a function, add tests or reduce
its branching — never raise the threshold to dodge it.

= Documentation

`README.md`, `AGENTS.md`, this file, and everything under `docs/contributing/`
are #strong[generated]. Editing one directly is wasted work: #just.docs-md-check()
fails on the drift, and the next #just.docs-md() overwrites it.

```bash
just docs md      # render docs/src/*.typ -> the Markdown each source declares
just docs serve   # the docs site, locally, with hot reload
just docs check   # build, index, and validate the site
```

#link(
  "docs/contributing/generated-docs.md",
)[docs/contributing/generated-docs.md]
covers the pipeline: which facts are derived from where, how to add a source, and
the typlite constraints a source has to write within.

The rule that matters: #strong[a doc states a fact by reading it]. The manifest
reference is generated from `mmz --schema`, the CLI reference from `mmz --help`,
every transcript from a real run against `examples/demo`, and the gate table
above from `just --dump`. If you find yourself typing a fact that already exists
in a file the build can read, derive it instead.

= Dependency drift

`outdatty.yaml` declares groups coupling `source` files to the `dependents` that
must stay in sync with them. #just.outdatty-check() (part of the gate) fails when
a source changed but its dependents were not re-confirmed.

After editing a source, review the listed dependents, update them as needed, then
run #just.outdatty-update() to record the new state into `outdatty.lock`, and
commit it. A recorded hash is a review watermark — it means a human looked, not
that a tool verified.

= Commits

- #link("https://www.conventionalcommits.org/")[Conventional Commits] for the
  subject line, in English, matching this repo's history.
- Add a `Refs: tasks/<stem>.typ` footer naming the MindTape task the commit
  closes or advances. File a task first if none exists; never guess a stem. Add
  no other trailers.

Task tracking is #link("https://github.com/mlavrinenko/mindtape")[MindTape]: one
Typst file per task under `tasks/`, ruled by `.mindtape/config.toml`. Drive it
with the `mt` CLI (`mt ls`, `mt add`, `mt flip`, `mt check`) rather than by
hand-editing task files. Not to be confused with `.mmz/`, which is mmz's own
command cache.

Closing a task runs `mmz --is-fresh --tag gate`, which passes only when every
gate-tagged rule last succeeded with its inputs unchanged. Run #just.check() to
record a pass, then flip; `--force` waives the gate and records the waiver in the
note. This is mmz dogfooding its headline feature on its own backlog — see
#www-link("/gating/").

= Submitting

1. #just.fix-check() — formats, applies clippy fixes, then runs the full gate.
2. If you touched a doc source, #just.docs-md() and commit the regenerated
  Markdown alongside it.
3. If you touched a coupled source, #just.outdatty-update() and commit the lock.
