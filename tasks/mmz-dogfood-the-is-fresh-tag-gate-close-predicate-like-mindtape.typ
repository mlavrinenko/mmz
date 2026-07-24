#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "mmz: dogfood the --is-fresh --tag gate close predicate like mindtape",
  priority: ice(
    impact: 6,
    confidence: 0.8,
    ease: 6,
  ),
  tags: ("config", "gating", "build"),
  status: proposed(2026, 7, 24),
)

== Summary

mmz dogfoods its own checks only weakly compared to how MindTape
(`~/projects/home/mindtape`) uses mmz, and the gap hides exactly the class of
bug this review found. Adopt MindTape's proven pattern in mmz's own repo:

- Per-gate scopes. Today every check rule in `.mmz/config.yaml` shares one
  broad `rust` scope, so any `.rs` edit busts test + clippy + fmt + machete
  alike. MindTape scopes each gate to what it actually reads: `cargo machete`
  on the manifests only, `cargo fmt` on `rust` + `rustfmt.toml`, `cargo clippy`
  on `rust` + `clippy.toml`. Split mmz's scopes the same way so a manifest-only
  or config-only edit re-runs just the affected gate.
- `gate` tags. Tag the check rules `tags: [gate]` so a subset is gateable.
- A close predicate. mmz uses MindTape for its own backlog but its
  `.mindtape/config.toml` has no flip hook. Add MindTape's `[[on.flip]]`
  action so closing a task asserts the build already passed:

  ```toml
  [[on.flip]]
  when = 'into("done") || into("cancelled") || into("failed")'
  actions = [
    { run = "mmz --is-fresh --tag gate", gate = true, message = "checks have not passed against this worktree — run `just check`, then flip again" },
  ]
  ```

- `gates-meta: [Justfile]` on each rule so a recipe-body edit re-runs the gate
  it wraps.

== Why

`--is-fresh` is mmz's headline feature and MindTape drives it as a real close
gate; mmz asserting nothing about its own build at task-close means the feature
goes un-exercised on its home turf. Wiring the same predicate makes mmz's own
task lifecycle depend on mmz working — the strongest dogfood. Had this been in
place, the `--is-fresh` parametric breakage would have shown up the first time a
gate rule fanned per-file.

Note: a parametric rule is NOT forced into the dogfood — mmz's gates have no
natural per-file command, so contriving one would be noise. The parametric
`--is-fresh` path stays covered by the regression tests added under
`mmz-is-fresh-ignores-parametric-rules`. This task is the honest, non-contrived
slice of MindTape's pattern.

== Scope

- `.mmz/config.yaml`: split scopes per gate, add `tags: [gate]`, add a
  `gates-meta` (Justfile) scope on each rule. Keep the `dev-docs` outdatty
  group's `.mmz/config.yaml` dependent re-confirmed.
- `.mindtape/config.toml`: add the `[[on.flip]]` close-gate action.
- Optionally switch the `check` recipe to `mmz just <subgate>` wrapping (as
  MindTape does) for uniform cache identity, and drop `chronic` in favour of
  mmz's own quiet `on_hit` note. Justfile is a `dev-docs` source, so reconcile
  CONTRIBUTING.md / AGENTS.md and re-confirm the lock if the recipe changes.
- Document the close gate in CONTRIBUTING.md / AGENTS.md.

== Home

mmz's own backlog. Raised in review: mmz's dogfooding is a weaker subset of
MindTape's, and closing that gap exercises the very path the review found broken.
Depends on the `--is-fresh` parametric fix landing first so the gated feature is
sound. Reference: `~/projects/home/mindtape/.mmz/config.yaml` and its
`.mindtape/config.toml` `[[on.flip]]` hook.
