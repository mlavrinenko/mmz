// Rendering helpers shared across content pages.

#import "site.typ": PKG_VERSION, u

// Read one of www/generate.sh's verbatim captures. Every transcript, schema and
// fixture file on this site comes through here, so no page ever hand-types
// output the binary produces.
#let capture(name) = read("../generated/" + name)

// A dark terminal panel holding a plain transcript. Leading and trailing blank
// lines are trimmed so a capture's own padding never becomes layout.
#let terminal(body) = html.elem(
  "div",
  attrs: (class: "terminal"),
  raw(body.trim("\n", repeat: true), block: true),
)

// A terminal panel straight from a capture name — the common case, and one
// fewer place for a page to spell a path.
#let transcript(name) = terminal(capture(name))

// A highlighted code block from a capture, for the artifacts that are files
// rather than output (the scaffolded config, a cache record, the fixture's
// manifest). `lang` drives Typst's syntax highlighting.
#let listing(name, lang: none) = raw(
  capture(name).trim("\n", repeat: true),
  lang: lang,
  block: true,
)

// Callout box. kind ∈ note | tip | warn.
#let callout(kind, body) = html.elem(
  "div",
  attrs: (class: "callout callout-" + kind),
  body,
)

// The `$schema` URL `mmz --init` writes, version-pinned exactly as the binary
// pins it. Quoted on the quickstart and manifest pages; derived so a release
// bump propagates without a human reconciling three copies.
#let schema-url = (
  "https://raw.githubusercontent.com/mlavrinenko/mmz/v"
    + PKG_VERSION
    + "/schema/mmz.schema.json"
)

// One face of the home page's hero: a captioned panel. The three faces are one
// rule's whole life — the manifest that declares it, the run that records it,
// the re-run that skips it — so each is a real capture, never an illustration.
#let face(kind, caption, body) = html.elem(
  "figure",
  attrs: (class: "face face-" + kind),
  {
    html.elem("figcaption", [#caption])
    html.elem("div", attrs: (class: "face-body"), body)
  },
)

#let hero() = html.elem("section", attrs: (class: "hero"), {
  html.elem("div", attrs: (class: "hero-lede"), {
    html.elem("h1")[Skip the work that is already done.]
    html.elem("p", attrs: (class: "hero-sub"))[
      #raw("mmz") is a memoized command runner. Prefix any command with it and
      it runs — then skips, for as long as the inputs you declared are
      byte-for-byte unchanged since that command last succeeded.
    ]
    html.elem("div", attrs: (class: "hero-cta"), {
      html.elem(
        "a",
        attrs: (class: "btn", href: u("/quickstart/")),
        [Get started],
      )
      html.elem(
        "a",
        attrs: (class: "btn ghost", href: u("/concepts/")),
        [How it works],
      )
    })
  })
  html.elem("div", attrs: (class: "faces"), {
    face("config", "declare · .mmz/config.yaml", listing(
      "demo-config.yaml",
      lang: "yaml",
    ))
    face("run", "run · once, then again", terminal(
      capture("run-cold.txt") + "\n" + capture("run-warm.txt"),
    ))
    face("status", "inspect · mmz --status", transcript("status.txt"))
  })
  html.elem("p", attrs: (class: "faces-note"))[
    One rule, three moments: what you declared, what it cost, what it is worth now.
  ]
})
