#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "mmz: cut the 0.8.0 release",
  priority: framework("ice", confidence: 0.9, ease: 8.0, impact: 7.0),
  tags: ("build",),
  links: (
    related("mmz-a-released-changelog-section-can-vanish-unnoticed.typ")[the
      corruption this release had to repair first],
    related("mmz-verify-the-lang-all-release-build-on-every-target.typ")[the
      human half, which this release's workflow run is the subject of],
  ),
  status: wip(2026, 8, 21)[changelog repaired and dated; bumping the manifest],
)

== Summary

Cut `v0.8.0`: the composition layer, the in-process JSON and AST probes,
`probe_shell`, the two-flavour release matrix, archived assets, and the
measured binary-size facts. Thirteen commits' worth of changelog entries have
been sitting under `Unreleased` since `v0.7.0` on 2026-08-17.

A minor bump rather than a patch or a major: every headline is additive, and a
manifest written for 0.7.0 loads unchanged under 0.8.0.

== Steps

+ Repair the corrupted `Unreleased` section — the 0.7.0 heading `4c57277` ate,
  and the split prose line beside it. The sibling task carries the detail.
+ Date `== [0.8.0]` and leave `Unreleased` empty above it, the shape `7a7579e`
  set.
+ Bump `Cargo.toml` and `Cargo.lock`.
+ `just check`, then `just outdatty-update` for the `release-notes` group,
  whose source is `Cargo.toml`.
+ Push `main`, then `just release 0.8.0` — which re-runs the gate, tags, and
  pushes the tag the Release workflow triggers on.

== What this release is the first to prove

Every asset shape in it is new and unrun: two flavours per target, `.tar.gz`
and `.zip` archives instead of raw binaries, and `SHA256SUMS` over the set.
The Windows leg archives through `7z` and has never executed. Watching the
workflow and extracting an asset is the sibling verify task, deliberately not
this one — this task closes when the tag is pushed.
