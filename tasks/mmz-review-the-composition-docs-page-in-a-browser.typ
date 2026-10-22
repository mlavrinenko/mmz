#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "mmz: review the composition docs page in a browser",
  priority: framework("ice", confidence: 0.9, ease: 8.0, impact: 4.0),
  tags: ("docs",),
  links: (
    depends-on("mmz-document-manifest-composition.typ")[the page this reviews]
      + related("mmz-review-the-built-docs-site-in-a-browser.typ")[the same
        pass, earlier]
  ),
  status: done(
    2026,
    10,
    22,
  )[reviewed; drift and narrow-screen clipping filed as follow-ups],
)

== Summary

Run `just docs serve`, open `/composition/`, and look at it. The gates prove the
page compiles, has a route and sits in the nav; none of them proves it reads
well or that its examples are the ones a newcomer needs.

== What to look at

- Does the duplicate-key argument land, or does it read as an arbitrary
  restriction? That paragraph is why the page exists.
- Is the path asymmetry — import paths file-relative, globs root-relative —
  obvious from the example, or only from the sentence next to it?
- The nav placement, seen rather than reasoned about: does `/composition/` sit
  where someone hunting for it would look?
- The `--dump-config` sample output, against what the binary actually prints.
- Long YAML samples and the store path: wrapping, overflow, both themes,
  narrow viewport.
- Every link the page adds, followed.

== Done when

The session was run and its outcome is written into this task — including
"nothing to change", if that is the outcome. Anything found gets its own task
linked back here; do not reopen the docs task for it.

== Outcome

Run on 2026-10-22 against `just docs serve`, desktop and 375px, both themes.

- The duplicate-key argument lands: the stale-generator scenario and the "false green" line make it a design choice, not a restriction.
- The path asymmetry reads from the example itself: the failing `.mmz/conf.d/` import, the `conf.d/` fix, and the root-relative glob in the fragment.
- Nav placement is right: last under Guide, after gating, and the manifest reference links in where imports first appear.
- Every link and on-page anchor resolves.
- The `--dump-config` sample, the wrong-import error and the prose's policy-key count have drifted from the binary, because the page types them by hand: filed as `mmz-the-composition-pages-transcripts-are-hand-typed`, with two wording slips.
- At 375px the wrong-import error clips behind a scroll with no cue: filed as `mmz-a-long-code-line-on-a-narrow-screen-scrolls-out-of-sight`, a site-wide CSS fix.
