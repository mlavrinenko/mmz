// docs/src/readme.typ — source for the repo-root README.md. Rendered to
// Markdown by `docs/generate-md.sh` via typlite; see docs/src/lib.typ for what
// that render path can and cannot do.
//
// The README is a front door, not a manual. The previous hand-written one was
// 500 lines and carried the entire user manual — every manifest key, the
// matching rules, the correctness contract — which is why it needed its own
// linecop override and why every feature landed as another section appended to
// it. All of that now lives on the docs site, where it can be generated from
// the schema and the binary instead of retyped.
//
// So no page is restated below. Every site link goes through `www-link`, which
// resolves the route against www/generated/site-pages.json and fails this render
// naming an unknown one, and the page list reads each page's own summary out of
// that same manifest — a page added, renamed or re-summarised on the site lands
// here with no edit.
//
// The centered header is a deliberate rebuild, not a literal port:
// `<p align="center">` is how a README normally centers its logo and tagline,
// but typlite silently strips every `<p>` (docs/src/lib.typ).
// `<h1 align="center">`/`<h4 align="center">` survive both typlite and GitHub's
// HTML sanitizer, so the header below uses those. This document therefore has
// no separate `doc-title` ATX `#`: the centered `<h1>` IS the title.
#import "lib.typ": capture, fact, just, www-label, www-link, www-summary

#metadata((
  output: "README.md",
)) <mmz-md>

// Every docs-site page except the site root. `www-link` renders a page's own
// title, and the page's `group`/`order` place it under the site's own group
// headings; the root's title is "mmz" over a summary that restates this
// README's own opening line — an index entry pointing the reader back at where
// they already are. The header's Documentation link is that page.
#let site = fact("site-pages")
#let doc-groups = site.groups.map(title => (
  title: title,
  routes: site
    .pages
    .keys()
    .filter(route => (
      route != "/" and site.pages.at(route).at("group") == title
    )),
))

#html.elem("h1", attrs: (align: "center"))[
  #html.elem("img", attrs: (
    src: "https://raw.githubusercontent.com/mlavrinenko/mmz/main/www/assets/images/logo.svg",
    alt: "mmz",
    width: "96",
  ))
  #html.elem("br")
  mmz
]

#html.elem("h4", attrs: (align: "center"))[
  Memoize what's done.
]

#html.elem("h4", attrs: (align: "center"))[
  #www-link("/")[Documentation] · #www-link("/quickstart/") · #www-link("/comparison/")
]

\-\-\-

#link("https://github.com/mlavrinenko/mmz/actions/workflows/ci.yml")[#html.elem(
  "img",
  attrs: (
    src: "https://github.com/mlavrinenko/mmz/actions/workflows/ci.yml/badge.svg",
    alt: "CI",
  ),
)]
#link("https://crates.io/crates/mmz")[#html.elem("img", attrs: (
  src: "https://img.shields.io/crates/v/mmz.svg",
  alt: "crates.io",
))]
#link("LICENSE-MIT")[#html.elem("img", attrs: (
  src: "https://img.shields.io/crates/l/mmz.svg",
  alt: "License: MIT",
))]

Configure inputs for a command and run it with `mmz ...` as a prefix.
`mmz` remembers the last successful run and skips further executions while those
inputs remain unchanged.

For example, the
#link("https://github.com/mlavrinenko/mmz/tree/main/examples/demo")[demo project's] `.mmz/config.yaml`
declares two commands and their inputs:

// The fixture's `$schema` line is relative to examples/demo; drop it from the
// copy shown here, where it would point outside the reader's project.
#raw(
  capture("demo-config.yaml")
    .split("\n")
    .filter(line => not line.starts-with("# yaml-language-server:"))
    .join("\n")
    .trim("\n", repeat: true),
  lang: "yaml",
  block: true,
)

Running `mmz ./bin/validate.sh` does the work:

#raw(
  capture("run-cold.txt").trim("\n", repeat: true),
  lang: "console",
  block: true,
)

Running it again skips the command:

#raw(
  capture("run-warm.txt").trim("\n", repeat: true),
  lang: "console",
  block: true,
)

Then, you can put `mmz --is-fresh --tag gate` in your git pre-commit hook to
refuse a commit when a gated check is out of date:

```bash
#!/usr/bin/env sh
# .git/hooks/pre-commit
mmz --is-fresh --tag gate || {
  echo "run the gated checks first" >&2
  exit 1
}
```

No checks run during `git commit`, but you're still covered.

See how it #www-link("/comparison/")[compares] with other tools.

= Installation

```bash
cargo install mmz
```

#www-link("/quickstart/") covers the prebuilt binaries, `nix run`, and
scaffolding a manifest.

= Usage

```bash
mmz --init                # write a starter .mmz/config.yaml
mmz cargo test            # skipped when its inputs are unchanged
mmz --status              # each command's state and last-run age
mmz --is-fresh --tag gate # exit 0 if every gate-tagged command is up to date; runs nothing
mmz --prune               # drop cache records for commands that no longer exist
mmz --import ../wt        # copy cache records from another checkout
```

= Documentation

#for group in doc-groups [
  #if group.routes.len() > 0 [
    *#group.title*

    #list(..group.routes.map(route => [
      #www-link(route)[#www-label(route)] — #www-summary(route)
    ]))
  ]
]

= Contributing

See #link("CONTRIBUTING.md")[CONTRIBUTING.md].

= License

MIT.
