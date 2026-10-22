#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "Quiet cargo clippy --fix in just fix-check",
  status: done(2026, 10, 23),
  tags: ("tweak",),
  estimate: (src-files: 1),
)

clippy-fix printed cargo's Checking and Finished lines on every just fix-check, the one loud step in an otherwise quiet run
