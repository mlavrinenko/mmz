#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "mmz: command-driven input hashes",
  priority: framework("ice", confidence: 0.7, ease: 4.0, impact: 6.0),
  tags: ("config", "cache", "cli"),
  status: proposed(2026, 8, 16),
)

== Summary

A rule can take an input whose bytes come from a command's stdout instead of a
file's contents. A new top-level `probes:` map declares them by name, and
`inputs:` references a probe name exactly as it references a scope name:

```yaml
probes:
  fmt-recipe:
    run: just --dump --dump-format json | jq -c '.recipes["fmt-check"]'

commands:
  - name: just fmt-check
    inputs: [rust, fmt-recipe]
```

The probe's stdout is hashed and joined into the rule's input digest. Nothing
else about a rule changes.

== Why

A scope can only name whole files, so a rule that depends on part of a file has
to hash all of it. MindTape's `.mmz/config.yaml` shows the cost: every rule
shares a `gates-meta` scope holding the whole `Justfile`, so a one-line edit to
a docs recipe busts all eighteen rules and re-runs clippy and the full test
suite. The dependency is one recipe body, and that body is reachable —
`just --dump --dump-format json` prints it — it is simply not a file.

The same shape covers a toolchain fingerprint (`rustc -vV`), a resolved
dependency set, or anything else a project can print deterministically. just's
own cached recipes carry this as the `extra` key, for the same reason.

== Failure modes, and where each one is owned

The primitive can lie in a way a file hash cannot, so the boundary is the
design:

- A probe that exits non-zero is a hard error. mmz names the probe, its exit
  code, and its stderr, and exits without consuming the output or writing a
  record. A failed command must never reach the hasher.
- A probe that cannot be spawned is the same error.
- Empty stdout is an error by default, with `allow_empty: true` to opt in. It
  is the cheapest catch for a selector that matched nothing.
- Content correctness is the consumer's. A probe that prints valid but wrong
  JSON, or that is non-deterministic, is the manifest author's bug: pin the
  ordering, strip the timestamps, assert the shape in the probe itself
  (`jq -e`, a schema check) so a bad shape becomes a non-zero exit and hits the
  rule above. mmz does not validate meaning, and should not learn to.

Document that trade honestly next to the key: a wrong scope costs time, a wrong
probe can lie. Anyone reaching for this should read that sentence first.

== Scope

- `src/manifest.rs`: `probes:` map, `run` plus optional `allow_empty`. A name
  collision between a probe and a scope is a parse error — one namespace for
  `inputs:` entries, so a reader never has to guess which kind a name is.
- `src/resolve.rs`: resolve a probe once per process, not once per referencing
  rule. Eighteen rules sharing one probe run it once.
- `src/freshness.rs`: probe digests join the input digest; the stale reason
  names the probe when it is the thing that changed.
- `src/status.rs`, `schema/status.schema.json`: expose each probe's current
  digest in `--status=json`, so a consumer can see what mmz saw.
- `schema/mmz.schema.json`, `README.md`, `www/index.html`.
- Tests: a probe changing output busts the rule; a non-zero probe errors and
  writes nothing; an empty probe errors unless opted in; a probe shared by two
  rules runs once; a name colliding with a scope is rejected at parse.

== Cost to weigh while implementing

Every `mmz --is-fresh` resolves every referenced probe, and that path runs in
git pre-commit hooks. Keep the once-per-process resolution honest and measure a
realistic manifest before calling this done.

== Home

mmz's own backlog. The MindTape side of the motivation is its own task: split
the shared `gates-meta` scope first, measure what churn is left, and only then
decide whether the Justfile rules should move to probes.
