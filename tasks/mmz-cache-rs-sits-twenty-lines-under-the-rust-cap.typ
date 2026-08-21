#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "mmz: cache.rs sits twenty lines under the Rust cap",
  priority: framework("ice", confidence: 0.9, ease: 9.0, impact: 3.0),
  tags: ("tests",),
  status: done(
    2026,
    8,
    21,
  )[Ran \`just eject\`, which is what \`just fix-check\` does anyway: 480 lines became 277 in \`cache.rs\` and 203 in \`cache\_tests.rs\`, wired by the \`\#\[path\]\` module the other ejected files use. No test moved, changed or was added — this is the file split, nothing else.],
)

== Summary

`src/cache.rs` is 480 lines against a 500-line cap — past the 90% baseline
`just eject` works from, so the next unrelated edit to it lands on the cap
rather than near it. Nothing about the file is wrong; its inline test module
has simply grown to the point where it is the larger half.

This is the case `just eject` exists for, and the fix is to run it. Filed
because a change needs a task to reference, not because the decision needs one.
