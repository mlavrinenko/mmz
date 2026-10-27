#import "@local/mindtape:0.2.0": *
#import "/.mindtape/lib.typ": *

#show: task.with(
  title: "Report whether a wrapped run skipped or ran through MMZ_REPORT",
  kind: "feat",
  priority: ice(
    impact: 4,
    confidence: 0.9,
    ease: 7,
  ),
  difficulty: 3,
  tags: ("cli",),
  estimate: (src-files: 4),
  x-goals: (
    product: (
      no-false-green: neutral,
      exact-inputs: neutral,
      cheap-gates: helps[a wrapper learns whether its gate was skipped without asking `--is-fresh` first, which hashes every input a second time],
      agent-drivable: helps[a script reads `skipped` or `ran` from a file instead of guessing from output a quiet `on_hit` leaves empty],
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
  status: done(2026, 10, 28),
)

== Problem

+ Declare a rule `sh` over `*.txt` with `on_hit: ""`, as a gate runner that wants a silent `just check` does.
+ Run `mmz sh -c true >log 2>&1` twice.
+ 🔴 Both runs leave an empty `log` and exit 0, so a caller cannot tell the skip from the run. Mindtape's `.just/scripts/mmz-arm.sh` asks `mmz --is-fresh -- just <gate>` before every arm to learn it, which resolves and hashes the rule's whole input set twice and can disagree with the run that follows it.


== TO-BE

- 🟢 `MMZ_REPORT=<path> mmz <command>` writes `skipped` to the path when mmz skips the command, and `ran` when it runs it, memoized or not, before the command starts.
- 🟢 Unset or empty writes nothing, so a manifest and a run without it behave as before.
- 🟢 A path mmz cannot write stops the invocation with exit 8 naming it, before the command runs and before any record is written.
- 🟢 `--is-fresh` and `--status` run nothing, so they ignore the variable.


== Changes

- `tests/it/cli_report.rs` and its `mod` line in `tests/it/main.rs`: the red test first, a run then a skip writing `ran` then `skipped`.
- `src/report.rs`: resolve `MMZ_REPORT` and write the word, mapping a failure to a new `Error::ReportWrite`.
- `src/engine.rs`: write `skipped` on the skip and `ran` before every spawn.
- `src/error.rs`, `src/main.rs`: the new case at exit 8, and `MMZ_REPORT` in `--help`'s Environment list.
- `www/content/cli.typ`, `www/utils/exit-code-notes.typ`, `CHANGELOG.md`: document the variable.


== Alternatives

- Let the arm set a non-empty `on_hit` and match the log against it: the notice shares stderr with the command, so a command that printed the same line reads as a skip, and every streamed run outside the arm prints the notice too.
- Record each hit in the cache and read it back with `--status=json`: a hit would write the cache, and the caller still needs a second invocation.
- A `--report=<path>` flag before the command: mmz reads its first token only, so a flag ahead of a wrapped command needs a new argv grammar, and `MMZ_DISABLE` already set the precedent of an environment switch that reaches every `just` arm at once.


== Out of scope

- Reporting why a run ran (stale, never, ttl, forced). `--status=json` already says why before the run.
- A JSON report. One word is all the known caller reads, and a richer shape can come with a caller that needs it.


== Dogfood

Not applicable: `just memo` streams each arm's output and needs no count of skips. Mindtape's `mmz-arm.sh` is the caller that drops its `--is-fresh` pre-check for it.

