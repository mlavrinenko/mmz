#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "mmz: the composition page's transcripts are hand-typed",
  tags: ("docs",),
  priority: framework("ice", confidence: 0.9, ease: 6.0, impact: 5.0),
  status: done(2026, 10, 22),
  links: related(
    "mmz-review-the-composition-docs-page-in-a-browser.typ",
  )[found by this review],
)

== Summary

`www/content/composition.typ` types its error, `--dump-config` and `--status` samples by hand, and the browser review of 2026-10-22 found them drifted from the binary. The wrong-import error prints `.mmz/.mmz/conf.d` with no trailing slash; the page adds one. The dump has a fifth policy line, `probe_shell: ["sh", "-c"]  (default)`, that the sample lacks, and the prose says a fragment may not set "four" policy keys when `probe_shell` makes it five. Every other transcript on the site comes from `www/generate.sh`, so this page is the one place a fact is typed instead of read.

Done when the three samples are captures from a real run against a composed fixture, the policy-key list and count in the prose are derived (from the gap between `--schema` and `--schema=fragment`) or at least match the binary, and two wording slips are fixed: "three directories deep" is two (`.mmz/conf.d/lint.yaml`), and the forward-compatibility rationale is a design note, so `callout("note")` rather than `warn`.

