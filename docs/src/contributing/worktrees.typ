// docs/src/contributing/worktrees.typ — source for docs/contributing/worktrees.md.
// Rendered to Markdown by docs/generate-md.sh via typlite.
//
// Split out of docs/src/contributing.typ, which sits at the Typst line cap: the
// process document keeps the two commands, this file the hooks behind them.
#import "../lib.typ": doc-title

#metadata((
  output: "docs/contributing/worktrees.md",
)) <mmz-md>

#doc-title[Worktrees]

Work in parallel in a sibling worktree. `just fork` makes one and `just land`
merges it back; both wrap #link("https://worktrunk.dev")[worktrunk] (`wt`, from
the dev shell) with the flags this repo needs, because a bare `wt merge` squashes
away each commit's `Refs:` footer and commits a dirty tree.

```bash
just fork <slug>           # ../mmz.<slug> on a new branch
just fork <slug> --base=@  # the same, from the current commit, not main
just land                  # in the worktree: gate, commit rules, fast-forward, remove
```

A worktree only gets commits, never your working tree, so commit first. Do not
`git stash` in one: all worktrees share a single stash list.

`.config/wt.toml` holds the hooks, so a clone with nix and direnv needs no
worktrunk config of its own. A new worktree gets its `.envrc` allowed. Before the
fast-forward the hooks run `just check` and `mt check --commits main..HEAD`,
which refuses a branch whose commits break the MindTape rules, such as one that
names a task still `wip`. As the worktree goes away they import its gate records
into the primary checkout (`mmz --import`), so the next `just check` there
skips every arm that already passed.
