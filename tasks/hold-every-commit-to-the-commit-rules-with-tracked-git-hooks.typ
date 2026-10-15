#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "Hold every commit to the commit rules with tracked git hooks",
  tags: ("tooling", "gating"),
  priority: framework("ice", confidence: 0.9, ease: 7.0, impact: 6.0),
  status: proposed(2026, 10, 11),
)

== Summary

mmz runs no git hook. CONTRIBUTING said to write `Refs: tasks/<stem>.typ`, which `mt check --commits` refuses as a path ("is a path; write the stem"), and nothing caught it until a range check on 2026-10-11: 8ff3966, 193cb81 and 8062520 carry that form on main, and the last two are pushed. 2fae1cc fixed the docs, but the next wrong footer still lands silently.

canibump holds every commit at commit time: `.githooks/commit-msg` runs `mt check --commit-msg`, `.githooks/pre-commit` asserts the gate, `just install-hooks` sets `core.hooksPath = .githooks`, and its flake's shellHook runs that recipe quietly, so a fresh clone with direnv gets the hooks without a step to remember. Copy that shape here.

Done when a commit with `Refs: tasks/x.typ` is refused at `git commit` in a fresh clone, and CONTRIBUTING's commit section names the hook.

