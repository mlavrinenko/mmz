#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "mmz: the close gate depends on which just is on PATH",
  priority: framework("ice", confidence: 0.9, ease: 5.0, impact: 6.0),
  tags: ("gating", "tooling"),
  links: (
    related("mmz-just-machete-is-not-busted-by-a-dev-shell-bump.typ")[the
      under-declared gate found while auditing this one's fourth option],
  ),
  status: proposed(2026, 8, 20),
)

== Summary

Closing a MindTape task runs `target/debug/mmz --is-fresh --tag gate` from
`.mindtape/config.toml`'s `[[on.flip]]` hook. mt runs that from the project
root with whatever PATH it inherited — which is not necessarily the dev
shell's. Every gate rule's probe shells out to `just`, so a close run outside
`nix develop` resolves a different `just` binary, gets different `--dump` JSON,
and reports every gate stale against a worktree whose checks just passed.

Observed while closing the composition loader task:

```
$ mt done mmz-load-and-merge-imported-manifest-fragments
  mmz: `just clippy` is stale (probe `recipe-clippy` changed since it last passed)
  ... every gate, same reason
$ direnv exec . mt done mmz-load-and-merge-imported-manifest-fragments
  flipped: wip → done
```

`just --version` was 1.43.1 on the host PATH and 1.51.0 in the dev shell. The
recipe JSON is byte-identical in content; the two versions order the keys
differently, and the probe hashes bytes.

== Why it matters, and why it is not urgent

It fails in the safe direction — a stale reading refuses a close that should
have been allowed, rather than allowing one it should not. Nobody ships a false
green from this.

But it makes the close predicate depend on how the operator's shell happened to
be set up, which is exactly the property a gate must not have, and the error it
prints ("re-run each listed command under mmz") sends someone off to re-run a
full `just check` that will not fix anything. The second `just check` records
against the dev-shell `just` again, the next bare `mt done` reads stale again,
and the loop does not terminate until someone notices the PATH.

This is the "a probe can lie" hazard from the probes design, in its benign
direction: the probe is not measuring what it claims to measure. It claims to
measure the recipe body. It measures the recipe body *as rendered by whichever
just is on PATH*.

== Options

- *Pin the tool in the probe.* Have each probe invoke the dev shell's `just`
  explicitly rather than whatever PATH offers. Most direct, and it makes the
  probe measure what it says it measures. Costs a wrapper or an absolute path
  in eleven probes.
- *Normalise the probe output.* Pipe through `jq -S` so key order stops
  mattering. Cheap, fixes this instance, and does nothing for the next
  version-dependent field just adds or renames. Worth doing regardless — an
  unsorted hash of a JSON object is a latent version dependency whatever else
  is decided.
- *Make the hook enter the dev shell.* `direnv exec . target/debug/mmz …` in
  `.mindtape/config.toml`. Fixes the close path only; a bare `mmz --is-fresh`
  in a pre-commit hook or CI has the same problem.
- *Add the toolchain to the input set.* Honest — the probe genuinely depends on
  the just version — but it means every gate busts on a dev-shell bump, which
  `flake.lock` in the `rust` scope already does for rustc. Arguably correct, and
  the most expensive.

== What the audit found

The fourth option is already implemented, and would not have helped.
`flake.lock` is in the `rust` scope, and nine of the ten gates name `rust`, so
a `nix flake update` that bumps `just` already busts them. (The tenth is
`just machete`, which is a real gap — filed separately.)

It would not have caught this bug because `flake.lock` is byte-identical
inside and outside `nix develop`. It records what the shell *would* hand you,
not what the probe *actually invoked*. Making the tool version a genuine input
needs a self-reporting probe (`just --version`), which detects the mismatch but
turns the phantom-stale into a real bust — honest, no more usable.

So the four options all answer "how do we make the digest stable", and none
answers the question the failure actually poses: is the environment part of the
rule's *identity*, or a *precondition* of the measurement?

- Identity means hashing the tool surface — every tool every recipe reaches.
  Unbounded, and one will be missed.
- Precondition means mmz *refuses to measure* in an environment it was not told
  to expect, instead of quietly hashing a foreign tool's output. That is the
  fifth option, and it matches the rest of the design: mmz already fails closed
  on a missing manifest, an unmatched command, an empty probe. An unmet
  environment precondition is the same class, and currently the only one that
  degrades into a wrong answer rather than an error.

== Progress

`jq -S` landed on all eleven probes, with the rationale recorded in
`.mmz/conf.d/10-rust.yaml` and the regression tests in
`tests/gate_probe_normalisation.rs` (order-stability, an unsorted control, and
a scan asserting every jq probe in this repo sorts). That closes the
accidental version dependency.

What remains open is the deliberate one: the probes still run under whatever
PATH `sh -c` inherits, and nothing declares what that PATH must contain. Decide
identity vs precondition before implementing further.

== Regression test

A test that runs the same probe under two PATHs and asserts the digest is
stable is the honest test, but it needs two just versions on hand. The cheaper
one: assert probe output is order-normalised, so a re-ordered JSON object
hashes the same.

== Note

Found while orchestrating the composition work, not by the composition work —
this predates it and would have bitten the next task close regardless.
