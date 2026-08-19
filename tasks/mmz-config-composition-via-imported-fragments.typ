#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "mmz config composition via imported fragments",
  priority: framework("ice", confidence: 0.7, ease: 4.0, impact: 7.0),
  tags: ("config",),
  status: proposed(2026, 8, 20),
)

== Summary

mmz reads exactly one manifest — the nearest `.mmz/config.yaml`, found by
searching upward — and offers no way to assemble it from parts. Every scope,
command and tag must be hand-written into that single file. A tool that wants
to contribute rules has to own and rewrite the whole manifest, clobbering
whatever the project wrote by hand.

Add composition: let a manifest pull scopes and commands out of other YAML
files, so generated rules and hand-written rules can coexist in one project.

== Why

The rules in a `.mmz/config.yaml` restate a fact that already lives in the
`Justfile`: which recipes are gates, and what each one reads. cratemplate keeps
the two in sync by hand and had to add a validation gate for the failure —
an arm of `just check` with no matching rule — because they desync on their
own. The same fact is restated a third time in the CI job list.

Collapsing that to one declaration means a generator emits the `linecop` rule
and its scope whenever the linecop gate is enabled. That is only possible if
mmz can merge a generated fragment with the project's own manifest. Without
composition the generator must own the entire file, which forbids any
hand-written rule beside it and makes adoption all-or-nothing.

This is worth having on its own terms: a workspace with several crates that
share a gate set hits the same wall today.

== Scope

Design questions to settle first, before any implementation:

- Spelling: an `imports:` key inside the manifest, a directory convention
  (`.mmz/conf.d/`), or both.
- Merge semantics per section. Scopes and commands are keyed maps, so the
  natural rule is "merge by key"; the open question is what a duplicate key
  means — last-wins, first-wins, or a hard error naming both sources.
- Whether an imported fragment may itself import, and if so, cycle detection.
- Whether a fragment can be a store path, since a generated one usually is.
- What `mmz --status` and validation errors report as the origin of a rule:
  a merged view hides which file a rule came from, which is exactly what
  someone debugging a surprising skip needs to know.
- Precedence against the schema: a fragment that is invalid alone but valid
  merged (or the reverse) needs a defined answer.

Non-goal for this task: a `--config` flag to relocate the manifest. Related,
but separable, and composition is the part that unblocks anything.

== Notes

Filed while designing wormfork, a language-agnostic project scaffolding tool
that would generate gate wiring (just recipes, mmz rules, CI jobs, devShell
packages) from one module declaration. mmz composition is one of its
prerequisites, but the feature stands alone if wormfork never ships.
