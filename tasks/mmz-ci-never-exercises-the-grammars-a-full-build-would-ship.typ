#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "mmz: CI never exercises the grammars a full build would ship",
  priority: framework("ice", confidence: 0.9, ease: 7.0, impact: 6.0),
  tags: ("tests", "build"),
  links: (
    related("mmz-the-release-ships-a-rust-only-binary.typ")[the release
      this is a prerequisite of],
  ),
  status: done(
    2026,
    8,
    21,
  )[Shipped as \`just test-lang-all\` plus a \`grammars\` CI job. Deliberately not a \`just check\` arm: twenty-seven C compiles do not belong in front of the recipe people run in a loop, and \`cover\`/\`crap\` are the existing precedent for a dev recipe with its own CI job. That also keeps it out of the gate-membership cross-check, which is only interested in \`check\`'s parallel arms, so no gate note and no \`.mmz/conf.d\` rule were needed. It found a bug on its first run: \`a\_language\_this\_build\_lacks\_names\_the\_feature\_to\_rebuild\_with\` hard-coded \`kotlin\` as the absent grammar and, under \`lang-all\`, asserted a refusal that correctly did not happen. Gated on \`not(feature = "lang-kotlin")\` rather than on \`not(lang-all)\` — the precise condition is the absence of the grammar the test itself names, which also holds for a build that enabled the flags individually. \`ast\_tests.rs\` already picked whichever language the build lacked; what was only testable at the CLI level is the exit code, and that needs a build missing something. Run locally: exit 0, 19 test binaries green under \`--features lang-all\`.],
)

== Summary

Every gate runs on default features, so every gate parses Rust and nothing
else. `ast_lang_tests.rs` asserts that each `TABLE` entry really parses — the
claim that makes the table more than a comment — but the table it walks is
`cfg`-gated down to one entry.

So twenty-seven of the twenty-eight languages mmz advertises are, as far as the
build is concerned, untested. That is a defensible position while the only
build anyone downloads carries one grammar. It stops being defensible the
moment a `lang-all` binary ships under this project's name: it would be a
binary whose whole selling point is the twenty-seven grammars nothing has ever
run.

Cargo.toml already argues this in the other direction — the default set is
`lang-rust` alone "because a default set may only promise what the suite
exercises". A shipped artifact is a promise of the same kind, and it needs the
same backing.

== The shape of the fix

A `test-lang-all` recipe and a CI job that runs it. Not a `just check` arm:
that would put twenty-seven C compiles in front of every local gate run, and
`check` is the recipe people run in a loop.

`[group("dev")]` rather than `[group("gate")]`, following `cover` and `crap` —
both dev recipes that CI runs on their own job. That also keeps it out of the
gate-membership cross-check, which is only interested in the parallel arms of
`check`.

`ast_tests.rs:240` already has the branch that returns early under `lang-all`
(the "no absent language to name" case), so the suite anticipates this run.

== Notes

- The job proves `lang-all` *compiles and passes* on `x86_64-linux` only. The
  other four release targets are a separate question — see the verify task.
