#import "../utils/page.typ": page
#import "../utils/ui.typ": callout
#import "../utils/site.typ": u

#let meta = (
  route: "/composition/",
  label: "Composition",
  title: "Composing a manifest from imports",
  summary: "Why imports: merges files instead of overriding them, the one path rule that trips everyone the first time, and how to debug what a composed manifest actually resolved to.",
)
#metadata(meta) <page-meta>

#show: page.with(..meta)

`mmz` reads one manifest, but that manifest does not have to be one file. A
top-level `imports:` list pulls scopes, probes and commands out of other YAML
files and folds them into the one manifest `mmz` actually runs against:

```yaml
# .mmz/config.yaml
imports:
  - conf.d/                            # every *.yaml/*.yml inside, lexical order
  - /nix/store/…-wormfork-mmz/rules.yaml

scopes:
  rust: ["src/**/*.rs"]
```

An entry names a file, a directory (expanded to the `*.yaml`/`*.yml` files
directly inside it, sorted lexically — a file nobody named in `imports:` never
changes behaviour, even if it sits right there in the directory), or an
absolute path, store paths included, used exactly as written. A fragment may
import too, and the chain is followed depth-first; the same file reached twice
by different routes loads once rather than erroring, because a diamond is not a
cycle. For the full key syntax see #link(u("/manifest/"))[the manifest
  reference] — this page is the argument for the shape, not a restatement of it.

= Why a duplicate key is an error, not last-wins

`scopes` and `probes` merge by key; `commands` merge by `name`. Declare the same
key in two files and `mmz` refuses to guess which one you meant:

```
mmz: scope `rust` is declared in both .mmz/config.yaml and .mmz/conf.d/lint.yaml
```

Last-wins was the obvious alternative, and it was refused on purpose. Picture
the case it exists for: a generator drops a fresh `rules.yaml` naming a `rust`
scope, and the project's own `.mmz/config.yaml` still has an older hand-written
`rust` scope left over from before the generator existed. Last-wins silently
picks one — and whichever file happened to load last quietly wins, with nothing
printed and nothing to notice. If the file that lost was the regenerated one,
every rule depending on `rust` is now checked against stale globs, and `mmz`
reports fresh with a straight face. That is a false green: the exact failure
this tool exists to catch, produced by the tool itself.

#callout("warn")[
  Erroring is also the only forward-compatible choice. Loosening an error into
  an override later is additive; the reverse — turning a working override into
  an error — breaks whoever depended on it. Ship the strict form first.
]

Ordering still exists — see below — but it decides which rule *matches an
invocation first*, never which of two same-named rules survives. There is no
same-named survivor.

= The path asymmetry

Two different things in a fragment resolve against two different directories,
and mixing them up is the mistake almost everyone makes once:

- `imports:` paths resolve against the *importing file's own directory*.
- Globs (`scopes[].globs`) and `commands[].outputs` resolve against the
  *project root*, exactly as they always have.

Worked example. A `conf.d` directory sits beside `.mmz/config.yaml`, so the path
everyone reaches for first is `.mmz/conf.d/` — it is, after all, where `conf.d`
lives relative to the project root:

```yaml
# .mmz/config.yaml
imports:
  - .mmz/conf.d/
```

That is wrong, and `mmz` says so by naming the path it actually resolved:

```
mmz: import in .mmz/config.yaml names `.mmz/.mmz/conf.d/`, which does not exist
```

`.mmz/config.yaml` already sits inside `.mmz`, so its own directory *is*
`.mmz`— resolving `.mmz/conf.d/` against it lands on `.mmz/.mmz/conf.d/`, one
level too deep. The importing file's directory is the base, not the project
root, so the entry wants no `.mmz/` prefix at all:

```yaml
# .mmz/config.yaml
imports:
  - conf.d/
```

Now put the fragment beside it and watch the other half of the asymmetry:

```yaml
# .mmz/conf.d/lint.yaml
scopes:
  rust-tests: ["tests/**/*.rs"]
```

