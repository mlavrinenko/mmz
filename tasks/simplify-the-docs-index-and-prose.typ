#import "@local/mindtape:0.2.0": *
#import "/.mindtape/lib.typ": *

#show: task.with(
  title: "Simplify the docs index and prose",
  kind: "docs",
  priority: ice(
    impact: 3,
    confidence: 0.9,
    ease: 7,
  ),
  difficulty: 2,
  tags: ("docs",),
  estimate: (src-files: 15),
  x-goals: (
    product: (
      no-false-green: neutral,
      exact-inputs: neutral,
      cheap-gates: neutral,
      agent-drivable: neutral,
      composable-manifest: neutral,
      ready-binaries: neutral,
      docs-teach-judgement: neutral,
    ),
    process: (
      mmz-gates-mmz: neutral,
      docs-read-facts: neutral,
      rules-not-memory: neutral,
      cheap-delegation: neutral,
    ),
  ),
  status: done(2026, 11, 8),
)

== Problem

+ Open the README's Documentation section: it lists every page's full site
  abstract, so the section is long and hard to skim.
+ The word "honest" is used as filler across docs, comments, tests and tasks
  ("honest exit codes", "one honest re-run").


== TO-BE

- 🟢 The README index groups pages under short headings and gives each page a
  single friendly line.
- 🟢 Grouping and order live in each page's `<page-meta>`; the generator
  rejects a missing group, a duplicate order or an empty group.
- 🟢 "honest" appears nowhere in the project.


== Source of truth

- `www/content/*.typ`: each page's `summary`, `group` and `order`.
- `www/utils/site.typ`: the `GROUPS` titles in reading order.
- `www/generate-site-pages.sh`: validates and sorts the manifest.


== Changes

- `www/content/*.typ`: add `group` and `order`, and shorten every summary.
- `www/utils/site.typ`, `www/templates/layout.typ`, `www/utils/page.typ`:
  derive the sidebar from page metadata.
- `www/generate-site-pages.sh`: validate group/order and emit the grouped
  manifest.
- `docs/src/readme.typ`, `docs/src/lib.typ`: grouped README index using
  sidebar labels.
- `docs/src/contributing/generated-docs.typ`: document group/order.
- `.just/scripts/check-doc-facts.sh`, `Justfile`: drop the NAV check.
- `.just/docs.just`, `www/generate.sh`, `www/measure-sizes.sh`,
  `.mmz/conf.d/{20-docs,30-repo}.yaml`, `flake.nix`, `src/*`, `tests/*`,
  `tasks/*`: replace "honest" with plainer wording.


== Out of scope

- No behavior change in mmz itself; the generator's validation is the only
  new code path.
- No release, tag or CHANGELOG entry; this is prose and docs plumbing.


== Dogfood

The docs build renders the grouped manifest and the README, and `just check`
covers both.

