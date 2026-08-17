#import "../utils/page.typ": page
#import "../utils/ui.typ": callout, transcript
#import "../utils/site.typ": u

#let meta = (
  route: "/gating/",
  label: "Gating with tags",
  title: "Gating with tags",
  summary: "Use --is-fresh to require that an expensive check already passed, and tags to decide which rules a gate is allowed to ask about.",
)
#metadata(meta) <page-meta>

#show: page.with(..meta)

= The inverse of wrapping

`mmz <command>` runs a stale command. `mmz --is-fresh -- <command>` refuses it:

- exit 0 when the rule is already fresh,
- exit 1 when it is not,
- and nothing runs either way.

With no command, `mmz --is-fresh` gates every rule in the manifest at once.

That is what a hook wants. A pre-push check can require that an expensive suite
was already run and memoized, without paying to run it at the least convenient
moment:

```bash
# pre-push: refuse the push if the VM checks were not run, but never run them here
mmz --is-fresh -- just check-vm || {
  echo "stale: run 'just check-vm' first" >&2
  exit 1
}
```

A non-fresh gate prints one line per offender, naming the rule and why it would
re-run:

#transcript("is-fresh-stale.txt")

The trailing hint matters more than it looks. `mmz` only observes commands it
wraps, so running the check by hand records nothing and leaves the rule exactly
as stale as it found it. The gate says so rather than letting someone loop.

= Tags

A rule can carry `tags:`, and `--is-fresh --tag <tag>` (or `--status --tag`)
narrows to rules carrying every listed tag:

```yaml
commands:
  - name: cargo test
    inputs: [rust]
    tags: [gate]
  - name: cargo bench
    inputs: [rust]
    tags: [bench]
```

`mmz --is-fresh --tag gate` here checks only `cargo test`. A bare
`mmz --is-fresh` still checks both.

#transcript("status-tag.txt")

Repeat `--tag`/`-t` to require more than one; repeats AND together. There is no
OR — call `mmz --is-fresh` once per tag for that. A rule with no `tags:` never
matches a `--tag` filter, and combining `--tag` with a targeted command is a
usage error, since a command already resolves to exactly one rule.

Tags are case-faithful and trimmed, blank entries are dropped, and declaring the
same tag twice on one rule is a manifest error.

= Why this is a tag and not a second manifest

One manifest can now hold a gating subset alongside memoized commands a gate
should ignore. Without tags the alternatives are both worse: split
`.mmz/config.yaml` per concern and maintain two files that share scopes, or let
the gate assert freshness of rules it has no business blocking on.

#callout("note")[
  Tag the rules a gate is _allowed to fail on_, not everything you memoize. A
  benchmark that nobody must run before pushing has no business turning a push
  red.
]

= mmz gates itself

This repository is the worked example. `.mmz/config.yaml` tags every
`just check` arm `gate`, and closing a task in the
#link("https://github.com/mlavrinenko/mindtape")[MindTape] backlog runs:

```bash
mmz --is-fresh --tag gate
```

which passes only when every gate-tagged rule last succeeded with its inputs
unchanged. So "done" means the build actually passed against this worktree —
asserted, not claimed — and asserting it costs nothing, because the assertion is
a hash comparison rather than a re-run. Run `just check` to record a pass, then
flip; `--force` waives the gate and records the waiver.

= Where to go next

- #link(u("/cli/"))[CLI reference] — every action and every exit code.
- #link(u("/agents/"))[For AI agents] — the same gate, from an agent's side.
