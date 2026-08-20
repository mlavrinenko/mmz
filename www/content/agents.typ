#import "../utils/page.typ": page
#import "../utils/ui.typ": callout, transcript
#import "../utils/site.typ": u

#let meta = (
  route: "/agents/",
  label: "For AI agents",
  title: "For AI agents",
  summary: "Driving mmz from an agent: machine-readable state, honest exit codes, and the one mistake an agent is most likely to make with it.",
)
#metadata(meta) <page-meta>

#show: page.with(..meta)

= Why this page exists

An agent working in a repository pays for every re-run twice — once in wall
clock, once in the context the output consumes. `mmz` is well suited to that:
its whole job is answering "has this already passed?" without producing the
output again.

But the same properties that make it useful make one failure mode likely, and it
is worth stating before anything else.

#callout("warn")[
  Do not widen `strict` or narrow a scope to make a red gate go green. A rule
  that reports fresh because its inputs stopped resolving is worse than a rule
  that fails: it is a check that will never run again, and nothing will say so.
]

= Machine-readable state

Every question has a JSON answer, so nothing needs to be parsed out of a table
meant for humans:

```bash
mmz --status=json                 # every rule, its inputs, their hashes
mmz --status=json-schema          # the schema for the above
mmz --schema                      # the manifest schema
```

The state vocabulary is small and total: `fresh`, `stale`, `never`, `failed`,
`no-inputs`, `missing-output` — see #link(u("/cli/"))[the CLI reference]. Reading
`--status=json` is the right move when a gate is red and the question is which
input moved:

```bash
mmz --status=json | jq '.rules[] | select(.state != "fresh") | {name, state, missing_output}'
```

= Honest exit codes

Every code means one thing, so branching on `$?` is safe:

- `1` is only ever a `--is-fresh` verdict, never an error.
- `3` is a strict refusal — the manifest declined to guess.
- `4` is a manifest problem, and is the one case no setting relaxes.
- `5` and `6` both mean a run happened and deliberately left no record: a
  declared output was missing, or a probe failed.
- `7` is a gate that selected no rule — a tag nothing carries, most often. It is
  not a stale build and not a passing one; it is a gate asking about nothing.

An agent should treat `4`, `5`, `6` and `7` as "fix the cause", never as
"retry".

= Recording a pass

`mmz` observes only the commands it wraps. Running `just check` by hand and then
asserting freshness will fail, correctly, and the gate says so:

#transcript("is-fresh-stale.txt")

So the loop is: run the check #emph[under] mmz to record the pass, then assert.
The assertion costs a hash comparison rather than a re-run, which is what makes
it usable in a hook or at the end of a task.

```bash
mmz just check            # runs, or skips if already fresh; records on success
mmz --is-fresh --tag gate # asserts, runs nothing, exits 1 if any gate is stale
```

= Adding a rule

When you add a memoized command, the scopes are the whole decision. The rule:
list every file the command could read.

- Sources, and the manifests and lockfiles that pin its dependencies.
- The toolchain pins, if a toolchain change should re-run it —
  `rust-toolchain.toml`, `flake.lock`. `mmz` trusts file content, never the
  ambient environment.
- The recipe or script body that defines the command, if it lives in a file. If
  it lives in _part_ of a file, use a #link(u("/inputs/"))[probe] rather than
  hashing the whole file.

Then check your work with `mmz --status=json`: if the rule's resolved input list
is shorter than you expected, a glob is not matching — very often because the
path is git-ignored.

#callout("note")[
  When in doubt, over-declare. An unnecessary re-run costs time. A missing input
  costs correctness, silently, and the silence is the expensive part.
]

= Do not commit the cache

`.mmz/cache` is derived, throwaway, machine-local state. `mmz --init` writes a
`.mmz/.gitignore` covering it. If a cache directory ever appears in a diff, the
fix is to ignore it, never to commit it.
