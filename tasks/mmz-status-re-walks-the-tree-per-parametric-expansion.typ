#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "mmz: --status re-walks the tree per parametric expansion",
  priority: ice(
    impact: 4,
    confidence: 0.8,
    ease: 6,
  ),
  tags: ("cli", "efficiency"),
  status: proposed(2026, 7, 24),
)

== Summary

`src/status.rs` resolves a rule's shared `inputs` globs with a full filesystem
walk once per expansion. For a parametric rule fanning M files, `collect`
first walks the tree once to resolve the macro domain
(`parametric::expand_rule` -> `resolve_domain`), then calls `rule_status` M
times, and each call re-runs `resolve::expand(globs_for(rule), ...)` — an
identical, full `WalkBuilder` traversal of the project root — just to re-derive
the same shared-pin file set before appending the one bound file.

So `mmz --status` on a parametric rule over a large source tree does
O(M x tree_size) work where O(tree_size) would do: the shared inputs are the
same for every expansion and need resolving once.

== Why

Not a correctness bug (the digests come out right), but `--status` is meant to
be a cheap "what would run right now?" query, and it degrades sharply as a
parametric scope grows. The engine avoids this because `run` resolves a single
hit; only the enumerating readers (`--status`) pay the multiplier.

== Scope

- `src/status.rs`: resolve each rule's shared `inputs` glob set once (keyed by
  rule), then per expansion only hash that cached set plus the bound file,
  rather than re-walking in `rule_status`.
- Confirm `parametric::expand_rule` need not re-walk the domain redundantly
  alongside the shared-input resolution.
- Keep the per-file digest identical (a golden/JSON assertion in
  `src/status_tests.rs`) so the optimization is behaviour-preserving.

== Home

mmz's own backlog. Found while reviewing the parametric read paths.
