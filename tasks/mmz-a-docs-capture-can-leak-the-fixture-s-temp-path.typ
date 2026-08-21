#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "mmz: a docs capture can leak the fixture's temp path",
  priority: framework("ice", confidence: 0.85, ease: 8.0, impact: 4.0),
  tags: ("docs", "gating"),
  links: (
    related("mmz-an-empty-tag-selection-passes-the-gate.typ")[a capture added
      there leaked one until it was caught by hand],
  ),
  status: done(2026, 8, 20),
)

== Summary

`www/generate.sh` runs the real binary against a throwaway copy of
`examples/demo` under `$TMPDIR`, so any capture of a command that prints a path
carries that build's temp directory verbatim. Nothing catches it. The site is
supposed to be reproducible from a clean checkout, and a capture naming
`/tmp/tmp.BIg78u4JqB/demo/.mmz/config.yaml` is neither reproducible nor
readable.

`www/generated/status-json.txt` carries one today:

```console
$ head -3 www/generated/status-json.txt
{
  "manifest": "/tmp/tmp.BIg78u4JqB/demo/.mmz/config.yaml",
```

No page renders that file, which is why it has gone unnoticed. The next capture
of a path-printing command is the one that ships it.

== Why the existing gates do not see it

`docs-check` builds and validates links; `docs-md-check` diffs generated
Markdown against its Typst source. Neither reads a capture's bytes, and neither
can: a capture is stdout, and stdout is the thing being trusted. The
determinism discipline is entirely in `generate.sh`'s header comment and in two
hand-placed `sed` calls — a comment and a convention, with nothing enforcing
either.

The clock got the treatment this wants: `$MMZ_NOW` pins it, so `ran_at` and
every `AGE` are fixed rather than corrected afterwards. The fixture's location
is the other non-repo input to a capture, and it has no equivalent.

== Shape of a fix, not yet chosen

An arm in `docs-check` (or its own gate) failing when any file under
`www/generated/` matches the temp-directory shape:

```bash
grep -rlE '/tmp/tmp\.[A-Za-z0-9]+' www/generated/ && exit 1
```

Two things to settle before writing it:

- Whether to match `$TMPDIR`/`/tmp/tmp.*` (cheap, catches what mktemp actually
  produces) or to pass the run's own `$COPY` and `$INIT_DIR` down and grep for
  those (exact, and cannot be defeated by a `TMPDIR` set somewhere else). The
  second is the honest version of the check; the first is the one that fits on
  a line.
- Whether the fix is a gate at all, or normalization at the source: `gen_run`
  could pipe every capture through the `sed` unconditionally, which would make
  the leak unreachable rather than caught. That trades the header comment's
  "every other byte is stdout as written" guarantee for one nobody can break,
  and the argument between those two is the decision.

Either way `status-json.txt`'s existing leak has to be normalized in the same
change, or the new gate fails on arrival.

== Test first

The gate is its own test if it is written second: add the arm, watch it fail on
the committed `status-json.txt`, then normalize that capture and watch it pass.
A regenerate-then-check gate needs the failing side demonstrated, since a check
that only ever ran green proves nothing about what it would catch.
