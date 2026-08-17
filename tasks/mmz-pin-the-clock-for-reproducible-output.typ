#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "mmz: pin the clock for reproducible output",
  priority: framework("ice", confidence: 0.8, ease: 7.0, impact: 4.0),
  tags: ("cli", "docs", "tooling"),
  status: done(2026, 8, 17),
)

== Summary

Give mmz a dedicated environment variable — `MMZ_NOW` — that pins "now" for one
process, so any output carrying a timestamp is reproducible when it is set and
unchanged when it is not.

Two surfaces read the clock today:

- `--status`'s `AGE` column, rendered relative ("14s ago").
- A cache record's `ran_at` field, a wall-clock epoch.

== Why

`www/generate.sh` captures live `mmz` output for the docs site, and a record
capture cannot be reproduced build-to-build: `ran_at` differs every run. The
generator normalizes it with a `sed`, which works but is a display lie sitting
between the binary and the docs — exactly the kind of hand-correction the
generated-docs pipeline exists to eliminate.

`--status`'s ages are reproducible today only by accident: the generator runs the
commands immediately before capturing, so every row reads "0s ago". A fixture
that wanted to show a genuinely aged record could not.

== Scope

- Resolve the clock ONCE per process, from `MMZ_NOW` when set, else the system
  clock. Thread the resolved value through rather than calling `SystemTime::now`
  at each use site — two calls in one process can disagree.
- Accept a Unix epoch (seconds). A malformed value is a HARD ERROR naming
  `MMZ_NOW`, never a silent fall-through to the system clock: a silent
  fall-through hides the misconfiguration and makes the output non-deterministic
  again, which is the failure the variable exists to remove.
- Never overload `$SOURCE_DATE_EPOCH`. Dev shells and CI routinely set it to the
  1980-01-01 zip-epoch floor, which would silently rewrite every stamped value.
- Drop `www/generate.sh`'s `ran_at` sed and its accompanying comment once the
  variable exists.

== Out of scope

Freshness itself. mmz compares digests, not times, and nothing about a pinned
clock should change which rules are fresh — the variable is for RENDERING and
for the `ran_at` a record stamps, not for the decision.

== Home

mmz's own backlog. Surfaced while building the docs pipeline
(`tasks/mmz-generated-docs-and-tola-site.typ`), which is the only consumer
asking for it today.
