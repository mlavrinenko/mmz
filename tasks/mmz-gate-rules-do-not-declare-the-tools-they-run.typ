#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "mmz: gate rules do not declare the tools they run",
  priority: framework("ice", confidence: 0.8, ease: 5.0, impact: 7.0),
  tags: ("gating", "tooling"),
  links: (
    related("mmz-the-close-gate-depends-on-which-just-is-on-path.typ")[the
      environment question this is the input-side answer to],
    related("mmz-just-machete-is-not-busted-by-a-dev-shell-bump.typ")[the one
      rule where the gap is already concrete],
  ),
  status: proposed(2026, 8, 20),
)

== Summary

A gate rule declares the files it reads and the recipe body it runs. It does
not declare the *tools* that run it. When the dev shell moves, the recorded
pass was earned by binaries that no longer exist, and nothing in the input set
says so.

The stack changes, so everything derived from it goes stale. Right now nothing
represents that.

== What stands in for it today, and why it does not hold

`flake.lock` is in the `rust` scope, and nine of the ten gates name `rust`, so
a `nix flake update` busts them. That is a blanket, and it is wrong in both
directions:

- *Too coarse.* Any dev-shell change busts every gate naming `rust`. Bumping
  typst re-runs clippy and the full suite. The churn is real and it is paid
  every time.
- *Too coarse to be right.* `just machete` names `[manifests, recipe-machete]`
  and no lockfile at all, so a `cargo-machete` bump leaves its record looking
  fresh. That is the dangerous direction and it is filed separately.
- *Not what it claims.* `flake.lock` pins input revisions — nixpkgs and the
  flake inputs, plus `"version": 7`, the lockfile schema version. It carries no
  package versions. It cannot distinguish "just went 1.43 to 1.51" from "some
  unrelated input moved", and it is byte-identical whether or not the caller is
  inside the dev shell.

== The shape

Declare each tool as a probe and name it per command, the way a scope is
declared once and referenced:

```yaml
probes:
  tool-just:    { run: just --version }
  tool-machete: { run: cargo-machete --version }

commands:
  - name: just machete
    inputs: [manifests, recipe-machete, tool-just, tool-machete]
```

Three properties this has and `flake.lock` does not. It is per-tool, so
`machete` busts on `cargo-machete` and nothing else — strictly less churn than
the blanket, not more. It measures the binary actually in use rather than the
one a lockfile describes. And it is honest under any PATH, because reporting
the ambient tool is precisely its job — unlike a recipe-body probe, which
claims to measure the recipe and actually measures the recipe as rendered by
whichever `just` answered.

If this lands, `flake.lock` should come *out* of `rust` rather than sit
alongside it. Two mechanisms for one property is how the `machete` gap stayed
invisible.

== Open

- Some version strings are noisier than what they guard: `cargo --version`
  carries a git hash and a date. Those need trimming in the probe, which is
  the manifest author's job, but the reference should say so.
- A recipe's transitive tools stop at the boundary declared. `just docs::check`
  runs tola, which runs typst; declaring tola and stopping is defensible, and
  it is still a boundary that can be drawn wrong.
- Eleven more probes is eleven more processes per `mmz --is-fresh`. Worth
  sequencing after in-process querying if that lands, or measuring first.

== Regression test

Assert that every rule tagged `gate` names at least one input whose digest
moves when the toolchain does. That is the property
`docs/contributing/gates.md` already claims under "Getting the scopes right"
and nothing currently enforces.
