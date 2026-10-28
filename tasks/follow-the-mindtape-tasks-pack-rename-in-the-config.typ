#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "Follow the mindtape:tasks pack rename in the config",
  status: done(2026, 10, 28),
  tags: ("tweak",),
  estimate: (src-files: 1),
)

mt 0.3.0 refuses the pack name mindtape-tasks as unknown, so every mt command failed. The pack is mindtape:tasks now.
