#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "mmz: a released changelog section can vanish unnoticed",
  priority: framework("ice", confidence: 0.9, ease: 6.0, impact: 6.0),
  tags: ("docs", "gating"),
  links: (
    related("mmz-cut-the-0-8-0-release.typ")[the release that found it],
  ),
  status: proposed(2026, 8, 21),
)

== Summary

`4c57277` inserted a `== Fixed` section under `== Unreleased` and overwrote the
`== [0.7.0] - 2026-08-17` heading line doing it. The whole released 0.7.0
section — every entry the tag shipped — silently became part of `Unreleased`,
and four later entries were appended into what readers would take for the
0.7.0 list. One prose line was split mid-sentence in the same edit.

Nothing caught it. Ten gates ran green over the corrupted file for four days
and thirteen commits, because no gate reads CHANGELOG.md: `outdatty` couples
`Cargo.toml` to it as a *dependent*, which asks that a human re-confirmed, not
that the content is intact, and `linecop` only counts its lines.

== Why it matters past the one-off repair

The repair is done (see the release below). What is missing is the guard. A
changelog is the one document whose past is immutable — a section for a shipped
tag is a historical record, and an edit to it is either a mistake or a rewrite
of history. That makes it unusually easy to check, and this repo's own rule is
that a fact worth stating is a fact something reads.

== Shape

A gate that, for every `v*` tag reachable from `HEAD`, asserts the `== [x.y.z]`
section in the working `CHANGELOG.md` is byte-identical to the one in
`git show <tag>:CHANGELOG.md`. Cheap: one `git show` per tag, no build.

Two open questions worth settling before writing it:

- A deliberate correction to a shipped section (a typo, a wrong link) would
  fail the gate. An escape hatch is needed, and a recorded-hash waiver in the
  `outdatty` spirit — a human looked — fits this repo better than a flag.
- Whether it belongs in `just check` or beside `just cover` as a CI-only arm.
  It is fast enough for the loop, but it needs the tags fetched, which a
  shallow CI checkout does not have by default.
