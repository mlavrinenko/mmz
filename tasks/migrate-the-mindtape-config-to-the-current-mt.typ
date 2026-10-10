#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "Migrate the MindTape config to the current mt",
  status: proposed(2026, 10, 10),
)

== Problem

`mt check` fails under mt built from mindtape main (382711d9, installed 2026-10-10):

```
.mindtape/config.toml:124:2: can't parse config: unknown field `handle`, expected one of `project`, `tasks`, `ls`, `rm`, `retire`, `templates`, `add`, `go`, `typst`, `format`, `views`, `dicts`, `commits`, `on`, `actors`, `roles`, `extends`, `disable`
```

Every `mt` command that loads the config refuses, so the backlog is unreadable until the config follows the current schema.

== TO-BE

- `mt check` passes under the current `mt`.
- `just check` (or the repo's gate) is green again.

== Changes

- `.mindtape/config.toml`: rename or drop the keys the current schema rejects; `mt config show --origin` and `mt init --full` show the current shape.
- Task files or templates the new rules refuse (a list in a single-valued field, a template slot with no `example`).

== Out of scope

- Pinning `mt` in the dev shell, which waits for an `mt` release.
