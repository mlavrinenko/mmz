#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "Migrate the MindTape config to the current mt",
  priority: ice(
    impact: 7,
    confidence: 0.9,
    ease: 9,
  ),
  tags: ("tooling",),
  status: proposed(2026, 10, 10),
)

== Summary

`mt check` fails under mt built from mindtape main (382711d9, installed 2026-10-10):

```
.mindtape/config.toml:124:2: can't parse config: unknown field `handle`, expected one of `project`, `tasks`, `ls`, `rm`, `retire`, `templates`, `add`, `go`, `typst`, `format`, `views`, `dicts`, `commits`, `on`, `actors`, `roles`, `extends`, `disable`
```

Every `mt` command that loads the config refuses, so the backlog is unreadable until the config follows the current schema.

== TO-BE

- `mt check` passes under the current `mt`.
- `just check` (or the repo's gate) is green again.

== Changes

- `.mindtape/config.toml`: drop `[handle] check-len`. A handle is now the task's stem hash, which grows only where two stems share a prefix, so it has no width to set and no ordinal to go stale.
- `.mindtape/templates/task.typ`: `mt check` now checks templates too, and the built-in one carries no Summary section, so it breached this backlog's Summary rule. The project template supplies one, so `mt add` files a task the rules accept.

== Out of scope

- Pinning `mt` in the dev shell, which waits for an `mt` release.
