#import "../utils/page.typ": page
#import "../utils/ui.typ": callout, listing, transcript
#import "../utils/site.typ": u

#let meta = (
  route: "/inputs/",
  label: "Inputs",
  title: "Inputs: scopes and probes",
  summary: "Named glob sets, the gitignore filter and how to opt one scope out of it, and probes for the inputs that are not files.",
)
#metadata(meta) <page-meta>

#show: page.with(..meta)

A rule's `inputs:` list names scopes, probes, or both. One namespace: a probe
sharing a name with a scope is a manifest error, so a reader never has to guess
which kind a name is, and an entry that is neither is refused at load.

= Scopes

A scope is a named set of globs, declared once and referenced by as many rules
as need it:

```yaml
scopes:
  rust: ["**/*.rs", "Cargo.toml", "Cargo.lock", "rust-toolchain.toml"]
  docs: ["docs/**/*.md"]

commands:
  - name: cargo test
    inputs: [rust]
  - name: mdbook build
    inputs: [docs]
```

Globs follow the common convention: `*` stays within a directory, `**` crosses
directories. A rule's file set is the union of its scopes' globs, resolved fresh
on every invocation.

Scopes exist so a shared input path lives in one place. When `rust` grows a
`build.rs`, every rule that pins it gets the fix at once.

= The gitignore filter

Glob expansion skips git-ignored paths by default. This is not a convenience —
it is what keeps build output out of an input set. Without it, a scope like
`["**/*.rs"]` would sweep up generated sources under `target/`, and every rule
naming it would bust on its own output.

Alongside it: `.git` is never traversed, and symlinks are not followed.
Explicitly listed literal paths are always kept, so naming one file directly
works even when a broad ignore rule would have covered it.

#callout("note")[
  A scope that resolves to _zero_ files is an error by default (`no_inputs`),
  precisely because the gitignore filter makes that easy to do by accident.
]

= Artifact scopes

Which raises the case the filter gets wrong. Sometimes a rule genuinely depends
on a build artifact — a coverage report feeding a threshold check, a compiled
bundle feeding a size gate. Artifacts live in git-ignored paths by definition, so
under the default filter such a scope expands to nothing, and a rule whose inputs
resolve to nothing is not an error you notice: it is a rule that reports fresh
forever.

Spell that one scope as an object and pin `gitignore` for it alone:

```yaml
scopes:
  src: ["src/**"]              # array form: inherits the manifest-level setting
  lcov:
    gitignore: false           # this scope only
    globs: ["target/coverage/lcov.info"]
```

Now the artifact is a tracked input: regenerate it, and the rule goes stale.

Keep the override at the scope that names the artifact. Flipping the
manifest-level `gitignore` instead would drag every sibling scope through
`target/` and any other ignored tree — slow, and a source of spurious cache
busts. There is no per-glob override: one knob, at the level a reader can see it.

A rule may mix both kinds freely; each scope is expanded under its own setting,
so the siblings keep filtering. Absent means inherit. An object without `globs`,
or with an empty `globs` list, is a manifest error.

= Probes: inputs that are not files

A scope can only name whole files, so a rule that depends on _part_ of a file has
to hash all of it — one recipe body in a `Justfile` busts every rule that pins
the `Justfile`. And some dependencies are not files at all: the toolchain
version, a resolved dependency set, the output of a tool that reports its own
configuration.

A `probes:` entry closes both gaps. `run` is a command line, its stdout is
hashed, and `inputs:` references the probe by name exactly as it references a
scope:

```yaml
probes:
  fmt-recipe:
    run: just --dump --dump-format json | jq -S -e -c '.recipes["fmt-check"]'

commands:
  - name: just fmt-check
    inputs: [rust, fmt-recipe]
```

Nothing else about the rule changes: the probe's digest joins the rest of its
input digest, so `just fmt-check` re-runs when its own recipe body moves and
ignores every other recipe in the same file.

== Read this before reaching for one

#callout("warn")[
  A wrong scope costs time. A wrong probe can lie.
]

Over-declaring a scope buys an unnecessary re-run — harmless. A probe that prints
the wrong bytes buys a wrongly _fresh_ rule, which is the failure `mmz` exists to
prevent. So every way a probe can fail visibly is a hard stop:

- A probe that exits non-zero is an error naming the probe, its exit code, and
  its stderr. `mmz` exits 6 without consuming the output and without writing a
  record — a failed command never reaches the hasher.
- A probe that cannot be spawned is the same error.
- Empty stdout is an error by default; `allow_empty: true` opts in. It is the
  cheapest catch there is for a selector that matched nothing.

Content correctness and determinism are #strong[yours, not mmz's]. A probe that
prints valid but wrong output, or that varies run to run, is a manifest bug and
`mmz` cannot see it. Pin the ordering, strip the timestamps, and assert the shape
_inside_ the probe so a bad shape becomes a non-zero exit:

```yaml
probes:
  fmt-recipe:                                   # note the -S and -e
    run: just --dump --dump-format json | jq -S -e -c '.recipes["fmt-check"]'
```

`jq -e` exits non-zero when its selector yields `null` or `false`, turning a
renamed recipe into a loud probe failure instead of a digest that quietly stops
tracking anything. `jq -S` is the ordering half of the same discipline: it sorts
object keys, so the digest tracks the selection's content rather than the key
order the renderer happened to pick. An unsorted hash of a JSON object is a
latent dependency on the tool that printed it — two `just` versions emit the
same recipe with the keys in a different order, which without `-S` reads as a
busted rule. `mmz` does not validate meaning, and will not learn to.

== Mechanics

`run` is executed by `sh -c` from the project root — the directory holding
`.mmz` — with stdin closed, so a probe waiting on input fails instead of hanging
a gate, and with stderr captured for the failure message.

A probe is resolved #strong[once per invocation] however many rules name it, so
eighteen rules sharing one probe cost one process. That shape matters: a bare
`mmz --is-fresh` gates every rule at once and runs in git hooks. A declared probe
that no rule names is never run at all.

A rule whose only input is a probe has inputs — it is memoized, not `no-inputs`.

== When a probe is what changed

`mmz --is-fresh` names it, rather than reporting a vague "inputs changed":

```
mmz: `just fmt-check` is stale (probe `fmt-recipe` changed since it last passed)
```

And `--status=json` reports every resolved probe's current digest under `probes`,
with what each record saw under `cached.probes` — so the two are diffable when
they disagree.

= Where to go next

- #link(u("/matching/"))[Matching] — how a rule is chosen, and how one rule can
  fan over a scope's files.
- #link(u("/outputs/"))[Declared outputs] — the other way a record stops being
  valid.
- #link(u("/manifest/"))[Manifest reference] — every key, generated from the
  schema.
