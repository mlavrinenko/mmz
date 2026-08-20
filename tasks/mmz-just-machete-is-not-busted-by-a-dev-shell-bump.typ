#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "mmz: `just machete` is not busted by a dev-shell bump",
  priority: framework("ice", confidence: 0.9, ease: 8.0, impact: 4.0),
  tags: ("gating", "tooling"),
  links: (
    related("mmz-the-close-gate-depends-on-which-just-is-on-path.typ")[found
      while auditing that one's "add the toolchain to the input set" option],
    related("mmz-gate-rules-do-not-declare-the-tools-they-run.typ")[the general
      form of this gap, which would subsume it],
  ),
  status: proposed(2026, 8, 20),
)

== Summary

Every gate rule but one names the `rust` scope, and `rust` carries
`flake.lock` — so bumping the dev shell busts them, which is the behaviour
`docs/contributing/gates.md` argues for under "Getting the scopes right".

`just machete` is the exception. Its inputs are `[manifests, recipe-machete]`:

```yaml
  - name: just machete
    inputs: [manifests, recipe-machete]
    tags: [gate]
```

`manifests` is `["Cargo.toml", "Cargo.lock"]`. Nothing in that set moves when
the dev shell does, and `cargo-machete` comes from the dev shell
(`flake.nix`). So a `nix flake update` that changes which `cargo-machete` runs
leaves the recorded pass looking fresh, and `mmz --is-fresh --tag gate` will
assert a gate that last ran under a different binary.

== Why it matters

This is the dangerous direction, unlike its sibling task: a wrongly-*fresh*
gate is a green build that proved nothing. It is narrow — `cargo-machete`
finding a different set of unused dependencies between versions is not the
likeliest failure this repo will see — but the asymmetry the whole tool is
built on says over-declare, and this rule under-declares.

The reason it was written this way is sound and should survive the fix:
`cargo-machete` reads the Cargo manifests, not the sources, so a source-only
edit must not bust it. Adding `flake.lock` keeps that property. Adding `rust`
would not.

== Fix

Either add `flake.lock` to the `manifests` scope, or give the rule a
toolchain scope of its own and name it alongside `manifests`. The second is
probably better: `manifests` reads as "the Cargo manifests" and a lockfile for
the dev shell does not belong under that name, while a `toolchain` scope
(`rust-toolchain.toml`, `flake.lock`) is a thing several rules could name
honestly.

Note that `rust` would then overlap it. Whether `rust` should be decomposed
into sources + toolchain, with every current caller naming both, is the real
question the fix opens.

== Regression test

Assert that every rule tagged `gate` names an input set whose closure includes
`flake.lock`. That is stronger than fixing this one rule — it is the property
`docs/contributing/gates.md` already claims and nothing currently enforces,
and it would have caught this rule the day it was written.
