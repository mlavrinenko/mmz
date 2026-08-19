#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "mmz: load and merge imported manifest fragments",
  priority: framework("ice", confidence: 0.8, ease: 3.0, impact: 7.0),
  tags: ("config",),
  links: (
    parent("mmz-config-composition-via-imported-fragments.typ")[the settled
      design this implements]
  ),
  status: proposed(2026, 8, 20),
)

== Summary

Teach `Manifest::load` to follow a top-level `imports:` list and merge what it
finds into one model, carrying each entry's source file. The chokepoint task:
the fragment schema, `--status`'s `source` field, `mmz --dump-config`, the docs
page and the dogfood split all sit on top of this.

Read the parent task first — every rule below is a decision made there, not a
choice to re-open while implementing.

== Surface

```yaml
imports:
  - .mmz/conf.d/                       # directory: every *.yaml / *.yml inside
  - shared/gates.yaml                  # relative to THIS file's directory
  - /nix/store/…-wormfork-mmz/rules.yaml
```

- Directory entry expands to the `*.yaml` and `*.yml` files directly inside it
  (not recursive), sorted lexically by file name. An empty declared directory
  is fine; a missing one is an error.
- Relative paths resolve against the directory of the file that names them, not
  the project root. Globs, `outputs` and `cache_dir` keep resolving against the
  project root, unchanged.
- Absolute paths are allowed with no special casing.
- A fragment may declare `imports:` itself.

== Merge rules

- `scopes`, `probes`: merge by key. A key present in two files is an error
  naming the key and both files.
- `commands`: the importing file's own rules first, then each import in listed
  order, depth-first. A `name` present in two files is an error naming both;
  within one file the existing `DuplicateCommand` error already covers it.
- `cache_dir`, `gitignore`, `strict`, `on_hit`: root manifest only. Any of them
  in an imported file is an error naming the key and the file. A command's own
  `on_hit` is a `Command` field and stays legal anywhere.
- `imports` itself never appears in the merged model — it is consumed by the
  loader.

== Validation order

Syntactic per file, semantic once on the merged model. Concretely: each file
deserializes on its own (`deny_unknown_fields` still applies, and the
policy-key rejection happens here), and `Manifest::validate` runs exactly once,
against the merge. A fragment naming a scope a sibling defines is valid.

== Cycles and diamonds

Canonicalize every path before recording it. A path already on the current
import stack is a cycle — error with the whole chain, root first. A path
already loaded but not on the stack is a diamond: skip it, load once, no error.
Canonicalizing (rather than comparing literal paths) is what makes a symlinked
`conf.d` entry and a store path behave.

== Errors

New variants in `src/error.rs`, all reported at exit code 4 with the rest of
the manifest-invalid family:

- `ImportMissing` — path named by an import does not exist, naming the
  importing file and the resolved path.
- `ImportNotReadable` and a parse failure inside a fragment must name the
  *fragment*, not the root manifest. Today `Error::ManifestParse` carries a
  path; check it is the one the reader needs.
- `ImportCycle` — the chain, root first.
- `DuplicateScope`, `DuplicateProbe` — key plus both source files.
- `DuplicateCommand` gains a cross-file spelling naming both files. Keep the
  single-file message as it is; a user with no imports must see no change.
- `FragmentPolicyKey` — key plus the fragment that set it, and the sentence
  that it may only be set in the root manifest.

== Provenance

The merge has to know which file each entry came from to produce those errors
at all, so keep it rather than discarding it after the check. Whatever shape it
takes (a `Sourced<T>` wrapper or a side map keyed by name), it must survive into
`Located` so `--status` and `--dump-config` can read it, and it must record the
root manifest as the source of its own entries — a single-file project is the
degenerate case, not a special case.

Paths in provenance should display project-root-relative when they are under
the root, and absolute otherwise, so a store path stays recognisable.

== Files

`src/manifest.rs` is 468 lines against a 500-line Rust cap, so the loader does
not go in it. Add `src/compose.rs` (import resolution, cycle detection, merge)
plus `src/compose_tests.rs`, and have `Manifest::load` call into it. Read
`.linecop.yaml` before writing rather than splitting after the gate trips.

Gate-coupled, and inside this task's boundary:

- `schema/mmz.schema.json` gains the `imports` property.
- `www/utils/config-notes.typ` gains an `imports` entry — `just check-doc-facts`
  asserts the schema's property set and the notes' key set match exactly in
  both directions, so the schema edit alone fails the gate.
- `src/schema.rs`'s `schema_documents_every_manifest_field` lists the keys it
  expects; add `imports`.

== Tests

- A fragment's scopes, probes and commands all reach the merged model.
- Command order: host rules precede imported ones; two imports keep list order;
  a nested import lands depth-first.
- Duplicate scope, probe and command across files each error naming both files.
- A policy key in a fragment errors; the same key in the root does not.
- A fragment referencing a scope its sibling defines validates.
- A fragment that is invalid *alone* but valid merged is accepted; a merge that
  is invalid is rejected even when every fragment is valid alone.
- Cycle errors with the chain; a diamond loads once and does not error.
- Missing import file and missing import directory both error naming the path.
- An empty import directory is accepted.
- A directory entry sorts lexically and ignores non-YAML files.
- Relative paths resolve against the importing file, proven by a nested
  fragment one directory down importing a sibling.
- An absolute path outside the project root loads.
- A manifest with no `imports:` key behaves exactly as before, including every
  existing error message.
