#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "mmz: --is-fresh message should say to run the command under mmz",
  priority: ice(
    impact: 4,
    confidence: 0.7,
    ease: 8,
  ),
  tags: ("cli",),
  status: done(2026, 7, 24),
)

== Summary

The `mmz --is-fresh` gate prints a reason like "is stale (inputs changed since
it last passed)" (and a "never" variant). The reason names the cache state but
not the fix: mmz only observes a command it wraps, so running the command
standalone never records a pass and the rule stays stale or never, no matter
how many times it passed outside mmz. A user who just ran the bare command
reads "stale" as a lie.

Make the non-fresh output name the remedy: run the command under mmz, e.g.
`mmz just check`, so the pass is recorded — a standalone run is not tracked.
That is exactly what every downstream gate needs the user to do (mindtape's
`mt flip` closing gate calls `mmz --is-fresh -- just check`).

== Why

Hit while closing a mindtape task: `just check` passed standalone, then
`mmz --is-fresh -- just check` reported stale, because mmz never saw the pass.
The mindtape gate's own message already hints "run mmz just check", but mmz's
core `--is-fresh` output — reused by any project — does not, so the lesson does
not generalize past the one project that spelled it out in its gate config.

== Scope

- Reason strings live in `src/status.rs` (the Stale and Never reasons); the gate
  line is assembled in `src/freshness.rs` and `src/main.rs`.
- Add a one-line remediation to the non-fresh `--is-fresh` output: how to record
  a pass, stated once, not per rule.
- Keep the machine-readable state labels (stale, never, failed) unchanged; this
  is the human hint only.

== Home

mmz's own backlog. Filed from mindtape after the message tripped a task close.
