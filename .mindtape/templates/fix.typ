#import "@local/mindtape:0.2.0": *
#import "/.mindtape/lib.typ": *

#show: task.with(
  title: slot(
    "title",
    prompt: "Task title, imperative verb first",
    example: "Read a record with a deleted output as missing-output",
  ),
  kind: "fix",
  priority: ice(
    impact: slot("impact", kind: "int", default: 5),
    confidence: slot("confidence", kind: "float", default: 0.8),
    ease: slot("ease", kind: "int", default: 5),
  ),
  difficulty: slot("difficulty", kind: "int", default: 3),
  tags: slot("tags", kind: "list", default: ()),
  estimate: slot("estimate", kind: "text", default: "(src-files: 2)"),
  x-goals: (
    product: (
      no-false-green: slot(
        "no-false-green",
        kind: "raw",
        default: "neutral",
        example: "helps[a deleted artifact re-runs its rule instead of reading fresh]",
      ),
      exact-inputs: slot(
        "exact-inputs",
        kind: "raw",
        default: "neutral",
      ),
      cheap-gates: slot(
        "cheap-gates",
        kind: "raw",
        default: "neutral",
      ),
      agent-drivable: slot(
        "agent-drivable",
        kind: "raw",
        default: "neutral",
      ),
      composable-manifest: slot(
        "composable-manifest",
        kind: "raw",
        default: "neutral",
      ),
      ready-binaries: slot(
        "ready-binaries",
        kind: "raw",
        default: "neutral",
      ),
      docs-teach-judgement: slot(
        "docs-teach-judgement",
        kind: "raw",
        default: "neutral",
      ),
    ),
    process: (
      mmz-gates-mmz: slot(
        "mmz-gates-mmz",
        kind: "raw",
        default: "neutral",
      ),
      docs-read-facts: slot(
        "docs-read-facts",
        kind: "raw",
        default: "neutral",
      ),
      rules-not-memory: slot(
        "rules-not-memory",
        kind: "raw",
        default: "neutral",
      ),
      cheap-delegation: slot(
        "cheap-delegation",
        kind: "raw",
        default: "neutral",
      ),
    ),
  ),
)

== Problem

#slot(
  "problem",
  kind: "raw",
  prompt: "Numbered steps anyone can follow, commands in backticks; open the step that shows the defect with 🔴 and quote what it printed",
  example: ```typ
  + Declare `outputs: [dist/app]` on a rule, run it, then delete `dist/app`.
  + Run `mmz --status`.
  + 🔴 The row reads `fresh`, though the artifact the record promised is gone.
  ```,
)

== Environment

#slot(
  "environment",
  kind: "raw",
  prompt: "Bullets: the `mmz --version` or commit, and the platform, where the Problem steps reproduce",
  example: ```typ
  - `mmz` 0.13.0 built from `main` at the commit that filed this, on Linux.
  ```,
)

== TO-BE

#slot(
  "to-be",
  kind: "raw",
  prompt: "Bullets opening with 🟢: how it should work, behave and feel once this lands",
  example: ```typ
  - 🟢 The row reads `missing-output` and names `dist/app`, so the next run re-runs the rule.
  ```,
)

== Changes

#slot(
  "changes",
  kind: "raw",
  prompt: "One bullet per file: `path`, then what changes there; the red test first",
  example: ```typ
  - `tests/it/cli_outputs.rs`: the red test first, a deleted output reading `missing-output`.
  - `src/status_verdict.rs`: check declared outputs before calling a record fresh.
  ```,
)

== Out of scope

#slot(
  "out-of-scope",
  kind: "raw",
  prompt: "Bullets: what not to touch, and why",
  example: ```typ
  - The other states in `--status`. Each gets its own task once someone hits it.
  ```,
)

== Dogfood

#slot(
  "dogfood",
  kind: "raw",
  prompt: "How this repo will use the change, or Not applicable and why",
  example: ```typ
  Every `just check` arm in this repo reads its verdict through the changed code.
  ```,
)
