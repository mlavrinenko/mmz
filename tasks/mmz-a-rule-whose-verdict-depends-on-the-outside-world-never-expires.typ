#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "mmz: a rule whose verdict depends on the outside world never expires",
  priority: framework("ice", confidence: 0.8, ease: 6.0, impact: 6.0),
  tags: ("cli", "docs", "tooling"),
  status: done(
    2026,
    10,
    2,
  )[shipped in 0.11.0: `ttl` on a rule, the `expired` state, `ttl`/`expires_at` in --status=json; tests/cli_ttl.rs drives expiry through MMZ_NOW],
  links: related(
    "mmz-pin-the-clock-for-reproducible-output.typ",
  )[`MMZ_NOW` is the clock a ttl is measured against, and the test hook],
)

== Summary

Some commands' verdict depends on the world, not on their inputs: `npm audit`
reads an advisory database that changes daily, and a check that every image a
compose file references still exists asks a registry. Memoized on input digests
alone, one pass of either stays fresh forever — the exact false green mmz
exists to prevent.

Add an optional per-rule `ttl`:

```yaml
commands:
  - name: just audit
    tags: [gate]
    inputs: [web-pkg, api-pkg]
    ttl: 24h
```

== Scope

- A successful record whose age reaches the rule's `ttl` is not fresh: the
  wrapped command re-runs, `--is-fresh` exits 1, and `--status` reports a
  distinct `expired` state — not `stale`, since no input moved and a reader
  told "stale" would go diffing files that did not change.
- `--status=json` carries the state, the declared `ttl`, and `expires_at`, the
  instant the recorded pass stops being fresh.
- Duration grammar: one positive whole number and one unit, `s`, `m`, `h` or
  `d` (`30m`, `12h`, `7d`). Zero, a missing unit, a compound (`1h30m`), a sign,
  a leading zero, or an overflow is a load error at exit 4.
- Age is measured against the process clock (`src/clock.rs`), so `MMZ_NOW`
  pins it — which is also how the CLI suite tests expiry without sleeping.
- A rule with a `ttl` and no declared `inputs` is legal: time is its input.
  A ttl rule whose declared inputs resolve to nothing is still `no-inputs`,
  since that is the misconfiguration the strict case exists to catch.
- No `ttl` keeps today's behaviour byte for byte.

== Decisions

- *No manifest-level default.* `on_hit` has one because a notice is
  presentation, uniform across rules. A ttl is a claim about one command's
  dependence on the outside world; a default would make every input-determined
  rule re-run on a timer for nothing, and it would be a sixth policy key with
  the whole compose wiring that implies. A YAML anchor shares one value across
  the few rules that want it.
- *No record format change.* `ran_at` has been a required record field since
  0.1.0 and is stamped on every write, so a successful record's `ran_at` is its
  success time. There is no legacy record without one: a record lacking it
  fails to parse and is already a miss.
- *`expired` only when the digest still matches.* A record that is both old and
  stale reports `stale`, the fact that would re-run it regardless; a declared
  output that is gone outranks both, as it already outranks `stale`.
- *Boundary.* A record is expired once its age is at least the ttl, so
  `expires_at` (= `ran_at` + ttl) is the first second it is no longer fresh. A
  record stamped in the future (a rewound clock) has age zero.

== Done when

`ttl` parses, validates, expires records in `mmz <command>`, `--is-fresh` and
`--status`, is described by both manifest schemas and the status schema, and has
prose on the docs site.
