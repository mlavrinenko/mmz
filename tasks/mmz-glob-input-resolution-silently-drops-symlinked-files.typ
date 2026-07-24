#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "mmz: glob input resolution silently drops symlinked files",
  priority: ice(
    impact: 3,
    confidence: 0.9,
    ease: 6,
  ),
  tags: ("cache", "config"),
  status: proposed(2026, 7, 24),
)

== Summary

`src/resolve.rs` treats a symlinked source file inconsistently between the two
ways a scope can name it:

- Literal path (`inputs` glob = `link.rs`): `expand_literal` uses
  `base.join(pattern).is_file()`, which follows the symlink, so the file IS
  included in the input set.
- Glob (`inputs` glob = `*.rs` / `**/*.rs`): `expand_globs` walks with
  `follow_links(false)` and keeps only entries where
  `file_type().is_some_and(|k| k.is_file())`. A symlink's type is not `file`,
  so the symlinked source is silently dropped from the input set.

Reproduced on 0.5.0: a rule with `*.rs` omits a `link.rs -> real/target.rs`
symlink that the same rule keyed on the literal `link.rs` includes.

== Why

The common case is glob inputs (`**/*.rs`). If a project has a symlinked source
file, it never enters that rule's digest, so editing the symlink target never
busts the cache — a silent under-skip. That is exactly the asymmetry
`src/engine.rs`'s own docs say mmz protects against ("mmz never wrongly skips a
command it claims is fresh; the asymmetry it protects is silent under-skipping").
Niche (symlinked sources are uncommon), but the failure is silent and wrong, and
the glob-vs-literal split is surprising.

== Scope

- Decide the intended policy for symlinked inputs and make glob and literal
  resolution agree. Either follow symlinks to files in `expand_globs` (match the
  literal behaviour) or exclude them in both (and document it), rather than the
  current split. Follow-to-file is the safer default given the under-skip risk.
- Guard against symlink loops / escaping the project root if enabling
  `follow_links`.
- Regression test in `src/resolve.rs` asserting a symlinked file resolves the
  same way under a glob and a literal.

== Home

mmz's own backlog. Found in a critical review of input resolution.
