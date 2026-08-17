#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "mmz: review the monochrome docs theme in a browser",
  priority: framework("ice", confidence: 0.9, ease: 8.0, impact: 5.0),
  tags: ("docs",),
  links: (
    related("mmz-monochrome-square-docs-theme.typ")[the restyle this reviews]
      + related("mmz-review-the-built-docs-site-in-a-browser.typ")[the review of
        the theme this replaced]
  ),
  status: proposed(2026, 8, 17),
)

== Summary

Run `just docs serve`, open the site, and look at it. The previous review is
closed against a theme that no longer exists, so its verdict does not carry over
— this is the same claim about the same kind of evidence, made again about what
replaced it.

== What to check

- #strong[Both themes on every page.] The palette is two tokens and dark mode is
  their exact inversion, so a component that pins a literal colour instead of a
  token will look right in one theme and wrong in the other. That is the failure
  mode to hunt for.
- #strong[Listings without syntax highlighting.] Typst's baked-in token colours
  are dropped, so every code block is monochrome. Check that the manifest and
  CLI reference pages still parse by eye without them — this is the trade most
  likely to want revisiting.
- #strong[The comparison heatmap.] Fill weight plus a glyph, no hue. Check the
  three levels are still distinguishable from each other at a glance, and that a
  filled cell fills the whole cell in a row where another column wraps.
- #strong[The inverted panels.] A transcript is set as the inverse of the page.
  Check that a page carrying several of them alongside code blocks does not read
  as a stack of unrelated slabs.
- #strong[Narrow viewport.] Below 52rem the sidebar becomes a wrapped row and
  the on-this-page index collapses. Check the matrix scrolls sideways and its
  sticky first column still works.
- #strong[The brand mark.] It is a CSS-masked span, and it has failed silently
  twice. Check the glyph is actually visible in both themes.
- #strong[Search.] Pagefind's index only exists after a production build, so
  check it with `just docs build` and a local server, not with `serve`.

== Done when

The session was actually run and its outcome is recorded here — pass or fail. A
problem found gets its own new task linked back to this one, rather than
reopening this one or the restyle: reopening would lose the fact that a real
review happened.
