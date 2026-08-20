#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "mmz: Located root is not canonical while provenance paths are",
  priority: framework("ice", confidence: 0.75, ease: 7.0, impact: 3.0),
  tags: ("config",),
  links: (
    related("mmz-report-each-rule-s-source-file-in-status.typ")[surfaced while
      reviewing the `source` field]
  ),
  status: done(
    2026,
    8,
    20,
  )[Located::at canonicalizes the config path and derives the root from it, so root and provenance share one representation; the loader gets that root and renders every path it names through Provenance::shorten. Regression tests: a symlinked root, and the duplicate-key error in both directions.],
)

== Summary

`Provenance` stores canonicalized paths; `Located.root` is built by taking the
discovered config path's parent's parent, and is never canonicalized. Rendering
a source path root-relative is a `strip_prefix` of one against the other, so the
two representations have to agree — and nothing makes them.

Where they disagree, an in-tree fragment renders as an absolute path instead of
a relative one. Cosmetic, and it fails in the readable direction, but it is an
invariant held by luck rather than by construction.

== Not currently reproducible, and that is the point

I tried to trigger it through a symlinked project root and could not: on Linux
`current_dir()` hands back the resolved physical path, so `discover` walks a
canonical chain already and the two sides happen to match.

That is exactly why this is worth a line of code rather than a shrug. The
invariant depends on a platform behaviour nobody wrote down, in a crate that
ships prebuilt binaries for several platforms and also exposes `mmz::run(&argv,
path)` as a library entry point — a caller can hand it any path it likes,
including one through a symlink, and never touch `current_dir()` at all.

== Fix

Canonicalize `root` in `Manifest::locate`, so everything downstream of
`Located` shares one representation. Globs, `outputs` and `cache_dir` all
resolve against it, so this makes those consistent too rather than only fixing
the display.

Check before landing whether any test compares `Located.root` against a
non-canonical path it constructed itself; those are the ones that will notice.

The alternative — canonicalizing inside `Provenance::display` — treats the
symptom, leaves the two representations disagreeing everywhere else, and pays
a syscall per rendered row.

== Test

A project root reached through a symlink renders an in-root fragment's source
relative, not absolute. Construct the symlink in the test rather than relying
on the ambient filesystem.

== The same thread fixes a second inconsistency

Composition errors print absolute paths, while `--status` and `--dump-config`
print the same files root-relative:

```
mmz: scope `rust` is declared in both /home/me/proj/.mmz/config.yaml and
     /home/me/proj/.mmz/conf.d/lint.yaml
```

The errors are built inside the loader, which never receives the project root —
`Manifest::locate` computes the root and then calls `load` without it — so they
have no way to shorten a path even when it sits under the root. Reports go
through `Provenance::display`, which does.

Nobody is misled: an absolute path is unambiguous, and the errors read fine.
But it means the docs cannot show a real error capture without either pasting
someone's home directory or normalising it away, which is what the composition
page does today (following `www/generate.sh`'s established sed). Threading the
root into the loader closes both this and the canonicalization mismatch above,
which is why they are one task rather than two.

Weigh one thing before doing it: an absolute path is the more useful form when
the error is about a file *outside* the root, which is exactly the store-path
case composition exists to support. `Provenance::display`'s rule — relative
under the root, absolute otherwise — already handles that correctly, so reusing
it rather than inventing a second rule is the whole job.
