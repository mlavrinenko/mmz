#import "@local/mindtape:0.2.0": *
#import "/.mindtape/lib.typ": *

#show: task.with(
  title: "Hold open tasks to the mindtape-tasks practice",
  kind: "dev",
  tags: ("tooling",),
  priority: framework("ice", confidence: 0.9, ease: 6.0, impact: 6.0),
  difficulty: 2,
  estimate: (src-files: 11),
  assignee: "agent",
  x-goals: (
    product: (
      no-false-green: neutral,
      exact-inputs: neutral,
      cheap-gates: neutral,
      agent-drivable: neutral,
      composable-manifest: neutral,
      ready-binaries: neutral,
      docs-teach-judgement: neutral,
    ),
    process: (
      mmz-gates-mmz: neutral,
      docs-read-facts: neutral,
      rules-not-memory: helps[the task shape and the flip flow are held by `mt check` and the commit hook, not remembered from CONTRIBUTING],
      cheap-delegation: helps[an agent handed one task file gets the steps, the files to touch and what done looks like],
    ),
  ),
  status: done(2026, 10, 23),
)

== Problem

+ Run `mt add "mmz: anything"` with a one-line `Summary` body.
+ Run `mt check`.
+ 🔴 It passes. A task carries no kind, difficulty, assignee or goal verdict, and its close can land in a commit apart from its work.

== TO-BE

- 🟢 `.mindtape/config.toml` extends `mindtape-tasks`, so an open task is Problem, TO-BE, Changes and Out of scope, plus its kind's section, then Dogfood.
- 🟢 Every open task gives each goal in `.mindtape/goals.typ` and `.mindtape/process-goals.typ` a verdict, and one that helps none is refused.
- 🟢 `mt add -t <template>` files any kind with mmz examples, and `CONTRIBUTING.md` says how.
- 🟢 `mt wip` and `mt done` ride with the work, so the history stops carrying `chore(tasks): close …` commits.

== Changes

- `.mindtape/config.toml`: extend the pack; add kind, difficulty, a `src-files` estimate with a judgment cap of 8, assignee, WIP caps, the Dogfood rules, the changelog and tweak commit rules, views and the stale gate.
- `.mindtape/lib.typ`, `.mindtape/goals.typ`, `.mindtape/process-goals.typ`: `check-goals` and the goals read off this repo's history.
- `.mindtape/templates/*.typ`: one template per kind, plus `tweak` and `finding`.
- `docs/src/contributing.typ`: the task shape, the flip flow and the `Changelog: none` trailer.
- `tasks/mmz-the-release-never-runs-its-aarch64-linux-binary.typ`: rewritten into the new shape, and back to `accepted` while it waits on a release.

== Rollback

- Revert the commit. Closed tasks keep their `Summary` bodies, since every new rule exempts a terminal task.

== Out of scope

- Rewriting closed tasks. The rules apply to open ones only.
- A sweep that files tasks from linecop or coverage findings. This repo has no recipe for one.

== Dogfood

This task is the first one closed under the rules it adds: its `mt done` rides in the commit that lands them.
