#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: slot(
    "title",
    prompt: "Task title, imperative verb first",
    example: "Fix the typo in the `mmz --prune` help text",
  ),
  status: done(2026, 10, 23),
  tags: ("tweak",),
  estimate: (src-files: 1),
)

#slot(
  "why",
  kind: "raw",
  prompt: "Why the change, one line",
  example: "The help said `remove` where the action says `delete`",
)
