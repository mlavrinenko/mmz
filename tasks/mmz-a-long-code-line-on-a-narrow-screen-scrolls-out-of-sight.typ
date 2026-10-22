#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "mmz: a long code line on a narrow screen scrolls out of sight",
  tags: ("docs",),
  priority: framework("ice", confidence: 0.8, ease: 7.0, impact: 4.0),
  status: wip(2026, 10, 22),
  links: related(
    "mmz-review-the-composition-docs-page-in-a-browser.typ",
  )[found by this review],
)

== Summary

Every `pre` on the site scrolls sideways (`www/assets/styles/main.css:196`, `overflow-x: auto`) with no visible cue. At a 375px viewport the composition page's wrong-import error is cut off right after "names" and the first two characters of the path, so the path that carries the page's whole argument is off-screen and nothing says there is more. The dump and status samples scroll the same way, just less.

Done when a long line in a short error or output block is readable at 375px without a hidden scroll, in both themes, checked in a browser on the composition page. Wrapping output blocks (`white-space: pre-wrap`) or a visible scroll cue both qualify; YAML samples may keep scrolling if wrapping would mislead about indentation.

