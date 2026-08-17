#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "mmz: review the built docs site in a browser",
  priority: framework("ice", confidence: 0.9, ease: 8.0, impact: 5.0),
  tags: ("docs",),
  links: (
    related("mmz-generated-docs-and-tola-site.typ")[the build this reviews]
      + related("mmz-monochrome-square-docs-theme.typ")[filed from this review]
      + related("mmz-brand-mark-renders-as-a-solid-square.typ")[filed from this
        review]
      + related("mmz-review-the-monochrome-docs-theme-in-a-browser.typ")[the
        same review, run again against what replaced the theme]
  ),
  status: done(2026, 8, 17),
)

== Summary

Run `just docs serve`, open the site, and look at it. The build gates prove the
pages compile, that every internal link resolves, and that no derived fact is
missing its prose — none of them prove the site reads well or looks right.

== Why this is its own task

`just docs::check` is a correctness gate, not a design review. It cannot see a
terminal panel overflowing its column, a heatmap cell whose text is unreadable
against its fill, a dark-mode token that vanishes, or a table that a phone
squeezes into unreadability. Those need eyes.

Splitting it out also keeps the implementation task's "done" honest: the code is
finished and gated, and this is a separate claim about a separate kind of
evidence.

== What to check

- #strong[Both themes.] Toggle light/dark on every page. The code listings use a
  fixed light "paper" surface in both (Typst bakes one highlight theme into the
  span colours), so check that inline code chips are still legible in dark mode.
- #strong[Narrow viewport.] Below 52rem the sidebar becomes a wrapped row and
  the on-this-page index collapses to a `details` toggle. Check the comparison
  matrix scrolls sideways rather than squeezing, and that its sticky first
  column still works.
- #strong[The hero.] Three captured faces stacked on the home page. Check none
  of them overflow, and that the transcripts are not clipped.
- #strong[Search.] Pagefind's index only exists after a production build, so
  check it with `just docs build` and a local server, not with `serve`.
- #strong[The generated reference pages.] `/manifest/` and `/cli/` render a
  schema description and a hand-written note per entry. Check the two voices
  read as distinct rather than as a repetition.

== Done when

The session was actually run and its outcome is recorded here — pass or fail. A
problem found gets its own new task linked back to this one and to
`mmz-generated-docs-and-tola-site.typ`, rather than reopening either: reopening
would lose the fact that a real review happened.

== Outcome

Run 2026-08-17. #strong[Failed] — six findings, all fixed. None of them were
things a gate could have caught, which is what this task existed to establish.

The two substantial ones carry their own tasks:

- The whole visual design was MindTape's, borrowed rather than chosen —
  `mmz-monochrome-square-docs-theme.typ`.
- The header brand mark rendered as a featureless coloured square on every page
  — `mmz-brand-mark-renders-as-a-solid-square.typ`.

Four were single-line copy defects in `docs/src/`, fixed in the same commit
rather than filed:

+ The README's License section carried an MSRV sentence that had nothing to do
  with licensing. Dropped.
+ "It is not a build system: no task ordering, no dependency graph, no artifact
  replay, no remote cache" — a list of things mmz is not, in place of a claim
  about what it is, and repeated in `agents.typ`. Both now state the one
  question and point at the comparison page for the boundary.
+ `agents.typ` hand-typed the repository URL. It reads `package.repository` out
  of the generated crate map instead.
+ `agents.typ` spelled recipe invocations two ways in one section. Every one now
  goes through the generated `just` module, so a rename fails the render.

The theme this reviewed no longer exists, so the verdict does not transfer:
`mmz-review-the-monochrome-docs-theme-in-a-browser.typ` makes the same claim
again about what replaced it.
