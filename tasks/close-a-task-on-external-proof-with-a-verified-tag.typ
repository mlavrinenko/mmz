#import "@local/mindtape:0.2.0": *
#import "/.mindtape/lib.typ": *

#show: task.with(
  title: "Close a task on external proof with a verified tag",
  kind: "dev",
  priority: ice(
    impact: 4,
    confidence: 0.9,
    ease: 8,
  ),
  difficulty: 2,
  tags: ("tooling",),
  estimate: (src-files: 1),
  x-goals: (
    product: (
      no-false-green: neutral,
      exact-inputs: neutral,
      cheap-gates: neutral,
      agent-drivable: neutral,
      composable-manifest: neutral,
      ready-binaries: neutral,
      docs-teach-judgement: neutral,
    ),
    process: (
      mmz-gates-mmz: neutral,
      docs-read-facts: neutral,
      rules-not-memory: helps[a task proven by a CI run closes under a rule that demands the link, instead of a --force nobody reviews],
      cheap-delegation: neutral,
    ),
  ),
  assignee: "agent",
  status: proposed(2026, 10, 23),
)

== Problem

+ Land a change whose proof is an external run, as `mmz-the-release-never-runs-its-aarch64-linux-binary` does with the release workflow.
+ After the run passes, `mt done <stem>` and commit the flip alone.
+ 🔴 `flip-with-work` refuses it, since the commit holds only task files, and only `--force` gets past.


== TO-BE

- 🟢 A task tagged `verified` may close in a task-only commit.
- 🟢 A `verified` task that is `done` carries exactly one `Verification` section, linking the run that proved it.
- 🟢 Every other task closes with its work, as before.

```toml
[[tasks.body.headers.rules]]
name = "verification-for-verified"
path = ["Verification"]
action = "unique"
when = 'has_tag("verified") && status == "done"'
```


== Changes

- `.mindtape/config.toml`: the `verified` tag, a same-named `flip-with-work` that also exempts a staged `verified` task, and the `Verification` heading and link rules.
- `docs/src/contributing/tasks.typ`: say when a task closes on external proof.


== Rollback

- Revert the commit. A task already closed under it keeps its section, which no other rule reads.


== Out of scope

- Checking that the link resolves or that the run passed. The reviewer opens it.


== Dogfood

`mmz-the-release-never-runs-its-aarch64-linux-binary` closes this way once the 0.14.0 release run is green.

