#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "mmz: the error enum cannot take a new variant under the size cap",
  priority: framework("ice", confidence: 0.8, ease: 4.0, impact: 4.0),
  tags: ("tooling",),
  status: proposed(2026, 8, 21),
  links: related(
    "mmz-an-i-o-error-while-hashing-an-input-names-no-path.typ",
  )[the fix whose size cost forced the cap raise],
)

== Summary

`src/error.rs` sat at 486 of its 500-line cap. Adding the two variants
0.8.1 needed — `InputVanished` and `InputUnreadable`, with the field docs and
the two-to-four lines of prose every other variant carries — put it at 521, and
there was no prose left to cut that would have brought it back under: two
variants cost at least 20 lines even written terse, and the headroom was 14.

The cap was raised to 550 in `.linecop.yaml`, argued in place. That is a
holding position, not an answer: the next variant spends the new headroom and
the same conversation happens again with less room to have it in.

== Why the usual remedies do not apply

`just eject` moves inline tests out of an oversized file; `error.rs`'s tests
were ejected to `src/error_tests.rs` long ago, so there is nothing left for it
to take. And a `thiserror` enum is one item — no text boundary splits it, so
the `manifest.rs` → `manifest_strict.rs` move that answered the same pressure
elsewhere has no equivalent here.

== The technique that does work is already in the file

`Error::ProbeAst` carries `Box<crate::ast::AstFailure>`: a family of related
failures pushed into a sub-enum that lives with the code that raises them, held
by one variant here. Nine probe variants (`ProbeFailed`, `ProbeSpawn`,
`ProbeEmpty`, `ProbeSource`, `ProbeFileUnreadable`, `ProbeJson*`) are the
obvious next application — they share an exit code, they share a consequence
("mmz consumed no output and wrote no cache record"), and they are about 90
lines.

== Why it is not a drive-by

- It is a public API change. `Error` is exported, and a library caller
  matching `Error::ProbeFailed { .. }` would match `Error::Probe(..)` instead.
- `exit_for` in `src/main.rs` collapses to one arm for the group, which is
  either a simplification or a loss of the per-variant record — worth deciding
  deliberately rather than in passing.
- `src/error_tests.rs` asserts message text per variant and would move with
  them.

So it wants its own change, its own changelog entry, and a version that says
what moved — not a smuggled refactor inside whichever fix next trips the cap.

== Done when

`.linecop.yaml` no longer needs an override for `src/error.rs`, and the file is
back under the shared Rust cap with room for the next failure mode.
