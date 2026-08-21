#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "mmz: a released changelog section can vanish unnoticed",
  priority: framework("ice", confidence: 0.9, ease: 6.0, impact: 6.0),
  tags: ("docs", "gating"),
  links: (
    related("mmz-cut-the-0-8-0-release.typ")[the release that found it],
  ),
  status: done(
    2026,
    8,
    21,
  )[\`just check-changelog-history\` is the eleventh gate: for every \`v\*\` tag reachable from HEAD it diffs the \`== \[x.y.z\]\` section against \`git show \<tag>:CHANGELOG.md\`, and \`tests/changelog\_history\_gate.rs\` pins ten behaviours including the swallowed-heading shape that started this. Both open questions settled. The escape hatch is \`just changelog-waive \<version> \<reason>\`, recording the REWRITTEN section's own hash in CHANGELOG.waivers — outdatty's watermark shape, so a waived section that moves again fails again and a waiver with nothing to cover fails as stale. It sits in \`just check\` rather than beside \`just cover\`: it costs one \`git show\` per tag, and a merge-blocking check a contributor cannot run locally is a different check. The tags it needs are bought with \`fetch-depth: 0\` on the two workflows that run the gate, and a shallow clone is refused rather than passed over.],
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
