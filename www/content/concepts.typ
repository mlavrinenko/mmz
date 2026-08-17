#import "../utils/page.typ": page
#import "../utils/ui.typ": callout, listing, transcript
#import "../utils/site.typ": u

#let meta = (
  route: "/concepts/",
  label: "Concepts",
  title: "Concepts",
  summary: "The model behind mmz: rules, records, freshness, and the asymmetry that decides every design question.",
)
#metadata(meta) <page-meta>

#show: page.with(..meta)

= A rule is a claim

Everything in `mmz` is built from one object. A rule names a command and the
inputs that command depends on:

```yaml
commands:
  - name: cargo test
    inputs: [rust]
```

When a wrapped invocation matches that rule and succeeds, `mmz` writes a record.
The record is a claim, and it is worth reading literally:

#quote(block: true)[
  This command exited 0 while its inputs hashed to H.
]

Freshness is that claim still holding. For a verdict command — `fmt --check`, a
linter, a test suite — it holds for exactly as long as H holds, because the
command produced nothing but a verdict. For a command that _produces_ something,
the claim carries a side effect, and the effect can be undone without touching a
single input. That is the second way a record stops being valid, and why
#link(u("/outputs/"))[declared outputs] exist.

= What a record looks like

#listing("record.yaml", lang: "yaml")

One YAML file per rule, in a git-ignored cache directory, written atomically
(temp file plus rename) so a crash or a concurrent writer never leaves a
truncated record. Derived, throwaway state: delete the whole directory and the
only cost is one honest re-run.

A record is fresh only when its `status` is `ok`, its content digest, format,
algorithm and command all still match, and every output its rule declares is
still on disk. Anything else re-runs.

= Live resolution, every time

There is no watcher and no index to maintain. Every invocation re-resolves the
matched rule's scopes against the filesystem, hashes what it finds, and compares.
The state on disk is the only state.

- Edits by hand, by script, by a rebase, or by another tool are all equal — the
  next run sees them.
- The cost is per-invocation resolution, not a background process.
- Nothing to invalidate, because nothing was cached but the answer.

= The governing asymmetry

#callout("warn")[
  Under-declaring a rule's inputs makes `mmz` skip a command that should have
  run — a false green. Over-declaring buys an unnecessary re-run. These are not
  comparable mistakes.
]

Every design decision here falls out of that. A rule's scopes must be a superset
of every file any matching invocation could depend on. When in doubt, broaden.

Toolchain sensitivity is modelled as an ordinary input rather than as ambient
magic: put `rust-toolchain.toml` or `flake.lock` in a scope and a toolchain bump
busts the cache. `mmz` trusts file content, not the environment it happens to be
running in — and a #link(u("/inputs/"))[probe] only shifts who is trusted, from
a file's bytes to a command's stdout, which is why a probe's content is the
manifest author's to get right.

= Fail closed

`mmz` errors rather than guessing:

- No manifest found, or an invalid one — always an error, never a passthrough.
- No rule matches the invocation.
- A matched rule's inputs resolve to zero files.

The last two are relaxable per project with `strict`, and then the invocation
runs unmemoized instead of stopping. The first is not relaxable at all. What
`mmz` never does, under any setting, is skip a command whose inputs it has not
confirmed unchanged.

`no_inputs` deserves the suspicion its default gives it. A rule that resolves to
nothing is rarely a rule that genuinely has no inputs; it is usually a scope
whose glob stopped matching — a directory renamed, or a path that turned out to
be git-ignored. Left as a warning, such a rule reports fresh forever.

= Identity is the rule, not the argv

The cache identity is the matched rule's `name`, not the full command line. So
`cargo test` and `cargo test --workspace` share one record when one rule matches
both.

That is a knob, not an oversight: you control granularity by how specifically
rules are written. Split a rule, or narrow it with `match: exact`, when one rule
would conflate invocations that do genuinely different work. See
#link(u("/matching/"))[Matching] for the rules of the match itself.

= What is trusted

#table(
  columns: 2,
  table.header([Trusted], [Not trusted]),
  [The bytes of every file a rule's scopes resolve to],
  [Timestamps, inode numbers, file ordering],

  [The exit status of the wrapped command], [Its stdout or stderr],
  [The stdout of a declared probe], [That the probe's output _means_ anything],
  [The existence of a declared output], [The contents of that output],
)

The right-hand column is not a backlog. Hashing an output would buy tamper
detection and nothing else — the input digest already proves an existing artifact
is the one those inputs produced — and inferring dependencies by tracing syscalls
would trade an explicit, reviewable declaration for a heuristic that fails
silently on the platform you did not test.
