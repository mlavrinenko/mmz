#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "Bump qahq to current main again",
  status: done(2026, 10, 31),
  tags: ("tweak",),
  estimate: (src-files: 1),
)

qahq main moved to 97d0174, anchoring the shared naersk on linecop so a consumer can drop mindtape
