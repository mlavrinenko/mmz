#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "mmz: dump the merged manifest with provenance",
  priority: framework("ice", confidence: 0.8, ease: 5.0, impact: 6.0),
  tags: ("config", "cli"),
  links: (
    parent("mmz-config-composition-via-imported-fragments.typ")
      + depends-on("mmz-load-and-merge-imported-manifest-fragments.typ")
  ),
  status: wip(2026, 8, 20)[wave 3],
)

== Summary

A new action printing the manifest mmz actually assembled, with the source file
of every scope, probe and command. Two readers: a person asking "which file
made this rule skip?", and a generator asserting in a gate that the fragment it
emitted is the one in effect.

== Surface

```
mmz --dump-config                 print the merged manifest and where each entry came from
mmz --dump-config=json            the same as JSON
```

Spelled `--dump-config`, not `--config`: `--config` stays reserved for the flag
that relocates the manifest, which the parent task lists as a separate,
deliberately deferred feature. Shipping `--config` as a reader and later
wanting it as a locator is a rename nobody gets to take back.

The human form leads with the source list, in load order, so the import graph
is visible before the entries are:

```
sources:
  1  .mmz/config.yaml
  2  .mmz/conf.d/10-rust.yaml
  3  /nix/store/…-wormfork-mmz/rules.yaml
```

then each section's entries with the source they came from. The JSON form
carries the same facts under stable keys — a `sources` array and a `source` on
every scope, probe and command — because the gate hook is half the point.

Exit 4 (manifest missing or invalid) when the manifest does not load, like
every other reader. `--dump-config` prints the merged model *after* validation;
it is not a debugging aid for a manifest that fails to merge, and the merge
errors already name both files.

== Files

New `src/dump.rs` beside `src/status.rs`, plus its tests. `src/main.rs` gains
the usage lines and dispatch.

== Gate-coupled, and inside this task's boundary

`just check-doc-coverage` requires a `www/utils/cli-notes.typ` entry for every
token in `mmz --help`'s `Usage:` block, in both directions. Both new lines need
entries — `--dump-config` and `--dump-config=json` are separate keys, the same
way `--status` and `--status=json` are.

Do not run this on the same wave as the fragment-schema task: both edit the
`USAGE` block in `src/main.rs` and both edit `cli-notes.typ`.

== Tests

- A single-file project dumps with itself as the only source.
- A composed project attributes every scope, probe and command to the right
  file, including through a nested import.
- The JSON form round-trips through `serde_json` and carries `sources` in load
  order.
- A store-path fragment shows its absolute path; a fragment under the root
  shows a root-relative one.
- An invalid manifest exits 4 with the merge error, printing no partial dump.

== Deferred

`--dump-config=json-schema`, and a `schema/config-dump.schema.json` beside it.
`--status` has that arm, so the symmetry is obvious — but it is a third action,
a third `cli-notes` entry and a fourth schema file for a document whose only
consumer today is a gate that can assert on keys directly. File it when a
second consumer appears.
