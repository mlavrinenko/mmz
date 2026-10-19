#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "Exclude paths from a scope's or rule's inputs",
  tags: ("config", "cache"),
  priority: framework("ice", confidence: 0.8, ease: 6.0, impact: 7.0),
  status: done(2026, 10, 20),
)

== Summary

A gate's inputs cannot leave anything out: a scope is a list of globs, and mmz 0.12.1 has no way to subtract from it. In Mindtape, gates globbing `examples/**` hashed gitignored `.mindtape/cache/` files that another gate deleted mid-hash, which failed the run, and the only fix on offer was a `.gitignore` line. Any project whose gate globs over a tree another gate writes has the same problem, gitignored or not.

Add `exclude:`, a list of patterns, beside `globs` in a scope's object form and beside `inputs` on a command rule. A path is left out when it, or any directory above it, matches an exclude pattern, so an excluded tree is pruned during the walk: it is never listed in `--status=json`, never hashed, and never a `{file:scope}` domain entry. A scope's `exclude` applies to that scope's own patterns, literals included. A rule's applies to every file its scopes resolve to, its `{file:scope}` domain included. A manifest with no `exclude` resolves exactly as before.

Why a key and not gitignore-style `!pattern` negation: YAML reads an unquoted leading `!` as a tag, so an unquoted negation is a parse error or a silent tag rather than a pattern. globset, which compiles every glob mmz reads, has no negation, so `!` would mean a new ordered-pattern grammar. And a rule's scopes share one walk, so an inline negation in one scope would leak into its siblings. The object form already carries a per-scope key, `gitignore`, and `exclude` sits beside it the same way.

Done when tests cover a manifest with no exclusions resolving unchanged, an exclusion inside a recursive glob, a rule-level exclusion, and an excluded directory mmz cannot read. That last one stands in for a file created and deleted between listing and hashing, a race no test can stage: a path mmz never opens can neither vanish nor refuse it. The schema, the inputs and manifest pages, the notes and the CHANGELOG describe the key.
