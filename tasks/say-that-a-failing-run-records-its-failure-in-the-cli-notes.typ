#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "Say that a failing run records its failure in the CLI notes",
  status: done(2026, 10, 23),
  tags: ("tweak",),
  estimate: (src-files: 1),
)

The run action's note said a record is written only on exit 0, but `src/engine.rs` records every outcome and only an `ok` one counts as a pass
