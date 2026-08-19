#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "mmz: document manifest composition",
  priority: framework("ice", confidence: 0.9, ease: 5.0, impact: 5.0),
  tags: ("docs", "config"),
  links: (
    parent("mmz-config-composition-via-imported-fragments.typ")
      + depends-on("mmz-load-and-merge-imported-manifest-fragments.typ")
      + depends-on("mmz-ship-a-fragment-json-schema.typ")
      + depends-on("mmz-dump-the-merged-manifest-with-provenance.typ")
      + related("mmz-review-the-composition-docs-page-in-a-browser.typ")[the
        human pass over what this writes]
  ),
  status: proposed(2026, 8, 20),
)

== Summary

The generated reference pages pick up `imports:` and the new actions on their
own — the manifest page renders the schema, the CLI page renders `--help`. What
none of that carries is the argument: why a duplicate key is an error, why
import paths resolve differently from globs, and what a generated fragment is
for. That wants a hand-written page.

== The page

`www/content/composition.typ`, with the route `/composition/` and a
`<page-meta>` block like every other page. `just check-doc-facts` asserts every
`www/content/*.typ` has a route matching its filename *and* a `NAV` entry in
`www/utils/site.typ`, in both directions — a page with no nav entry fails the
gate, and so does a nav entry that 404s. Add both in one change.

Default placement: the `Guide` group, after `/gating/`. `Reference`, beside
`/manifest/`, is defensible too; pick one and do not leave it out of `NAV`
while deciding.

== What the page has to say

- The shape: an `imports:` list, a directory entry, a store path.
- A duplicate key is an error, and *why* — last-wins would let a stale
  hand-written rule silently replace a regenerated one, which is the false
  green the whole tool is built against. This is the paragraph the page exists
  for.
- The path asymmetry, stated loudly: import paths resolve against the importing
  file's directory, globs and outputs against the project root. Show a nested
  fragment so the difference is concrete rather than asserted.
- The sharp edge of that asymmetry, as a worked example rather than a caveat:
  the `conf.d` beside the config is imported as `conf.d/`, not `.mmz/conf.d/`,
  because the importing file already sits in `.mmz`. The path everyone thinks
  in is the wrong one, so print the right one next to a scope glob that is
  root-relative and looks like it disagrees.
- The fragment surface: rules, not policy, and the fragment schema to validate
  against.
- Ordering: host rules first, imports depth-first, and that this only matters
  for overlapping token prefixes.
- Debugging: `mmz --dump-config` and `--status`'s `source`, with real output.
- The generator story, one paragraph — a tool contributes a fragment, the
  project keeps its manifest. Do not oversell it: no generator ships yet.

== Also touched

- `www/content/manifest.typ`'s intro still says mmz reads *the* nearest
  `.mmz/config.yaml`, full stop. It now reads that file and whatever it
  imports; fix the sentence and link across.
- `docs/src/readme.typ` and `docs/src/agents.typ` — a line each, if
  composition changes what an agent should do. Anything in `docs/src` is a
  source for the committed Markdown, so `just docs::md-check` will demand the
  regenerated output in the same commit.

== Definition of done

`just check` green, including `check-doc-facts` and both docs arms. The browser
pass is its own task and does not gate this one.
