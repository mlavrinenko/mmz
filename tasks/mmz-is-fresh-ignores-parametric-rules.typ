#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "mmz: --is-fresh ignores parametric rules",
  priority: ice(
    impact: 8,
    confidence: 0.9,
    ease: 5,
  ),
  tags: ("cli", "gating"),
  status: done(2026, 7, 24),
)

== Summary

`mmz --is-fresh` does not expand parametric (`{scope}`-fanned) rules, so its
gate is broken for any manifest containing one. `run`, `--status`, and
`--prune` all route through `src/parametric.rs`; `src/freshness.rs::evaluate`
alone iterates raw `manifest.commands` and keys each verdict on `rule.name`.

Two concrete failures, both reproduced against 0.5.0:

- Untargeted (`mmz --is-fresh`) or tag-filtered: a parametric rule is evaluated
  under its literal template name (`sh -c true sh {targets}`). `cache::read`
  keys on per-file expanded identities, so the template name never has a
  record and the verdict is always `never` (or `no-inputs` when the rule
  declares no shared `inputs`). Any manifest with a parametric rule fails the
  gate forever, regardless of the real per-file cache state.
- Targeted (`mmz --is-fresh -- sh -c true sh src/a.rs`): `matcher::first_match`
  does static token matching, so `{targets}` never equals `src/a.rs` and the
  invocation returns `NoMatch` (exit 3). A single per-file expansion cannot be
  gated at all.

Meanwhile `mmz --status` correctly enumerates per-file rows with correct
states, so the two commands disagree on the same manifest.

== Why

`--is-fresh` is the documented reach-for gate (a pre-push hook, `just check`,
MindTape's `mt flip` closing gate). Parametric rules (0.4.0) shipped the
per-file cache but `--status` was the only reader updated to expand them;
`freshness.rs` was never taught the expansion, and the 0.5.0 tag work extended
`evaluate` without closing the gap. A downstream project that adds a per-file
gate rule sees its gate wedged permanently non-fresh.

== Repro

```
scopes: { targets: ["src/**/*.rs"] }
commands: [{ name: "sh -c true sh {targets}", inputs: [targets] }]
```

Record one file (`mmz sh -c true sh src/a.rs`), then:
- `mmz --status` shows `src/a.rs fresh`, `src/b.rs never`.
- `mmz --is-fresh` prints "`sh -c true sh {targets}` is never" and exits 1.
- `mmz --is-fresh -- sh -c true sh src/a.rs` prints "no rule matches" (exit 3).

== Scope

- `src/freshness.rs`: `evaluate` must expand parametric rules like
  `status::collect` does (`parametric::expand_rule`), keying each `Verdict` on
  the expansion identity, so untargeted/tag gates report one verdict per
  per-file record. Also run `parametric::detect_collision` on the expansions,
  which the untargeted path skips today.
- Targeted path: bind the invocation through `parametric::resolve_matches`
  (or `match_rule`) instead of `matcher::first_match`, so
  `--is-fresh -- <cmd> <file>` gates the one expansion it resolves to.
- Decide and document the whole-rule gate semantics (a bare `--is-fresh` over a
  parametric rule passes only when every per-file expansion is fresh).
- Regression tests: a CLI test (`tests/cli.rs`) plus unit tests in
  `src/freshness.rs` covering untargeted, tag-filtered, and targeted gates over
  a parametric rule.

== Home

mmz's own backlog. Found in a critical review of the `--is-fresh` path against
the shipped parametric feature.
