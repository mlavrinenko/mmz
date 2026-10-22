// docs/src/contributing.typ — source for the repo-root CONTRIBUTING.md,
// rendered to Markdown by `docs/generate-md.sh` via typlite (see
// docs/src/lib.typ for what that render path can and cannot do).
//
// The process document: how to run the gates, what each is for, what a commit
// owes. Every fact with a machine source is READ, not retyped — the gate table
// from `just --dump` (www/gates.jq), the caps from .linecop.yaml, the recipe
// names through the generated `just` module. The hand-written half is the
// reasoning, which belongs nowhere else. Material too long for it lives in
// docs/contributing/*.md, each rendered from its own docs/src/contributing/*.typ
// source and linked from here rather than inlined.
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

Gate membership is derived: a gate carries `[group("gate")]` AND appears in
`check`'s dependency list, or the docs build fails naming both sides. That list
is also this table's row order, so reordering rows is an edit to the `check` line.

Run one gate on its own, unmemoized, for its output: `just clippy`,
`just test`, `just docs::check`.

= Code style

Every clippy lint in `Cargo.toml`'s `[workspace.lints]` is `deny`, so the project
does not compile with a violation. The ones that shape the code most:

- No `unwrap()` or `expect()` — propagate with `?`.
- No `todo!()`, `unimplemented!()`, `unreachable!()` — handle the case.
- No `unsafe`, no wildcard imports, no single-character names.
- Bounded functions: `too_many_lines`, `cognitive_complexity` and
  `too_many_arguments` are all denied rather than warned.

Errors are a `thiserror` enum in `src/error.rs`; a family of related cases goes
in a sub-enum beside the code that raises it (`ast::AstFailure`,
`probe::ProbeFailure`), since no text boundary splits an enum. A new case needs
an exit code in `src/main.rs`'s `exit_for` and an entry in
`www/utils/exit-code-notes.typ`, or #just.check-doc-facts() fails naming the code.

= Project structure

Keep `src/main.rs` a thin entry point: argv parsing, logger init, a call into the
library. It is excluded from coverage, so logic there would go untested.

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
// deserve different presentation: a different number is a budget decision, an
// exemption a claim that the metric does not apply to that file at all. Both are
// argued in place in .linecop.yaml; splitting them keeps each table meaning one thing.
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

As a Rust file nears its cap, #just.eject() (part of #just.fix-check()) moves
its inline `#[cfg(test)]` module into a sibling `_tests.rs` via
#link("https://github.com/mlavrinenko/ejectest")[ejectest], so sources stay
under the cap and keep their inline tests.

= Testing

- Unit tests live inline in a `#[cfg(test)] mod tests` block beside their code.
- CLI and integration tests live in `tests/it/` and drive the built binary with
  #link("https://docs.rs/assert_cmd")[assert_cmd] and
  #link("https://docs.rs/predicates")[predicates]. The suite is one binary: a
  new file is `tests/it/<name>.rs` plus a `mod <name>;` line in
  `tests/it/main.rs`.
- Every bug fix gets a regression test. The bug is evidence that the case was
  reachable; the test is what stops it being reachable twice.

Coverage is enforced separately from #just.check(), in CI and on demand, and so
is the grammar set:

```bash
just cover          # tarpaulin, fails under 70%
just crap           # CRAP metric, fails above 30 — needs the lcov `just cover` writes
just test-lang-all  # the suite with every tree-sitter grammar compiled in
```

#just.check() runs on default features, the Rust grammar alone, so
`ast_lang_tests.rs` proves one of twenty-eight languages parses.
#just.test-lang-all() covers the rest on its own CI job, since the release ships
a `lang-all` binary. It is no gate arm: twenty-seven C compiles do not belong in
the loop you rerun. Run it after touching `src/ast_lang.rs` or a `lang-*` feature.

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
covers the pipeline: which facts come from where, how to add a source, and what
typlite can render.

The rule that matters: #strong[a doc states a fact by reading it]. The manifest
reference is generated from `mmz --schema`, the CLI reference from `mmz --help`,
every transcript from a real run against a fixture under `examples/`, and the gate table
above from `just --dump`. If you find yourself typing a fact that already exists
in a file the build can read, derive it instead.

= Adding a manifest key

`Manifest` carries `deny_unknown_fields`, so the struct in `src/manifest.rs` is
the manifest's whole surface — and the JSON Schema, the reference page's prose
notes and the composition layer's policy-key handling are all derived from it
rather than discovering it. A key added in one place and nowhere else fails a
gate rather than shipping half-wired.

The step list lives on `Manifest`'s own doc comment, next to the field you are
adding, so it cannot drift away from the struct it describes.
#link("docs/contributing/manifest-keys.md")[docs/contributing/manifest-keys.md]
covers why each coupling exists, which of them will catch you, and what makes a
key a root-only policy key.

= Dependency drift

`outdatty.yaml` declares groups coupling `source` files to the `dependents` that
must stay in sync with them. #just.outdatty-check() (part of the gate) fails when
a source changed but its dependents were not re-confirmed.

After editing a source, review the listed dependents, update them as needed, run
#just.outdatty-update() and commit `outdatty.lock`. A recorded hash is a review
watermark: a human looked, no tool verified.

= Worktrees

`just fork <slug>` makes a sibling worktree, `just land` merges it back; see
#link("docs/contributing/worktrees.md")[docs/contributing/worktrees.md].

= Commits

- #link("https://www.conventionalcommits.org/")[Conventional Commits] for the
  subject line, in English, matching this repo's history.
- A `Refs: <stem>` footer names the task the commit advances, by bare stem.
  File a task first if none exists; never guess a stem.
- A change to `src/` or `schema/` adds an entry under `[Unreleased]` in
  `CHANGELOG.md`, or a `Changelog: none` trailer. Add no other trailers.

`git commit` enforces these: `.githooks/commit-msg` runs `mt check --commit-msg`,
and `.githooks/pre-commit` refuses a tree #just.check() has not passed or an
unstaged `outdatty.lock`. The dev shell installs them on entry, through
#just.install-hooks().

Tasks live under `tasks/`, driven by `mt`; their shape and flip flow are in
#link("docs/contributing/tasks.md")[docs/contributing/tasks.md], and how a close
gates on mmz itself in #www-link("/gating/").

= Submitting

1. #just.fix-check() — formats, applies clippy fixes, then runs the full gate.
2. If you touched a doc source, #just.docs-md() and commit the regenerated
  Markdown alongside it.
3. If you touched a coupled source, #just.outdatty-update() and commit the lock.
