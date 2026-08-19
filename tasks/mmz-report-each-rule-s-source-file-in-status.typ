#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "mmz: report each rule's source file in status",
  priority: framework("ice", confidence: 0.85, ease: 6.0, impact: 5.0),
  tags: ("config", "cli"),
  links: (
    parent("mmz-config-composition-via-imported-fragments.typ")
      + depends-on("mmz-load-and-merge-imported-manifest-fragments.typ")[the
        merge is what carries the source]
  ),
  status: done(2026, 8, 20)[merged; just check green on the merged HEAD],
)

== Summary

Once a manifest can be assembled from several files, a merged view hides which
file a rule came from — which is exactly what someone debugging a surprising
skip needs. Surface the source the merge already carries in `mmz --status`.

== Surface

`--status=json`: each rule grows a `source`, the file that declared it, shown
project-root-relative when it is under the root and absolute otherwise.
`schema/status.schema.json` gains the property.

`--status` (the table): a `SOURCE` column, shown *only* when more than one file
contributed rules. A project with no `imports:` sees no new column and no
change to its output at all — the field's cost has to land on the people using
the feature.

== Files

`src/status.rs` is 494 lines against a 500-line Rust cap, so this does not fit
in place. Read `.linecop.yaml` and split before writing: the table rendering is
the natural seam to lift out, and it leaves `status.rs` owning the report model.
Do not write it fat and refactor after the gate trips.

== Tests

- A rule from an imported fragment reports the fragment as its `source`.
- A rule from the root manifest reports the root manifest, including in a
  single-file project.
- A store-path fragment's rules report the absolute path.
- The table grows the column with two sources and does not with one — assert
  the single-source output is byte-identical to today's.
- `--status=json-schema` and the emitted JSON agree on the new property.

== Note

Rule-level source is the version this task ships. Scope- and probe-level
provenance is `mmz --dump-config`'s job, not the status table's — a rule is what
`--status` is a report about.
