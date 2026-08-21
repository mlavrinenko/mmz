#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "mmz: verify the lang-all release build on every target",
  priority: framework("ice", confidence: 0.9, ease: 4.0, impact: 6.0),
  tags: ("build", "tests"),
  links: (
    depends-on("mmz-the-release-ships-a-rust-only-binary.typ")[the matrix
      this proves],
  ),
  status: proposed(2026, 8, 21),
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
- Both assets are attached, and their names distinguish the flavours.
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
