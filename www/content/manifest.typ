#import "../utils/page.typ": page
#import "../utils/ui.typ": callout, listing, schema-url
#import "../utils/config-ref.typ": SCHEMA, entry-prose, groups, summary-table
#import "../utils/site.typ": PKG_VERSION, u

#let meta = (
  route: "/manifest/",
  label: "Manifest reference",
  title: "Manifest reference",
  summary: "Every key .mmz/config.yaml can declare, generated from the JSON Schema the binary ships.",
)
#metadata(meta) <page-meta>

#show: page.with(..meta)

// Everything below the hand-written intro is generated from
// `mmz --schema` (www/generate.sh's capture) plus the hand-written notes in
// www/utils/config-notes.typ. `just check-doc-facts` fails when the two key sets
// disagree in either direction, so this page cannot go stale against the binary
// and cannot carry prose for a key that no longer exists.

`mmz` reads the nearest `.mmz/config.yaml`, searching upward from the working
directory, plus whatever it names in a top-level `imports:` list — see
#link(u("/composition/"))[composing a manifest from imports]. The directory
holding `.mmz` is the project root, and every relative path in the manifest
resolves against it, except an `imports:` entry itself, which resolves against
the *importing file's* directory.

#listing("demo-config.yaml", lang: "yaml")

The manifest is validated at load: command names must be non-empty and unique,
every `inputs:` entry must name a defined scope or probe (and no name may be
both), outputs must be literal paths, and `strict` names must be known. A
manifest that does not validate is an error, never a warning — see
#link(u("/concepts/"))[fail closed].

Run `mmz --schema` for the schema itself; it is the source this page is generated
from, and it is what the `$schema` line `mmz --init` writes points at:

#raw(schema-url)

#callout("note")[
  The `$schema` URL is pinned to the `v#PKG_VERSION` tag rather than `main`, so a
  project keeps validating against the schema its mmz was built for. Two projects
  on different mmz versions each validate correctly.
]

#for group in groups {
  heading(level: 1, group.title)
  group.lede
  summary-table(group)
  for entry in group.entries {
    entry-prose(entry)
  }
}
