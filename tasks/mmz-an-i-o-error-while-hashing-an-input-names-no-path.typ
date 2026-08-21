#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "mmz: an i/o error while hashing an input names no path",
  priority: framework("ice", confidence: 0.9, ease: 7.0, impact: 6.0),
  tags: ("cli", "cache"),
  status: done(
    2026,
    8,
    21,
  )[0.8.1: hashing errors carry the path (Error::InputVanished / InputUnreadable, rendered through Provenance::shorten) and exit 8 rather than 70. Regression tests: src/hashing.rs resolves a scope, deletes an input, hashes; tests/cli\_unreadable\_inputs.rs drives the binary. Audit findings filed as the linked task; the two variants tripped src/error.rs's size cap, also linked.],
  links: related(
    "mmz-an-i-o-failure-outside-hashing-still-names-no-path.typ",
  )[the same shape at four sites this fix left alone]
    + related(
      "mmz-the-error-enum-cannot-take-a-new-variant-under-the-size-cap.typ",
    )[the size cap this fix's two variants tripped],
)

== Summary

`Error::Io` is a bare `#[from] std::io::Error` and `hashing::hash_file`
propagates `File::open`'s error unchanged, so an input that cannot be read
fails the whole run with:

```console
mmz: i/o error: No such file or directory (os error 2)
```

The one fact needed to act on that — which path — is the fact that gets
dropped. Every other variant in `src/error.rs` names its subject, and the
type's own doc comment sets the standard: "Every path a manifest-loading error
names is rendered the way a report renders one." A hashing error names none.

== Where it was hit

MindTape's `just check`, which runs ~20 gate arms `[parallel]`, each wrapped in
`mmz just <subgate>`. One arm went red at exit 70 with the line above, printed
*after* the wrapped command had already succeeded and said so:

```
[10:26PM] INF scanned ~7770103 bytes (7.77 MB) in 1.71s
[10:26PM] INF no leaks found
mmz: i/o error: No such file or directory (os error 2)
error: Recipe `check-secrets` failed on line 129 with exit code 70
```

So the gate passed and its memoizer is what failed the build. The next full run
and a direct re-run were both clean, and there is nothing left to go on: the
message names no file, and by the time it is read the file is back.

The rule's scope is that repo's widest — `*` plus `www/**`, `crates/**`,
`.just/**` and six more — and several concurrent arms rewrite files under it
while the walk and the hashing happen. Gitignored paths are pruned during the
walk (confirmed on that repo: touching a file under a `/generated` ignore left
the rule fresh), so the missing file was tracked or belonged to something else
entirely. Which of those is exactly what the message would have settled.

== Two defects, only one of them the message

The second is the exit code. `www/utils/exit-code-notes.typ` says 70 means
"Internal error", detail "Worth reporting: nothing a manifest can say should
produce one." An input file that vanished mid-run is not an internal error —
it is a condition of the tree, reachable from any manifest whose scopes cover a
directory something else writes. `src/main.rs` maps every `Error::Io` to 70, so
an unreadable input and a genuine bug in mmz are the same number to a caller
branching on `$?`.

That matters for the case this was found in: mmz's own use is gating a parallel
runner, where resolve-then-hash has a window by construction. A file resolved
by the walk and gone by the time `hash_file` opens it is a race the tool is
positioned to observe routinely, and the caller cannot tell it apart from mmz
being broken.

== Shape of a fix, not yet chosen

- Carry the path. Either a new variant (`Error::HashRead { path, source }`) or
  a wrap at `hash_file`/`hash_each`, rendered the way `Provenance::display`
  renders one — relative under the root, absolute otherwise — so the error and
  `--status` name a file identically, as `error.rs`'s own doc promises.
- Audit the other `?`-on-io sites for the same hole. `probe`'s `file:` read is
  the obvious sibling; `cache`'s reads already swallow with `.ok()?`.
- Decide whether `ENOENT` while hashing deserves its own message and its own
  exit code: "input `<path>` disappeared after it was resolved" names the race,
  where the errno names an accident. That is a behaviour change, so it wants
  the exit-code table and a docs line, not just a wrap.

== Test first

A test that resolves a scope, deletes one of its files, then hashes — asserting
the error names the path. The current behaviour is untested in either
direction, which is how a bare `#from` survived in a type whose every other
variant names its subject.
