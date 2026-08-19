#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "mmz config composition via imported fragments",
  priority: framework("ice", confidence: 0.7, ease: 4.0, impact: 7.0),
  tags: ("config",),
  links: (
    depends-on("mmz-load-and-merge-imported-manifest-fragments.typ")[the loader
      everything else builds on]
      + depends-on("mmz-ship-a-fragment-json-schema.typ")
      + depends-on("mmz-report-each-rule-s-source-file-in-status.typ")
      + depends-on("mmz-dump-the-merged-manifest-with-provenance.typ")
      + depends-on("mmz-document-manifest-composition.typ")
      + depends-on("mmz-dogfood-imports-in-the-project-manifest.typ")
      + related("mmz-the-manifest-does-not-feed-any-cache-digest.typ")[noticed
        while settling this design]
  ),
  status: accepted(2026, 8, 20),
)

== Summary

mmz reads exactly one manifest — the nearest `.mmz/config.yaml`, found by
searching upward — and offers no way to assemble it from parts. Every scope,
command and tag must be hand-written into that single file. A tool that wants
to contribute rules has to own and rewrite the whole manifest, clobbering
whatever the project wrote by hand.

Add composition: a top-level `imports:` list pulls scopes, probes and commands
out of other YAML files, so generated rules and hand-written rules coexist in
one project.

```yaml
# .mmz/config.yaml
imports:
  - conf.d/                            # every *.yaml inside, lexical order
  - /nix/store/…-wormfork-mmz/rules.yaml

scopes:
  rust: ["src/**/*.rs"]
```

== Why

The rules in a `.mmz/config.yaml` restate a fact that already lives in the
`Justfile`: which recipes are gates, and what each one reads. cratemplate keeps
the two in sync by hand and had to add a validation gate for the failure —
an arm of `just check` with no matching rule — because they desync on their
own. The same fact is restated a third time in the CI job list.

Collapsing that to one declaration means a generator emits the `linecop` rule
and its scope whenever the linecop gate is enabled. That is only possible if
mmz can merge a generated fragment with the project's own manifest. Without
composition the generator must own the entire file, which forbids any
hand-written rule beside it and makes adoption all-or-nothing.

This is worth having on its own terms: a workspace with several crates that
share a gate set hits the same wall today.

== The design, settled

Every open question below was decided against the same rule the rest of mmz
answers to: under-declaring a rule's inputs is a false green and dangerous,
over-declaring costs a re-run and nothing else. Wherever composition could
silently drop, replace or narrow a rule, it errors instead.

=== Spelling: `imports:`, with directory entries

A top-level `imports:` list of paths, in order. An entry naming a directory
expands to every `*.yaml` and `*.yml` directly inside it, lexically sorted —
which buys the drop-in ergonomics of a `conf.d` convention while keeping the
directory *declared*. Reading rules out of a directory nobody named contradicts
"a missing manifest always errors"; a stray file must never change behaviour.

Named `imports`, not `extends`: `extends` promises override semantics, which
the next decision deliberately refuses.

=== A duplicate key is a hard error

`scopes` and `probes` merge by key, `commands` by `name`. A key declared in two
files is an error naming both sources — never last-wins, never first-wins. A
stale hand-written rule silently replacing a regenerated one is exactly the
false green this tool exists to prevent, and the failure would be invisible.

This is also the forward-compatible direction: error to override is additive,
override to error is breaking.

=== Ordering: host first, then imports depth-first

Duplicate names are rejected above, but rule order still decides which of two
overlapping token-prefixes wins. The importing file's own `commands` come
first, then each import in listed order, depth-first. The project's own rules
get first crack at an invocation; nothing generated can shadow them.

=== Nesting, paths, missing files

A fragment may itself import. Cycles are detected over canonicalized paths and
error with the whole chain; the same file reached twice by different routes
loads once rather than erroring, because a diamond is not a cycle.

Import paths resolve relative to the *importing file's* directory. This is the
one resolution rule under which a store fragment can reference a sibling store
fragment at all. Note the asymmetry loudly in the docs: import paths are
file-relative, while globs and outputs stay project-root-relative as they are
today — a fragment in the store cannot express globs about itself anyway.

