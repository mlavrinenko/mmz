#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "mmz: ship a fragment JSON Schema",
  priority: framework("ice", confidence: 0.9, ease: 6.0, impact: 5.0),
  tags: ("config", "cli"),
  links: (
    parent("mmz-config-composition-via-imported-fragments.typ")
      + depends-on("mmz-load-and-merge-imported-manifest-fragments.typ")[the
        `imports` property has to exist in the base schema first]
  ),
  status: proposed(2026, 8, 20),
)

== Summary

An imported fragment has a narrower legal surface than the root manifest —
`cache_dir`, `gitignore`, `strict` and `on_hit` are root-only — so validating a
fragment against `schema/mmz.schema.json` accepts documents mmz rejects. Ship a
second schema for fragments and a way to print it, so a generator and an editor
both validate against what the loader actually enforces.

This is the whole reason the root-only policy decision is affordable: the
narrower surface is discoverable rather than folklore.

== Surface

`schema/mmz-fragment.schema.json`, embedded with `include_str!` beside
`SCHEMA`, and printed by a new action:

```
mmz --schema                      print the config JSON Schema
mmz --schema=fragment             print the JSON Schema for an imported fragment
```

The `=`-suffixed spelling follows `--status=json-schema`, which is the existing
precedent for a second document out of one action.

Fragments carry the same pinned-tag `$schema` URL shape the root manifest does,
pointing at the fragment schema:

```yaml
# yaml-language-server: $schema=https://raw.githubusercontent.com/mlavrinenko/mmz/vX.Y.Z/schema/mmz-fragment.schema.json
scopes:
  linecop-config: [".linecop.yaml"]
```

== Keeping the two schemas from drifting

The fragment schema is the config schema minus the four policy properties.
Do not generate it through a new gate — a generate-then-diff arm costs the
three-place gate wiring plus a `www/utils/gate-notes.typ` entry, for one file.

Assert the relationship in a Rust test in `src/schema.rs` instead: parse both
embedded documents and check that the fragment's property set is exactly the
config's minus `cache_dir`, `gitignore`, `strict` and `on_hit`, that every
shared property is byte-identical, and that both keep
`additionalProperties: false`. That runs under `just test`, wires nothing, and
fails on either direction of drift.

== Gate-coupled, and inside this task's boundary

`just check-doc-coverage` reads the action list out of `mmz --help`'s `Usage:`
block and requires a `www/utils/cli-notes.typ` entry for every token it finds,
in both directions. Adding the usage line without the note fails the gate; so
does a note for an action the binary does not advertise. Add the
`--schema=fragment` entry in the same change.

`just check-doc-facts` covers the *config* schema's properties against
`config-notes.typ`. The fragment schema's properties are a subset, so it needs
no new notes — confirm the gate script globs only `schema/mmz.schema.json`
before assuming it.

== Tests

- `mmz --schema=fragment` prints valid JSON and exits 0.
- The derivation test above, in both directions.
- The fragment schema rejects each of the four policy keys and accepts
  `scopes`, `probes`, `commands` and `imports`.
- `mmz --schema` output is unchanged.

== Note

`mmz --init` scaffolds a root manifest only. Whether it should learn to
scaffold a `.mmz/conf.d/` fragment is a separate question, and not obviously
yes — a generator writing a fragment does not need a template.
