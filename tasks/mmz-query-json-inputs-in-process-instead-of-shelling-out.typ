#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "mmz: query JSON inputs in-process instead of shelling out",
  priority: framework("ice", confidence: 0.7, ease: 4.0, impact: 7.0),
  tags: ("config", "efficiency"),
  links: (
    related("mmz-query-code-inputs-in-process-with-ast-patterns.typ")[the same
      argument for code-shaped files; deliberately a separate feature],
    related("mmz-gate-rules-do-not-declare-the-tools-they-run.typ")[what this
      makes cheap enough to be worth doing],
    related("mmz-the-close-gate-depends-on-which-just-is-on-path.typ")[where
      the cost of shelling out first showed up],
  ),
  status: done(
    2026,
    8,
    20,
  )[Shipped with jaq (1.41 MiB binary delta, under the 3 MB guardrail). Orchestrator verified end-to-end against the real flake.lock: per-node granularity holds (an unrelated node moving does not bust; the named node does), and all four refusals — null selection, empty selection, both sources, file without json — behave as specified.],
)

== Summary

A scope names whole files, so a rule that depends on one field of a JSON file
hashes all of it. The only way to narrow that today is a probe, and a probe is a
subprocess — so reaching one field of a file already on disk costs a shell, a
`cat` or a `jq`, and a dependency on both being present.

A probe should be able to read a file and select out of it with no process at
all:

```yaml
probes:
  ejectest-version:
    file: flake.lock
    json: '.nodes["ejectest"]["locked"]["narHash"]'
```

That is the primary shape. No `run:`, no shell, nothing on PATH — mmz opens the
file, parses it, selects, and hashes the result.

== Why this is the case that matters

*It is the only fully in-process one.* Everything else a probe does today
involves spawning something. This does not, so it has no ambient-tool
dependency, no shell quoting to get wrong, and no per-probe process cost on an
operation (`mmz --is-fresh`) whose whole value is being cheap enough to run
routinely.

*It reaches inputs that are otherwise inexpressible.* `flake.lock` in this repo
has over a hundred nodes. The `rust` scope hashes the whole file, so bumping
`nixpkgs-lib` busts clippy and the full suite. `.nodes["qahq"]["locked"]["narHash"]`
busts only when the input that actually supplies linecop, outdatty and ejectest
moves. That is not a smaller version of the same dependency — it is the
dependency the rule actually has, and there is currently no way to write it.

*It is the enabling half of declaring tools.* Per-tool version tracking is filed
separately and its main objection is cost: eleven more probes is eleven more
processes. Sourced from the lockfile instead of from `--version`, they are free.

== Combining with `run:`

`run:` plus `json:` should work too — select out of a command's stdout rather
than out of a file, which is what every recipe probe in this repo does today
through `jq`:

```yaml
probes:
  recipe-clippy:
    run: just --dump --dump-format json
    json: '.recipes["clippy"]'
```

That halves the spawns rather than removing them, and it is the weaker of the
two cases. It earns its place by removing `jq` from the ambient-tool surface and
by making canonical hashing structural — mmz would hash a parsed value rather
than a formatter's bytes, so key order stops mattering by construction instead
of by every author remembering `jq -S`. The scan in
`tests/gate_probe_normalisation.rs` exists precisely because that is currently a
convention someone can forget; this retires it.

`file:` and `run:` are mutually exclusive. A probe declaring both is a manifest
error, not a precedence rule to memorise.

== Open

- *Which query language.* `jaq` (jaq-core 3.1.0, June 2026, MIT, actively
  maintained) is the obvious embed, and is a jq *clone* — near-compatible, not
  identical, which is a new correctness surface in a tool whose thesis is not
  lying. Its embedding API moved 2.x to 3.0 in March 2026. A narrower
  pointer-style selector needs no new dependency at all (`serde_json` is already
  direct) and covers every probe in this repo and the lockfile case above. The
  cheap option is not obviously the weaker one; decide deliberately.
- A selector matching nothing must stay a hard error, for the reason `jq -e` is
  load-bearing today: a probe that silently tracks `null` is permanently fresh
  against a digest that measures nothing.
- Whether `file:` accepts more than one path, and whether it participates in the
  gitignore filter the way a scope does.
- Key naming (`json:` vs `select:`), and how it reads beside the `ast:` form the
  sibling task proposes.
