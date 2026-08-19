#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "mmz: dogfood imports in the project manifest",
  priority: framework("ice", confidence: 0.85, ease: 5.0, impact: 6.0),
  tags: ("config", "gating"),
  links: (
    parent("mmz-config-composition-via-imported-fragments.typ")
      + depends-on("mmz-load-and-merge-imported-manifest-fragments.typ")
      + depends-on("mmz-dump-the-merged-manifest-with-provenance.typ")[the
        before/after proof runs through it]
  ),
  status: proposed(2026, 8, 20),
)

== Summary

Split this repo's own `.mmz/config.yaml` across `.mmz/conf.d/` fragments and
import them. mmz memoizes its own checks already; composition is the one part
of the design that no test can tell you is pleasant to live with, and this repo
is the only project positioned to find out.

Last in the sequence on purpose: it edits the manifest every gate reads.

== The split

Root `.mmz/config.yaml` keeps policy and the import list — `on_hit`, and
whatever `strict`/`gitignore`/`cache_dir` it sets — plus the long comment about
why rules are named after recipes. Fragments carry rules, grouped so that a
reader looking for one gate knows which file to open:

- `.mmz/conf.d/10-rust.yaml` — the `rust`, `clippy-config`, `rustfmt-config`
  and `manifests` scopes, their recipe probes, and the `fmt-check`, `clippy`,
  `test` and `machete` rules.
- `.mmz/conf.d/20-docs.yaml` — the `docs-*` scopes and fixtures, their probes,
  and the `docs::check`, `docs::md-check`, `check-doc-coverage` and
  `check-doc-facts` rules.
- `.mmz/conf.d/30-repo.yaml` — `gate-scripts`, `linecop-config`,
  `outdatty-config`, and the `check-file-size` and `outdatty-check` rules.

Boundaries are a judgement call, not a decision the parent task made. What is
*not* a judgement call: no scope may be declared in two fragments, so a scope
several groups share (`rust`, `fixture`, `docs-src` all cross the lines above)
lives in exactly one file and the others reference it. Decide that placement
deliberately and say so in a comment, because the duplicate-key error will find
you otherwise.

Each fragment gets the fragment `$schema` line so editors validate it.

== The proof this did not change anything

The split must be behaviour-preserving. The cache digest is over resolved files
and probe stdout, and no rule takes `.mmz/**` as an input, so a correct split
leaves every existing cache record valid:

+ Run `just check` first, so every gate rule has a fresh record.
+ Capture `mmz --status=json` before the split.
+ Split.
+ `mmz --status=json` after must differ only by each rule's new `source`. Same
  rules, same digests, same freshness — no rule re-runs.
+ `mmz --is-fresh --tag gate` still exits 0 without running anything. If it
  does not, the split changed a rule's inputs and the diff says which.

Keep both captures in the task's closing note. A split that silently re-runs
the suite is a split that changed something.

== Also touched

`docs/src/contributing/gates.typ` says wiring a new gate touches three places
and names `.mmz/config.yaml` as the third. After the split it is "the right
fragment under `.mmz/conf.d/`", and the paragraph should say how to pick one.
That file is a source for `docs/contributing/gates.md`, so
`just docs::md-check` wants the regenerated Markdown in the same commit.

Check whether `www/generate-facts.sh` or `www/gates.jq` read `.mmz/config.yaml`
by path before assuming they do not.

== Definition of done

`just check` green, `mmz --is-fresh --tag gate` exits 0 without re-running, and
the before/after status capture is recorded on the task.
