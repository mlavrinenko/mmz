// How this repo builds mmz. Kept apart from goals.typ, which says what mmz is
// for. Every open task gives each goal here a verdict too, in its
// `x-goals: (process: ...)` field, checked by `check-goals(...)` from
// .mindtape/lib.typ.

#let goals = (
  mmz-gates-mmz: (
    title: "mmz gates its own repo",
    acid-test: [Every `just check` arm runs as `mmz just <gate>` under a rule
      declaring its recipe body and the flake input its tools come from, a
      no-op `just check` skips every arm, and `mt flip done` refuses unless
      `mmz --is-fresh --tag gate` passes.],
  ),
  docs-read-facts: (
    title: "Docs state facts by reading them",
    acid-test: [Every transcript, flag, manifest key, gate row, version and size
      figure in README.md or www/ is captured from the built binary or a source
      file, and drift fails `just check`.],
  ),
  rules-not-memory: (
    title: "Repo rules are enforced, not remembered",
    acid-test: [A commit without its `Refs:` stem, a file past its size cap, or
      a gate wired on one side only is refused by a hook or gate before it
      lands; no rule depends on someone having read CONTRIBUTING.],
  ),
  cheap-delegation: (
    title: "Delegated work costs little",
    acid-test: [An agent forks with `just fork` and lands with `just land`, a
      passing gate costs it one line of output, and nothing it needs lives
      outside the repo.],
  ),
)
