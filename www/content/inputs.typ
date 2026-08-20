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

A `probes:` entry closes both gaps. The shape to reach for first reads a file
and selects out of it with no process at all:

```yaml
probes:
  nixpkgs-input:
    file: flake.lock
    json: '.nodes["nixpkgs"]["locked"]["narHash"]'

commands:
  - name: nix flake check
    inputs: [nixpkgs-input]
```

A real `flake.lock` holds a hundred nodes, so a scope naming it makes every rule
depend on every input. The probe above depends on one, and reaching it costs a
file read: no shell, nothing on `PATH`, no process per rule on an
`mmz --is-fresh` that gates the whole manifest at once. `json:` is jq — a
compatibility choice rather than a power one, since probes written before this
key existed already use `,` and `with_entries(select(…))`, and a manifest key
must not change meaning in a later version.

`run:` is the other source — a command line whose stdout is the bytes — and it
may carry a `json:` too, selecting out of that stdout instead of a file:

```yaml
probes:
  fmt-recipe:
    run: just --dump --dump-format json
    json: '.recipes["fmt-check"]'
```

The two sources are mutually exclusive: a probe declaring both is a manifest
error, not a precedence rule to memorise. A `file:` with no `json:` is refused
too — hashing a whole file is what a scope is for, and a scope keeps the
gitignore filter and reports which file moved.

== Read this before reaching for one

#callout("warn")[
  A wrong scope costs time. A wrong probe can lie.
]

Over-declaring a scope buys an unnecessary re-run — harmless. A probe that
reports the wrong bytes buys a wrongly _fresh_ rule, which is the failure `mmz`
exists to prevent. So every way a probe can fail visibly is a hard stop: exit 6,
nothing consumed, no record written.

- A probe that exits non-zero, or cannot be spawned at all, names the probe, its
  exit code and its stderr.
- A `file:` that is missing or unreadable names the probe and the path.
- Bytes that are not exactly one JSON value are refused wherever they came from:
  a tool that logged a line before its JSON is a state `mmz` will not hash.
- Empty stdout is an error, and so is a `json:` selection that measured nothing.
  `allow_empty: true` opts into either.

That last one is load-bearing. `.a.b.c` against a document lacking them is not a
failure in jq — it is a successful selection of `null`, and a probe tracking
`null` reports one digest whatever the document does. The rule would read fresh
forever against an input nobody is measuring, which is precisely what `jq -e`
prevents in a shelled-out probe. `false` is a value and passes: jq conflates it
with `null` only because a shell exit code cannot tell them apart.

== Key order is not an input

`mmz` hashes its own rendering of the selected value — object keys sorted at
every depth, array order preserved — never the bytes it read. Two tool versions
that emit the same recipe with its keys in a different order produce one digest.

The alternative is a convention: a probe piping through `jq` hashes whatever the
renderer chose, so an author who forgets `-S` ships a rule that busts on a tool
upgrade — which happened here. Content correctness is still
#strong[yours, not mmz's] — a selector naming the wrong field is a manifest bug
`mmz` cannot see — but key order has stopped being one of the ways to get it
wrong.

== Mechanics

`run` is executed by `sh -c` from the project root — the directory holding
`.mmz` — with stdin closed, so a probe waiting on input fails instead of hanging
a gate, and with stderr captured for the failure message. A `file` path resolves
against that same root, the base scope globs use, and is read directly: the
`gitignore` filter never applies, since a probe names one file explicitly.

== Which environment a probe measures

This section is about `run` alone. A `file` probe measures the repository and
has no environment to get wrong, which is most of why it is the shape to prefer.

`sh -c` is only the default. A `run` line resolves its commands through whatever
`PATH` the caller had, which makes the caller's shell part of what the probe
reports — and a probe is supposed to report the project. The same probe run
inside a project shell and outside it can disagree about a tool's version, and
the disagreement surfaces as an unexplained stale rule rather than as an error,
because a digest that moved is indistinguishable from a digest that should have.

`probe_shell` pins the argv the line is handed to, so the answer stops depending
on where mmz was invoked from:

```yaml
probe_shell: ["direnv", "exec", ".", "sh", "-c"]

probes:
  fmt-recipe:
    run: just --dump --dump-format json
    json: '.recipes["fmt-check"]'
```

The first element is the program and the rest are fixed arguments; the `run`
line is appended as one final argument. Every `run` line in the manifest goes
through it untouched, and `["nix", "develop", "--command", "sh", "-c"]` works
the same way. It is root-manifest-only — an imported fragment setting it would
leave undecidable which one governs a probe declared in a third file — and an
empty list is a load error, since there would be nothing to spawn.

#callout("note")[
  This pins the environment; it does not make mmz aware of it. A probe measured
  under the wrong shell is still a probe mmz will trust. What the key buys is
  that there is no longer a wrong shell to be measured under.
]

A probe is resolved #strong[once per invocation] however many rules name it, so
eighteen rules sharing one probe cost one read or one process. That shape
matters: a bare `mmz --is-fresh` gates every rule at once and runs in git hooks.
A declared probe that no rule names is never resolved at all.

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