`rust-tests` is *not* `.mmz/conf.d/tests/**/*.rs`, even though the fragment
declaring it lives three directories deep. The glob resolves against the
project root exactly as it would in the root manifest — a fragment cannot
express globs about itself, only about the project it was imported into. Both
rules are visible in the same two files at once: the import path that named
this fragment is file-relative, the glob written inside it is root-relative.

#callout("note")[
  `imports:` is the only path in a manifest that is ever file-relative. Every
  other path — a scope's globs, a command's `outputs`, a probe's `run` line
  interpreted by the shell — stays project-root-relative, same as before
  composition existed.
]

= What a fragment may say

A fragment declares rules: `scopes`, `probes`, `commands`, and its own
`imports:`. It may not set `cache_dir`, `gitignore`, `strict` or `on_hit` —
those four govern the whole run, not one rule, and setting one in a fragment is
a load error naming the key and the file. Project policy stays with the
project; there is no precedence to learn because there is nothing to override.

Validate a fragment on its own with `mmz --schema=fragment`; the shape is
narrower than the full manifest schema precisely by those four keys.

= Order: host rules first, then imports, depth-first

Duplicate names are always an error, but *order* still decides which of two
non-duplicate rules matches an invocation first when their names overlap as
token prefixes (see #link(u("/matching/"))[matching]). The importing file's own
`commands` are tried before any import, and imports are tried in listed order,
depth-first. The project's own rules get first look at every invocation;
nothing generated can shadow them. This only matters when two rules' names
overlap — most manifests never hit it.

= Debugging a composed manifest

`mmz --dump-config` prints the merged manifest with the source file of every
scope, probe and command — the answer to "which file actually produced this
rule?" for a project built from several files:

```
sources:
  1  .mmz/config.yaml
  2  .mmz/conf.d/lint.yaml

policy:  # .mmz/config.yaml
  gitignore: true  (default)
  cache_dir: .mmz/cache  (default)
  strict: [no_match, no_inputs]  (default)
  on_hit: (none)  (default)

scopes:
  rust:  # .mmz/config.yaml
    globs: [src/**/*.rs]
  rust-tests:  # .mmz/conf.d/lint.yaml
    globs: [tests/**/*.rs]

commands:
  ./bin/build.sh:  # .mmz/config.yaml
    match: prefix
    inputs: [rust]
  ./bin/lint.sh:  # .mmz/conf.d/lint.yaml
    match: prefix
    inputs: [rust-tests]
```

`sources` is numbered in load order — root first, then each import depth-first
— so the import graph is visible before the entries it fed. Every scope, probe
and command carries a trailing `# <file>` naming exactly where it came from,
and the four policy keys are always resolved (defaulted ones marked so) since
they govern the run whether or not any file wrote them.

`--status` carries the same fact per rule, one column, alongside its freshness:

```
RULE            SOURCE                 STATE  AGE
./bin/build.sh  .mmz/config.yaml       fresh  0s ago
./bin/lint.sh   .mmz/conf.d/lint.yaml  fresh  0s ago
```

`--status=json` reports the same `source` field per rule, so a script can ask
"is the rule that just went stale one of mine, or one a generator wrote?"
without shelling out to `--dump-config` first.

= Generated fragments, today

The reason this exists: a generator that emits gate wiring can drop one
fragment naming its own scopes and rules without touching the rest of the
manifest, and the project keeps its own hand-written rules right beside it.
`imports:` is what makes that additive rather than all-or-nothing — the
generator owns its file, the project still owns everything else.

No such generator ships with `mmz` today. This is the seam it will plug into,
not a feature with a first user yet.

= Where to go next

- #link(u("/manifest/"))[Manifest reference] — every key, including `imports:`,
  generated from the schema.
- #link(u("/matching/"))[Matching] — how order decides between two rules whose
  names overlap.
- #link(u("/cli/"))[CLI reference] — `--dump-config`, `--schema=fragment`, and
  every other action.
