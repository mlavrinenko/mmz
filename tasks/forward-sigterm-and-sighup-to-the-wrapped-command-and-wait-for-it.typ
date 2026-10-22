#import "@local/mindtape:0.2.0": *
#import "/.mindtape/lib.typ": *

#show: task.with(
  title: "Forward SIGTERM and SIGHUP to the wrapped command and wait for it",
  kind: "feat",
  priority: ice(
    impact: 4,
    confidence: 0.8,
    ease: 5,
  ),
  difficulty: 5,
  tags: ("cli",),
  estimate: (src-files: 4),
  x-goals: (
    product: (
      no-false-green: helps[a run interrupted by a signal records no pass, even when the command's trap exits 0],
      exact-inputs: neutral,
      cheap-gates: neutral,
      agent-drivable: helps[a `timeout` or a CI cancel stops the wrapped command too, instead of leaving it running unseen],
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
  status: proposed(2026, 10, 23),
)

== Problem

+ Declare a rule `sh slow.sh`, with `slow.sh` trapping TERM into a file `got`, then `sleep 3 & wait` and `echo finished > done`.
+ Start `mmz sh slow.sh` in the background and `kill -TERM` its pid after a second.
+ 🔴 mmz exits 143 at once. The child never sees the signal, keeps running as an orphan, and writes `done`, not `got`.


== TO-BE

- 🟢 SIGTERM or SIGHUP sent to mmz reaches the wrapped command, and mmz waits for it, then exits with its status.
- 🟢 Ctrl-C reaches the command through the process group, as today, and mmz waits for it instead of exiting first.
- 🟢 A run during which mmz received any of the three records no `ok`, so it re-runs next time.
- 🟢 A signal the parent shell ignores, as under `nohup`, stays ignored for the command.


== Changes

- `tests/it/cli_signals.rs` and its `mod` line in `tests/it/main.rs`: the red test first, the Problem's steps asserting `got` exists and no `ok` record.
- `Cargo.toml`: add `signal-hook` to catch the signals and `rustix` with its `process` feature to send them, both safe APIs.
- `src/signals.rs`: a guard that catches SIGINT, SIGTERM and SIGHUP while the child runs, forwards TERM and HUP to its pid, remembers that one arrived, and restores the old dispositions on drop.
- `src/engine.rs`: run `exec` under that guard, and skip the `ok` record when it saw a signal.


== Alternatives

- `libc::kill`: needs `unsafe`, which the lints deny.
- `nix`: a far larger crate for the one call `rustix` makes.
- `ctrlc`: catches SIGINT only.
- Kill the child's whole process group: it reaches grandchildren the command meant to keep, and needs the child in its own group, which breaks Ctrl-C.


== Out of scope

- Windows, which has no SIGTERM or SIGHUP to forward.
- The exit code a signal death maps to. That is its own fix task.


== Dogfood

Ctrl-C or a CI cancel on this repo's `just check` stops each `mmz just <gate>` arm's command with it, and no arm records a pass it did not finish.

