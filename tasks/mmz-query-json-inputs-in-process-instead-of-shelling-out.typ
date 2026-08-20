#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "mmz: query JSON inputs in-process instead of shelling out",
  priority: framework("ice", confidence: 0.7, ease: 4.0, impact: 6.0),
  tags: ("config", "efficiency"),
  links: (
    related("mmz-query-code-inputs-in-process-with-ast-patterns.typ")[the same
      argument for code-shaped files; deliberately a separate feature],
    related("mmz-the-close-gate-depends-on-which-just-is-on-path.typ")[where
      the cost of shelling out first showed up],
  ),
  status: proposed(2026, 8, 20),
)

== Summary

Every probe in this repo pipes through `jq`, so resolving the gate set spawns
one `jq` per probe on top of one `just`. A probe that wants part of a JSON
document should be able to say so declaratively and have mmz do the selection
in-process.

```yaml
probes:
  recipe-clippy:
    run: just --dump --dump-format json
    json: '.recipes["clippy"]'
```

== Why

*Spawning is the cost and the risk.* A process per probe is paid on every
`mmz --is-fresh`, which is the operation the whole design leans on being
cheap enough to run routinely. It is also a risk surface: `jq` has to be on
PATH, the filter has to survive a shell round-trip intact, and every quoting
subtlety in the `run:` line is one more way a probe can be wrong in a way mmz
cannot see.

*It shrinks the ambient-tool surface.* Right now a recipe probe depends on two
external binaries. In-process selection removes one of them outright — not by
pinning it, by not needing it.

*It makes canonical hashing structural.* mmz would hash a parsed and
re-serialized value rather than whatever bytes a formatter emitted, so key
order stops mattering by construction instead of by every author remembering
`jq -S`. The scan in `tests/gate_probe_normalisation.rs` exists precisely
because that is currently a convention someone can forget; this would retire
it.

== What it does not fix

`run:` still shells out, so this halves the spawns rather than eliminating
them. The `just --dump` half is the environment question, answered by
`probe_shell`, not by this.

== Open

- *Which query language.* `jaq` (jaq-core 3.1.0, June 2026, MIT, actively
  maintained) is the obvious embed and is a jq *clone* — near-compatible, not
  identical, which is a new correctness surface in a tool whose thesis is not
  lying. Its embedding API moved 2.x to 3.0 in March 2026. A narrower
  pointer-style selector needs no dependency at all (`serde_json` is already
  direct) and covers every probe in this repo, at the cost of not being jq.
  Decide deliberately; the cheap option is not obviously the weaker one here.
- Whether the input is `run:`'s stdout or a file read directly. A file would
  eliminate the spawn entirely for probes that only read the repo.
- `format:`/`json:`/`select:` naming, and how it interacts with `allow_empty`
  and with a selector matching nothing — which must stay a hard error, for the
  reason `jq -e` is load-bearing today.
