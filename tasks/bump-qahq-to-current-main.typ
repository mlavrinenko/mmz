#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "Bump qahq to current main",
  status: done(2026, 10, 31),
  tags: ("tweak",),
  estimate: (src-files: 1),
)

qahq main moved to 3feae8d, so the pinned dev-shell tools are a revision behind
