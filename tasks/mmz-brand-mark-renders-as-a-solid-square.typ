#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "mmz: the header brand mark renders as a solid square",
  priority: framework("ice", confidence: 0.9, ease: 7.0, impact: 4.0),
  tags: ("docs",),
  links: (
    related("mmz-review-the-built-docs-site-in-a-browser.typ")[the review that
      found it]
      + related("mmz-monochrome-square-docs-theme.typ")[the restyle it landed
        with]
  ),
  status: done(2026, 8, 17),
)

== Summary

Every page's header rendered the logo as a featureless accent-coloured square.
`<span class="brand-mark">` was painted with the accent and masked by
`logo.svg`, but a CSS mask samples an image's ALPHA — and `logo.svg` is an
opaque tile, every pixel alpha 1. The mask revealed the whole square and the
glyph never appeared.

Fixed by splitting the asset: `logo.svg` stays the picture (favicon, README),
and a new `www/assets/images/mark.svg` carries the glyph alone as a mask source.
`.brand-mark` now paints in two layers — an ink tile, and over it a paper-filled
box masked down to the glyph — so both halves take theme tokens and invert with
the rest of the page.

== Two dead ends worth keeping

Both were tried, both fail silently, and neither is obvious from reading the
CSS:

- #strong[A real `<img>` instead of the masked span.] Typst treats an image as
  block content and paragraph-wraps the sibling after it, emitting
  `<a><img><p><span>mmz</span></p></a>`. An open `<p>` closes on the next `<p>`,
  so the parser splits the anchor into TWO anchors and the mark and the wordmark
  link separately. Confirmed in a browser, not reasoned about.
- #strong[Cutting the glyph out of the tile inside the SVG.] With
  `fill-rule="evenodd"` the two triangles overlap by 2 units, and the overlap
  crosses three subpaths, turns odd and fills back in as a sliver. With an SVG
  `mask` element the file renders fully transparent the moment a browser
  rasterizes it as a mask source, and the mark vanishes entirely.

== The one that cost the most time

The first `mark.svg` was correct and still rendered nothing, because its header
comment named the CSS custom properties by their real spelling. A double hyphen
is illegal inside an XML comment, so the file was unparseable and the mask
silently empty — no console error, no network failure, just an element with a
resolving mask URL and zero alpha. `mark.svg` now says so in its own header.

== Regression cover

None automated, and that is the honest state: `tola validate` proves the asset
resolves, which it already did while the mark was invisible. What this needs is
a rendered-pixel check, which the docs pipeline has no harness for. Recorded in
`mmz-review-the-monochrome-docs-theme-in-a-browser.typ` as something a human
looks at instead.
