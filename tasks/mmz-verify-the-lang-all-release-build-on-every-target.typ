#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "mmz: verify the lang-all release build on every target",
  priority: framework("ice", confidence: 0.9, ease: 4.0, impact: 6.0),
  tags: ("build", "tests"),
  links: (
    depends-on("mmz-the-release-ships-a-rust-only-binary.typ")[the matrix
      this proves],
    related("mmz-release-assets-ship-uncompressed.typ")[the archives this run
      also has to prove],
    depends-on("mmz-cut-the-0-8-0-release.typ")[the tag whose workflow run
      this watches],
  ),
  status: done(
    2026,
    10,
    22,
  )[v0.13.0 passed on all five targets; aarch64-linux inferred, filed],
)

== Summary

The two-flavour release matrix cannot be proven by any gate in this repo. `just
check` runs on one host, and the CI grammar job proves `lang-all` on
`x86_64-linux` alone. Whether twenty-seven tree-sitter crates compile under
`aarch64-linux-gnu` cross, under both Darwin targets and under `windows-msvc`
is answered by running the release workflow and looking.

This is the human half of the release change, split out so the implementation
task is not held open by a run nobody can automate.

== What to run

Push a tag (or dispatch the workflow) and confirm, for all five targets:

- The `full` leg compiles. `windows-msvc` is the one to watch: it goes from one
  C grammar to twenty-seven, and MSVC is the least forgiving compiler in the
  matrix.
- Both assets are attached, and their names distinguish the flavours. They are
  now archives — `.tar.gz` per unix target, `.zip` for Windows — so check that
  each extracts to a runnable binary under its plain name beside `LICENSE-MIT`,
  and that `sha256sum -c SHA256SUMS` passes over the set.
- The Windows packaging step in particular is unproven: it archives through
  `7z`, which the `windows-latest` image is documented to carry but which no
  run has yet exercised here. Git Bash's `tar` is GNU tar and cannot write a
  zip, so there is no fallback in the step if `7z` is missing.
- `mmz-full-<target> --version` reports 28 ast langs; `mmz-<target> --version`
  reports 1. That is the check that the matrix wired the features to the
  binary it labelled, rather than building the same thing twice.
- The `full` binary actually parses something that is not Rust — one `ast:`
  probe with `lang: python` is enough.

== Done when

The run happened and its outcome is recorded here — pass or fail. A failure
gets its own task linked back to this one and to the release task, rather than
reopening either: "never tried" and "tried and broke" are different states and
the backlog should not conflate them.

== Outcome

Passed on 2026-10-22, checked against the v0.13.0 release run (39332247908), the first tag since this task was filed whose run is complete.

- All ten build legs succeeded. The `windows-msvc` `full` leg compiled every grammar, `tree-sitter-python` included, in 2m31s, and its `7z a -tzip` packaging step succeeded.
- The release carries ten archives plus `SHA256SUMS`, named `mmz-full-<target>` against `mmz-<target>`. `sha256sum -c SHA256SUMS` passes for all ten, and each extracts to `mmz` (or `mmz.exe`) beside `LICENSE-MIT` and nothing else.
- `file` confirms each binary's architecture matches its label.
- `--version` reads `28 ast langs` for `full` and `1 ast lang` for slim. The workflow's own verify step asserts this on x86_64-linux, both Darwin targets and Windows, and a local run on x86_64-linux confirms it.
- An `ast:` probe with `lang: python` runs and invalidates on a body edit under the x86_64-linux `full` binary. The slim binary refuses it with exit 6, naming the missing grammar.
- aarch64-linux is the one target nothing executed: the verify step skips the cross-built leg, and this host has no qemu. Grammar-only node kinds (`elif_clause`, `jsx_element`) are in its `full` binary and absent from its slim one, which is evidence, not a run. Filed as `mmz-the-release-never-runs-its-aarch64-linux-binary`.
