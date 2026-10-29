#import "@local/mindtape:0.2.0": *
#import "/.mindtape/lib.typ": *

#show: task.with(
  title: "Check the task backlog in just check with mt from the dev shell",
  kind: "dev",
  priority: ice(
    impact: 5,
    confidence: 0.8,
    ease: 5,
  ),
  difficulty: 2,
  tags: ("tooling", "gating"),
  estimate: (src-files: 0),
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
      rules-not-memory: helps[`just check` and CI enforce the backlog's rules, not the host's own `mt`],
      cheap-delegation: neutral,
    ),
  ),
  assignee: "agent",
  status: done(2026, 10, 29),
)

== Problem

+ Run `command -v mt` in the dev shell, then read `just check`.
+ 🔴 `mt` resolves to whatever the host installed, since the dev shell has none, and no gate runs `mt check`, so CI never checks `tasks/` and the commit-msg hook runs a binary nobody pinned.


== TO-BE

- 🟢 The dev shell takes `mt` from qahq, pinned with the other gates.
- 🟢 `just check` runs `mt check` through mmz, so a broken task fails it locally and in CI:

  ```yaml
  commands:
    [{ name: just check-tasks, inputs: [tasks, recipe-check-tasks, qahq-tools], tags: [gate] }]
  ```


== Changes

- `flake.nix`: qahq's `mindtape` package in the dev shell.
- `flake.lock`: qahq bumped to the revision that ships it.
- `Justfile`: a `check-tasks` gate in `check`.
- `.mmz/conf.d/30-repo.yaml`: its rule, reading `tasks/`, `.mindtape/` and the qahq pin.


== Rollback

- Revert the commit. The hooks fall back to the host's `mt`.


== Out of scope

- The task rules themselves, which stay as `.mindtape/config.toml` sets them.


== Dogfood

Every `just check` here and in CI checks this backlog with the pinned `mt`.

