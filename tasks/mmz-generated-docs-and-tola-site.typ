#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "mmz: generate the root Markdown from Typst and rebuild www as a tola docs site",
  priority: framework("ice", confidence: 0.9, ease: 3.0, impact: 8.0),
  tags: ("docs", "tooling"),
  links: (
    related("mmz-review-the-built-docs-site-in-a-browser.typ")[the manual review
      the gates cannot do]
      + related("mmz-pin-the-clock-for-reproducible-output.typ")[surfaced by the
        record capture]
  ),
  status: done(2026, 8, 17),
)

== Summary

Adopt MindTape's docs pipeline here, both halves of it:

- `www/` becomes a real documentation site — Typst content compiled by tola,
  with the transcripts, the CLI surface and the config schema captured from the
  built `mmz` binary rather than transcribed. Today it is one hand-written
  `index.html` that restates the README from memory.
- `README.md`, `AGENTS.md` and `CONTRIBUTING.md` stop being hand-written. Each
  is rendered from a `docs/src/*.typ` source by typlite, and a freshness gate
  fails when a committed `.md` has drifted from its source.

== Why

Every fact this repo documents is currently written down two or three times: the
flag list lives in `src/main.rs`, in `README.md`, and again in
`www/index.html`; the manifest keys live in `src/manifest.rs`, in
`schema/mmz.schema.json`, and again in both docs. `outdatty.yaml` couples them
so a source edit at least *flags* the copies, but flagging is not deriving — a
reviewer still reconciles by hand, and the 500-line README is where that goes
wrong quietly.

The fix MindTape already proved: a doc STATES a fact by reading it. `mmz --help`,
`mmz --schema` and `mmz --status` are all machine-readable, so the pages that
describe them can be generated from the binary, and the copies stop existing.

== Scope

- `docs/src/`: `lib.typ` (typlite helpers, docs-site link resolution),
  `readme.typ`, `agents.typ`, `contributing.typ`, plus `contributing/*.typ` deep
  dives for the material the 200-line Markdown cap should not carry inline.
- `docs/generate-md.sh`: glob the sources, read each one's own `<mmz-md>` block
  for its output path, render with typlite.
- `www/`: `tola.toml`, `templates/`, `utils/`, `content/*.typ`, and the three
  generators — `generate.sh` (capture live `mmz` output), `generate-facts.sh`
  (derive facts from `Cargo.toml`, the `Justfile`, `.linecop.yaml`),
  `generate-site-pages.sh` (derive the page manifest from each page's own
  `<page-meta>` block).
- `.just/docs.just`: `docs::gen`, `docs::build`, `docs::serve`, `docs::check`,
  `docs::md`, `docs::md-check`.
- Gates into `just check`: `docs::check` (build + validate links), `docs::md-check`
  (generated Markdown matches its source), `check-doc-coverage` (every CLI flag
  is documented), `check-doc-facts` (every derived fact has prose).
- `flake.nix`: tola, typst, tinymist (for `typlite`), pagefind, jq, yq, typstyle.
- `.linecop.yaml`: cap the `.typ` sources, exclude every generated `.md` output.
- `outdatty.yaml`: the doc groups now point at `docs/src/**`, not the outputs.
- `.github/workflows/pages.yml`: build the site through the dev shell instead of
  uploading `www/` verbatim.

== Non-goals

- No Pagefind-indexed search on day one if it costs a gate; add it once the
  build is green.
- Do not port MindTape's mutation/coverage/signal-sweep deep dives. mmz has no
  such gates, and a doc for a gate that does not exist is worse than no doc.

== Home

mmz's own backlog. The generated-docs practice is MindTape's; this task is the
port, not a re-design.
