#import "@local/mindtape:0.2.0": *
#import "/.mindtape/lib.typ": *

#show: task.with(
  title: "End the signal wait loop when the command exits, not on SIGCHLD",
  kind: "fix",
  priority: ice(
    impact: 6,
    confidence: 0.8,
    ease: 6,
  ),
  difficulty: 5,
  tags: ("cli",),
  estimate: (src-files: 1),
  x-goals: (
    product: (
      no-false-green: neutral,
      exact-inputs: neutral,
      cheap-gates: neutral,
      agent-drivable: helps[a wrapped run under a tracer that drops SIGCHLD ends when its command ends, instead of hanging forever],
      composable-manifest: neutral,
      ready-binaries: neutral,
      docs-teach-judgement: neutral,
    ),
    process: (
      mmz-gates-mmz: helps[CI's coverage job, which runs this repo's suite under ptrace, finishes again],
      docs-read-facts: neutral,
      rules-not-memory: neutral,
      cheap-delegation: neutral,
    ),
  ),
  assignee: "agent",
  status: proposed(2026, 10, 23),
)

== Problem

+ Run `cargo tarpaulin --workspace --skip-clean --test it -- cli_signals` on `main` after `6350199`.
+ Wait ten minutes.
+ 🔴 It never finishes. The test thread sits in `unix_stream_read_generic` inside `Signals::wait`, beside its child `sh` already a zombie. tarpaulin's ptrace swallowed the SIGCHLD, and `Guard::wait` in `src/signals.rs` only re-checks the child when a signal wakes it. CI run 39863026837's Coverage job hung the same way and was cancelled after an hour.


== Environment

- `mmz` 0.14.0 from `main`, Linux, cargo-tarpaulin-tarpaulin 0.35.4 (ptrace engine).


== TO-BE

- 🟢 The wait loop ends when the command exits, whether or not a SIGCHLD reaches mmz.
- 🟢 A thread waits with `waitid(WEXITED | WNOWAIT)`, so the child stays unreaped until the main thread reaps it, and a forwarded signal can never hit a reused pid.
- 🟢 `just cover` passes with every signal test still running, in about the time it took before 0.14.0.


== Changes

- `src/signals.rs`: a waiter thread blocks in `rustix::process::waitid` with `WaitIdOptions::EXITED | NOWAIT` on the child's pid, then closes the `Signals` handle; the main thread relays from `forever()` until it ends, then reaps with `child.wait()`. SIGCHLD leaves the caught set.
- Red evidence first: the tarpaulin command in Problem, under `timeout`, hangs before the change and passes after.


== Out of scope

- Windows, which keeps its plain wait.
- Skipping the signal tests under tarpaulin with `cfg_attr(tarpaulin, ignore)`. That hides the hang instead of removing it.


== Dogfood

This repo's CI Coverage job runs `just cover` on every push to main, and finishes again.

