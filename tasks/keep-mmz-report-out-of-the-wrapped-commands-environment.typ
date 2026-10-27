#import "@local/mindtape:0.2.0": *
#import "/.mindtape/lib.typ": *

#show: task.with(
  title: "Keep MMZ_REPORT out of the wrapped command's environment",
  kind: "fix",
  priority: ice(
    impact: 4,
    confidence: 0.9,
    ease: 8,
  ),
  difficulty: 2,
  tags: ("cli",),
  estimate: (src-files: 1),
  x-goals: (
    product: (
      no-false-green: helps[an outer run that ran is never reported as skipped because a nested mmz skipped],
      exact-inputs: neutral,
      cheap-gates: neutral,
      agent-drivable: helps[a script reading `MMZ_REPORT` gets the answer for the run it started, however deep the command nests mmz],
      composable-manifest: neutral,
      ready-binaries: neutral,
      docs-teach-judgement: neutral,
    ),
    process: (
      mmz-gates-mmz: neutral,
      docs-read-facts: neutral,
      rules-not-memory: neutral,
      cheap-delegation: neutral,
    ),
  ),
  assignee: "agent",
  status: proposed(2026, 10, 28),
)

== Problem

+ Declare rules `sh` and `true` over `*.txt`, and run `mmz true` once to record a pass.
+ Run `MMZ_REPORT=report mmz sh -c 'mmz true'` on a fresh `sh` record.
+ 🔴 `report` reads `skipped`, though the outer `sh` ran. The wrapped command inherits `MMZ_REPORT`, so the nested mmz overwrites the outer run's `ran` with its own decision.


== Environment

- mmz 0.15.0, Linux.


== TO-BE

- 🟢 The wrapped command runs without `MMZ_REPORT` in its environment, so `report` holds the decision of the run the caller started.


== Changes

- `tests/it/cli_report.rs`: the red test first, the Problem's steps asserting `ran`, and a command that echoes `MMZ_REPORT` seeing it unset.
- `src/engine.rs`: remove `MMZ_REPORT` from the spawned command's environment.
- `www/content/cli.typ`, `CHANGELOG.md`: say the command does not inherit it.


== Out of scope

- `MMZ_DISABLE` and `MMZ_NOW`, which a nested mmz should keep inheriting: a forced or pinned outer run means the same for the runs inside it.


== Dogfood

Not applicable: no recipe here sets `MMZ_REPORT`. Mindtape's `just check` arms nest `mmz just docs::gen`, which is the caller this fixes.

