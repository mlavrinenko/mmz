#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "mmz: a passing gate run prints hundreds of lines",
  priority: framework("ice", confidence: 0.9, ease: 8.0, impact: 4.0),
  tags: ("tooling",),
  status: done(
    2026,
    10,
    2,
  )[memo arms run under chronic, cover prints its summary line, crap its per-crate table; a green `just fix-check` is four lines],
)

== Summary

A non-memoized `just check` printed about six hundred lines on a pass,
`just cover` about six hundred more, and `just crap` a row per function. Each
says one bit — passed — and every session that runs them pays the rest in
context. The quiet `chronic` wrapping `check` once had was dropped without a
stated reason when the docs rewrite replaced the recipe.

== Done when

A passing run of each prints a line or a summary, and a failing run still
prints everything needed to fix it.
