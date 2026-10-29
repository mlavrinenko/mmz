#import "@local/mindtape:0.2.0": *
#import "/.mindtape/lib.typ": *

#show: task.with(
  title: "Decide which files outdatty must see in a group",
  kind: "dev",
  priority: ice(
    impact: 5,
    confidence: 0.8,
    ease: 5,
  ),
  difficulty: 3,
  tags: ("tooling", "gating"),
  estimate: (src-files: 0),
  x-goals: (
    product: (
      no-false-green: helps[a new doc or source that restates another fails until a group couples it],
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
      rules-not-memory: neutral,
      cheap-delegation: neutral,
    ),
  ),
  assignee: "agent",
  status: proposed(2026, 10, 29),
)

== Problem

+ Delete `require_tracked` from `outdatty.yaml` and run `just outdatty-check` on outdatty 0.10.
+ 🔴 It fails on 247 files no group lists, 28 of them under `tests/it/`. mmz ran outdatty 0.3 until qahq 0.9.0, so the audit never ran here, and the manifest opts out with `require_tracked: ["!**"]`.


== TO-BE

- 🟢 `outdatty.yaml` names the files that must sit in a group, so a new page or source that restates another fails until someone couples it:

  ```yaml
  require_tracked: ["src/**", "docs/src/**", "www/content/**", "Justfile", "flake.nix"]
  ```


== Changes

- `outdatty.yaml`: a `require_tracked` list, and groups for the files it newly demands.
- `outdatty.lock`: their hashes.


== Rollback

- Restore `require_tracked: ["!**"]`.


== Out of scope

- Generated files and fixtures, which no human edits and no group should claim.


== Dogfood

`just check` here runs the audit on every commit.

