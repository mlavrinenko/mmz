#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "mmz: adopt probe_shell in this repo's own manifest",
  priority: framework("ice", confidence: 0.7, ease: 4.0, impact: 5.0),
  tags: ("gating", "tooling"),
  links: (
    related("mmz-the-close-gate-depends-on-which-just-is-on-path.typ")[the key
      exists because of that one; this is the half that dogfoods it],
  ),
  status: proposed(2026, 8, 20),
)

== Summary

`probe_shell` shipped, and this repo does not set it. Its eleven gate probes
still resolve `just` and `jq` through whatever PATH the caller had, which is
the defect the key was added to close.

It is not set because neither candidate value is both cheap and available in
CI, and picking one is a real decision rather than a formality.

== The measurements

Each distinct probe is its own process, and there are eleven, so the wrapper's
cost is paid eleven times on every `mmz --is-fresh` — which runs on every task
close and every `just check` arm.

#table(
  columns: 3,
  [*wrapper*], [*warm, one entry*], [*× 11 probes*],
  [`direnv exec .`], [0.12 s], [~1.3 s],
  [`nix develop --command`], [4.4 s], [~48 s],
)

48 seconds to answer "is the build still fresh" defeats the point of the
memoization the gate exists to exploit. `nix develop` is out.

== Why the cheap one is not simply correct

`direnv exec .` costs almost nothing, because nix-direnv caches the resolved
environment. But CI runs `nix develop --command just check`
(`.github/workflows/ci.yml`), and `direnv` is not in the dev shell's packages,
so a probe wrapped in it would fail to spawn there (exit 6). Adding `direnv`
to `flake.nix` is easy; the part that needs deciding is that `direnv exec`
refuses an `.envrc` that has not been allowed, so CI would need a
`direnv allow` step — trusting a checked-out `.envrc` on a runner, which is a
trust-model call and not one to make silently in a follow-up commit.

== Options

- *Add `direnv` to the dev shell and `direnv allow` in CI.* Cheapest at
  runtime. Costs a CI step that trusts the repo's own `.envrc` — defensible
  for a repo where the same commit already supplies the Justfile CI runs, but
  it should be stated rather than assumed.
- *A wrapper script in `.just/scripts/`* that prepends a known profile path
  and `exec sh -c "$1"`. Fast, no new dependency, works in CI. Costs inventing
  machinery to locate the profile, which is the part `direnv` already solves.
- *Leave it unset and rely on `jq -S`.* Honest about where things stand: the
  accidental version dependency is closed and the deliberate one is not. The
  gates keep working, and a foreign `just` on PATH is a loud spawn failure
  rather than a silent one only while no `just` is installed at all.

== Note

Filed rather than decided because the numbers above only became visible after
the key shipped. Nothing here blocks the feature, which is tested on its own
terms in `tests/cli_probes.rs`.
