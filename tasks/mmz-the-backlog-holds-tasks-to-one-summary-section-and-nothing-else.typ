#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "mmz: the backlog holds tasks to one Summary section and nothing else",
  tags: ("tooling",),
  priority: framework("ice", confidence: 0.9, ease: 6.0, impact: 6.0),
  status: proposed(2026, 10, 22),
)

== Summary

`.mindtape/config.toml` checks only that a task has one `Summary` section. A task carries no kind, difficulty, assignee or goal verdict, its title has no required shape, and nothing stops a task from closing in a commit apart from its work. The sibling mindtape repo already runs a stricter practice through the `mindtape-tasks` pack.

Done when this repo extends `mindtape-tasks`, scores every open task against product and process goals in `.mindtape/goals.typ` and `.mindtape/process-goals.typ`, carries a `Dogfood` section, estimates in `src-files` under a judgment cap, and ships matching templates, with CONTRIBUTING describing the new flow.

