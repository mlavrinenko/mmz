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
#import "lib.typ": capture, fact, www-link, www-summary

#metadata((
  output: "README.md",
)) <mmz-md>

#let pkg = fact("crate-map").package

// Every docs-site page except the site root. `www-link` renders a page's own
// title, and the root's title is "mmz" over a summary that restates this
// README's own opening line — an index entry pointing the reader back at where
// they already are. The header's Documentation link is that page.
#let doc-routes = fact("site-pages").pages.keys().filter(route => route != "/")

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
  A #link("LICENSE-MIT")[MIT]-licensed memoized command runner.
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

Prefix any command with `mmz`. When the matched rule's declared inputs are
byte-for-byte unchanged since that command last succeeded, `mmz` skips it and
exits 0. Otherwise it runs the command, streams its output, and records the
result on success.

It is not a build system: no task ordering, no dependency graph, no artifact
replay, no remote cache. It answers one question per invocation — is this rule's
work still done?

```yaml
# .mmz/config.yaml
scopes:
  rust: ["**/*.rs", "Cargo.toml", "Cargo.lock", "rust-toolchain.toml"]

commands:
  - name: cargo test
    inputs: [rust]
```

#raw(
  capture("run-cold.txt").trim("\n", repeat: true)
    + "\n"
    + capture("run-warm.txt").trim("\n", repeat: true),
  lang: "console",
  block: true,
)

= Installation

```bash
cargo install mmz
```

#www-link("/quickstart/") covers the prebuilt binaries, `nix run`, and scaffolding
a manifest.

= Usage

```bash
mmz --init                # write a starter .mmz/config.yaml
mmz cargo test            # skipped when the declared inputs are unchanged
mmz --status              # each rule's freshness and record age
mmz --is-fresh --tag gate # exit 0 if every gate-tagged rule is fresh; runs nothing
mmz --prune               # drop records whose rule no longer exists
```

= The trade

`mmz` cannot see a dependency you did not declare, so the asymmetry is the whole
contract:

- Under-declaring a rule's inputs skips a command that should have run — a false
  green, and dangerous.
- Over-declaring buys an unnecessary re-run — and nothing else.

Broaden the scope when in doubt. `mmz` fails closed everywhere else: a missing or
invalid manifest always errors, and so do an unmatched command and a matched rule
with no inputs, unless `strict` relaxes them.

= Documentation

#list(..doc-routes.map(route => [
  #www-link(route) — #www-summary(route)
]))

= Contributing

See #link("CONTRIBUTING.md")[CONTRIBUTING.md]. mmz memoizes its own checks, so
`just check` is itself the worked example — and closing a task asserts those
checks already passed with `mmz --is-fresh --tag gate`.

= License

MIT. Requires Rust #raw(pkg.rust_version) or newer.
