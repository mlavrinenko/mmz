#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "mmz: --version does not say which grammars the build carries",
  priority: framework("ice", confidence: 0.9, ease: 8.0, impact: 5.0),
  tags: ("cli", "docs"),
  links: (
    related("mmz-the-release-ships-a-rust-only-binary.typ")[the release
      this is a prerequisite of],
  ),
  status: done(
    2026,
    8,
    21,
  )[Shipped as a count rather than a list: \`mmz 0.7.0 (1 ast lang)\`, \`mmz 0.7.0 (28 ast langs)\`. The pluralisation and the count live in \`ast::language\_summary()\` — in the library rather than in \`main.rs\`, because it is the only branch and \`main.rs\` is excluded from coverage. \`ast\_lang::count()\` counts TABLE entries, so the number is the languages a manifest may write under \`lang:\` (28), not the grammar crates linked (27); \`typescript\` and \`tsx\` are two of the first and one of the second. Which languages specifically stayed where it already was, in the error naming all of them beside the flag to rebuild with — the count answers "which binary is this", the error answers "what do I do about it", and only the second wants a list. The help banner is untouched: \`USAGE\` is a \`concat!\` const and could not carry a runtime count without becoming a format call. Tests moved out of \`cli.rs\`, which sat exactly on the 500-line Rust cap, into \`tests/cli\_version.rs\`. The noun-agreement test checks the plural against mmz's own count rather than a constant, so it holds under any feature set a contributor builds; the exact 28 is asserted only under \`lang-all\`, the one set whose answer is knowable there. Verified against both binaries built locally.],
)

== Summary

`mmz --version` prints `mmz 0.7.0` and nothing else. Which grammars the binary
carries is a compile-time choice, so two builds of the *same version* parse
different languages — and there is no way to ask a binary which one it is.

Today that is survivable, because there is one published binary per version and
a `cargo install` you chose the features for yourself. It stops being
survivable the moment the release ships a second flavour: a bug report saying
"mmz 0.8.0 cannot parse Python" would name a version that is true of two
different binaries, and the first question back is one the reporter has no
command to answer.

The list already exists. `ast_lang::available()` renders it, and it is
reachable from exactly one place: the error you get for naming a language this
build lacks. You have to provoke a failure to learn what your own binary can do.

== The shape of the fix

A count, not a list. Twenty-eight names would swamp a line whose job is to say
which version this is:

```
mmz 0.8.0 (1 ast lang)
mmz 0.8.0 (28 ast langs)
```

That is enough to tell the flavours apart, which is the whole job here — and
the full list stays where it is already useful, in the error that names the
language you asked for and the flag that would add it.

"langs" rather than "grammars" is deliberate: the count is `TABLE` entries, the
names a manifest may write under `lang:`. That is 28 for a full build against
27 grammar crates, because `typescript` and `tsx` share one.

== Notes

- `USAGE` is a `concat!` const, so the help banner cannot carry a runtime
  count without becoming a format call. Leave it; `--version` is the line that
  answers this question.
- `--no-default-features` is a real build and must read sensibly: `(0 ast langs)`.
