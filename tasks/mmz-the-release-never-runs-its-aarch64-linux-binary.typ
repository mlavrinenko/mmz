#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "mmz: the release never runs its aarch64-linux binary",
  tags: ("build",),
  priority: framework("ice", confidence: 0.8, ease: 6.0, impact: 5.0),
  status: proposed(2026, 10, 22),
  links: related(
    "mmz-verify-the-lang-all-release-build-on-every-target.typ",
  )[found by this verification],
)

== Summary

`.github/workflows/release.yml` cross-builds `aarch64-unknown-linux-gnu` on an x86_64 runner, so its "Verify the flavour is what it claims" step is skipped for that target (`if: matrix.target != 'aarch64-unknown-linux-gnu'`). Every other target's binary is executed before it is published; this one ships having never started. The v0.13.0 verification could only infer from strings that its `full` binary carries the grammars.

Done when both aarch64-linux legs run the verify step on every release, either built natively on GitHub's `ubuntu-24.04-arm` runner or run under qemu-user after the cross build, and the skip condition is gone.

