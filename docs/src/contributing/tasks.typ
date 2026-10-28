// docs/src/contributing/tasks.typ — source for docs/contributing/tasks.md.
// Rendered to Markdown by docs/generate-md.sh via typlite.
//
// Split out of docs/src/contributing.typ, which sits at the Typst line cap: the
// process document keeps the commit rules, this file the backlog's shape.
#import "../lib.typ": doc-title

#metadata((
  output: "docs/contributing/tasks.md",
)) <mmz-md>

#doc-title[Tasks]

The backlog is #link("https://github.com/mlavrinenko/mindtape")[MindTape]: one
Typst file per task under `tasks/`, ruled by `.mindtape/config.toml` and changed
through the `mt` CLI, never by hand. `.mmz/` is unrelated: it is mmz's own
command cache.

The config extends MindTape's `mindtape:tasks` practice, and `mt check` holds
every open task to it:

- The title starts with an imperative verb.
- The body is Problem, TO-BE, Changes and Out of scope, plus the kind's own
  section (Environment for a `fix`, Alternatives for a `feat`, Source of truth
  for `docs`, Rollback for `dev`), then Dogfood: how this repo uses the change,
  or "Not applicable" and why.
- The fields carry `kind`, `difficulty` (the judgment it needs, 0 to 9),
  `estimate: (src-files: N)`, `assignee` and an `x-goals` verdict for every goal
  in `.mindtape/goals.typ` and `.mindtape/process-goals.typ`. A task that helps
  no goal is refused.

```bash
mt add --list             # the templates, one per kind plus tweak and finding
mt add -t fix "<title>"   # file one
mt ls @agent              # open work an agent can take
```

Filing is a task-only commit, `chore(tasks): file …`. After that the status
rides with the work: `mt wip <stem>` goes in the first commit that names the
task, and `mt done <stem>` in the last. A one-file change too small to plan is a
tweak, filed done in the commit that makes it. A task whose proof is an external
run, such as a release, carries the `verified` tag: it closes in a commit of its
own once the run passes, with a `Verification` section linking that run.

Closing a task runs `mmz --is-fresh --tag gate`, so run `just check` to record a
pass, then flip. `--force` waives the gate and records the waiver in the note.
That is mmz dogfooding its headline feature on its own backlog.