The asymmetry has one sharp edge, and this task's first draft fell straight on
it: the common case is a `conf.d` beside the config, and the path everyone
*thinks* in is `.mmz/conf.d/`. From inside `.mmz/config.yaml` that resolves to
`.mmz/.mmz/conf.d/` and errors. The error names the resolved path, so it fails
closed and reads clearly — but every example anyone copies has to say
`conf.d/`, and the docs page owes this a worked example rather than a sentence.

Absolute paths, store paths included, are allowed with no special casing. A
tool that already runs `probes` is not newly exposed by reading a YAML file
outside the project root.

A missing import — file or directory — is a hard error naming the path. An
empty declared directory is fine.

=== A fragment declares rules, not policy

`cache_dir`, `gitignore`, `strict` and `on_hit` may only be set in the root
manifest; any of them in an imported file is an error naming the key and the
file. Project policy stays with the project, and there are no precedence rules
to learn. The one hole this leaves — a root `gitignore: true` breaking an
imported artifact scope — is already closed by the per-scope `gitignore: false`
object form.

Because the fragment surface is narrower than the manifest's, fragments need a
schema of their own to validate against; that is its own task.

=== Validation splits syntactic from semantic

Each file is validated *syntactically* on its own: document shape, key types,
no unknown keys. Every *semantic* invariant — name resolution, uniqueness,
`inputs:` naming a declared scope or probe, a parametric `{scope}` existing —
is checked only against the merged model. A fragment may reference a scope a
sibling defines.

This costs nothing today: every top-level key is already `serde(default)`, so a
fragment is already a valid manifest document.

=== Provenance is carried, not reconstructed

The merge records the source file of every scope, probe and command, because
the duplicate-key error above cannot name both files without it. Having paid
for it, surface it: validation errors gain `at <file>`, `--status=json` grows a
`source` per rule, and `mmz --dump-config` prints the merged view with origins —
the answer to "which file made this rule skip?", and a gate hook a generator
can assert against.

== Deferred, on purpose

Each of these was considered and left out of the first cut. None is blocked by
the design above; all are additive.

- *Explicit override markers.* `extend: true` on a scope object (union with the
  imported one) and `override: true` on a command (replace the imported rule of
  that name). The object spelling for scopes already exists, so this costs no
  new syntax class. Ship it only when a real workspace hits the duplicate-key
  error and the right answer is genuinely "extend", not "rename".
- *Cross-file shadow detection.* Token-prefix shadowing between differently
  named rules is statically computable. Within one file it is a legitimate
  ordering tool; across files it is almost always an accident, so erroring on
  it would remove ordering as a thing anyone has to reason about. Left out
  because it changes what a valid single-file manifest means once inlined.
- *Optional imports.* A `{ path: …, optional: true }` entry form for a
  generated fragment that has not been written yet. Real, but fail-open; the
  downstream `strict.no_match` refusal already makes the consequence loud.
- *A `--config` flag to relocate the manifest.* Separable, and the reason
  `--dump-config` is spelled with a prefix: `--config` stays reserved for it.

== The work

Ordered by dependency. The loader is the chokepoint; everything else can run
beside its siblings once it lands.

+ `mmz-load-and-merge-imported-manifest-fragments.typ` — the loader, the merge,
  the errors, the provenance. Everything below depends on it.
+ `mmz-ship-a-fragment-json-schema.typ` and
  `mmz-report-each-rule-s-source-file-in-status.typ` and
  `mmz-dump-the-merged-manifest-with-provenance.typ` — parallel.
+ `mmz-document-manifest-composition.typ` — after the surface is final.
+ `mmz-dogfood-imports-in-the-project-manifest.typ` — last, because it is the
  end-to-end proof and it edits the manifest every gate reads.

Collision points to keep off the same wave: `src/main.rs` (the `USAGE` block)
and `www/utils/cli-notes.typ` are touched by both the fragment-schema and
`--dump-config` tasks; `www/content/manifest.typ` is touched by the docs task
and by anything adding a manifest key.

== Notes

Filed while designing wormfork, a language-agnostic project scaffolding tool
that would generate gate wiring (just recipes, mmz rules, CI jobs, devShell
packages) from one module declaration. mmz composition is one of its
prerequisites, but the feature stands alone if wormfork never ships.
