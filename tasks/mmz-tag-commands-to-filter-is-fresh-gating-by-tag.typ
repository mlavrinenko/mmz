#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "mmz: tag commands to filter --is-fresh gating by tag",
  priority: ice(
    impact: 6,
    confidence: 0.8,
    ease: 5,
  ),
  tags: ("cli", "config", "gating"),
  status: done(2026, 7, 24),
)

== Summary

A manifest's `commands:` entries gain an optional `tags: [..]` list. `mmz
--is-fresh` (and `--status`) gain a repeatable `--tag`/`-t <tag>` filter that
narrows the gated/reported rule set to commands carrying every listed tag
(AND semantics across repeats), skipping untagged rules entirely. A bare
`mmz --is-fresh` (no tag, no command) keeps gating every rule, unchanged.

== Why

MindTape wants one mmz manifest shared between (a) gating sub-commands of
`just check` and (b) other non-gating memoized commands (bench/prune-style),
with `mt flip`'s closing gate checking only the gating subset. Today `mmz
--is-fresh` with no command checks every declared rule, forcing a project to
keep the manifest narrow or split `.mmz/config.yaml` per concern. Tags let one
manifest hold both sets and gate by tag instead.

== Scope

- `src/manifest.rs`: `Command.tags: Vec<String>`, trimmed/empty-dropped at
  parse, duplicate tags within one rule rejected at `validate()`.
- `src/freshness.rs`: `evaluate` takes a tag filter; tag + a targeted command
  is a usage error (a command already resolves to one rule).
- `src/main.rs`: `--tag`/`-t` on `--is-fresh` and `--status`, repeatable, ANDed.
- `schema/mmz.schema.json`, `src/init.rs`, `README.md`: document the field and
  the filter.

== Home

mmz's own backlog. Filed while wiring MindTape's `just check` gate to share
one manifest across gating and non-gating commands.
