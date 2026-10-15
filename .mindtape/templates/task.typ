#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: slot(
    "title",
    prompt: "Task title",
    example: "mmz: --status names no rule source",
  ),
  tags: slot("tags", kind: "list", default: ()),
)

== Summary

#slot(
  "summary",
  kind: "raw",
  prompt: "The problem, stated once",
  example: "What is wrong today, and why it matters.",
)
