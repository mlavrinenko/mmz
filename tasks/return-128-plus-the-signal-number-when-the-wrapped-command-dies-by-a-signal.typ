#import "@local/mindtape:0.2.0": *
#import "/.mindtape/lib.typ": *

#show: task.with(
  title: "Return 128 plus the signal number when the wrapped command dies by a signal",
  kind: "fix",
  priority: ice(
    impact: 4,
    confidence: 0.9,
    ease: 9,
  ),
  difficulty: 1,
  tags: ("cli",),
  estimate: (src-files: 2),
  x-goals: (
    product: (
      no-false-green: neutral,
      exact-inputs: neutral,
      cheap-gates: neutral,
      agent-drivable: helps[a caller tells an OOM kill (137) or a Ctrl-C (130) from a failing test (1) by the exit code alone],
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
  status: done(2026, 10, 23),
)

== Problem

+ Declare a rule `sh die.sh` over any scope, with `die.sh` holding `kill -KILL $$`.
+ Run `sh die.sh`; it exits 137.
+ Run `mmz sh die.sh`.
+ 🔴 It exits 1. `exit_code` in `src/engine.rs` maps every signal death to 1, so an OOM kill reads as a failing test.


== Environment

- `mmz` 0.13.0 from `target/release/mmz`, on Linux.


== TO-BE

- 🟢 `mmz <cmd>` exits 128 plus N when the command dies by signal N, as a shell does: 137 for SIGKILL, 130 for SIGINT, 143 for SIGTERM.
- 🟢 The record still says `failed`, so the next run re-runs it.
- 🟢 `mmz --help` says so beside "the wrapped command's own exit code".


== Changes

- `tests/it/cli.rs`: the red test first, a command killing itself with SIGKILL exiting 137 through mmz.
- `src/engine.rs`: read `ExitStatusExt::signal()` under `cfg(unix)` in `exit_code`, and map signal N to 128 plus N.
- `src/main.rs`: name the 128 plus N case in the help's exit-code list.


== Out of scope

- Forwarding a signal mmz itself receives to the child. That is its own task, since it needs a dependency.
- Windows, where an exit status always carries a code.


== Dogfood

A gate arm of this repo's `just check` that the OOM killer stops now reads 137, not 1, so the failure names its cause.

