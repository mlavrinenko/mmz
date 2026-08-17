#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "mmz: restyle the docs site to the monochrome square theme",
  priority: framework("ice", confidence: 0.9, ease: 5.0, impact: 6.0),
  tags: ("docs",),
  links: (
    related("mmz-review-the-built-docs-site-in-a-browser.typ")[the review that
      asked for this]
      + related("mmz-brand-mark-renders-as-a-solid-square.typ")[a rendering bug
        the same pass fixed]
      + related("mmz-review-the-monochrome-docs-theme-in-a-browser.typ")[the
        review of what this produced]
  ),
  status: done(2026, 8, 17),
)

== Summary

The docs site shipped wearing MindTape's design — warm serif body, a single
orange accent, rounded corners, soft shadows, two webfonts off Google Fonts. It
was borrowed, not chosen, and it did not look like this project.

Replaced with the theme `www/index.html` carried before the site existed:
quadratisch, praktisch, gut. Two colours, one monospace family, 2px rules, no
radii, no shadows, no accent.

== What changed

- `www/assets/styles/main.css` and `components.css` rewritten against an
  `ink`/`paper` token pair. Dark mode is the exact inversion of those two, so no
  component carries a dark-mode branch of its own.
- Section headings are the signature: a small letterspaced uppercase label on a
  full-width rule, rather than a large bold word.
- The webfont links are gone from `www/templates/layout.typ`. A mono-only site
  can use the face the machine already has, so the page renders with no network
  font and no preconnect.

== What the palette forced

With one colour there is nothing left to encode meaning WITH, so three things
that used the accent were re-encoded:

- #strong[Selected nav item] fills instead of tinting.
- #strong[Callout kind] is written out (a `::before` label) and a warning also
  thickens its frame — two signals, neither of them colour.
- #strong[The comparison heatmap] keeps its rubric but carries it as fill weight
  plus a leading glyph. It also moved off the span onto the `<td>` via `:has()`:
  a span is only as tall as its own text, so in a row where another column
  wrapped to two lines the old rule painted a solid block over half a cell.

Typst bakes one light syntax-highlight theme into per-token inline styles on
every `raw(..., lang: ...)` block, and a two-colour page has nowhere to put
those — half of them are unreadable on one of the two papers. They are dropped
in CSS (`pre code span { color: inherit !important }`) rather than by stripping
`lang:` from each call site, so a new content page needs no rule of its own.

== Verified

`just check` green, and every page opened in a browser in both themes plus a
420px viewport: heatmap fills, sticky matrix column, search popup, the wrapped
narrow-viewport sidebar.
