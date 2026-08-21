#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "mmz: binary-size figures in the docs are hand-typed",
  priority: framework("ice", confidence: 0.9, ease: 4.0, impact: 6.0),
  tags: ("docs", "build"),
  links: (
    related("mmz-the-release-ships-a-rust-only-binary.typ")[the flavour split
      that made the stale figure visible],
    related("mmz-generated-docs-and-tola-site.typ")[the pipeline these facts
      have to join],
  ),
  status: done(
    2026,
    8,
    21,
  )[Measured, then read. \`just measure-sizes\` links the binary thirty times — the grammar-free baseline, \`default\`, \`lang-all\`, and each grammar as its own delta against the baseline — and commits \`www/sizes.yaml\`. \`www/generate-facts.sh\` republishes it as a fifth fact, and \`www/utils/sizes.typ\` is what a page reads.

    The stale figure is confirmed stale: \`default\` is 5.5 MB, not the 3.5 MB the page claimed. The others held up — \`json\` 164 KB against a claimed 160 KB, \`kotlin\` 5.9 MB against 5.8 MB — which is why one-token-correcting the first would have been the wrong shape of fix, not merely an incomplete one. Full is 45.4 MB, and all twenty-seven grammars come to 41.2 MB over a 4.2 MB binary carrying none.

    Three guards, none of them a gate that rebuilds: \`www/sizes.jq\` refuses a measurement whose grammar set disagrees with the crate\\x27s \`lang-\` features, naming both sides; \`outdatty\`\\x27s \`binary-size\` group asks a human to re-confirm once Cargo.toml or Cargo.lock moves under a recorded measurement; and the extremes the prose quotes (\`cheapest\`, \`dearest\`) are derived, so a re-measurement that reorders them rewrites the sentence rather than falsifying it.

    \`Cargo.toml\`, \`src/ast\_lang.rs\` and both JSON Schemas now make the argument without quoting a number, since none of them can read one. What is deliberately NOT shipped: the twenty-seven-row table on the page. \`code.typ\` and \`inputs.typ\` both sit at the 250-line Typst cap, which is the repo saying the page is full — the data is committed and published in \`sizes.json\`, so rendering it later is a page, not a re-measurement.],
)

== Summary

`www/content/code.typ` argues that a grammar is not small, and prices the
argument: twenty-seven grammars at "about 40 MB linked, against an mmz binary
of 3.5 MB", `json` at 160 KB, `kotlin` at 5.8 MB. Cargo.toml's `[features]`
header repeats the first two numbers, and the JSON Schema descriptions repeat
the 40 MB.

Every one of them was typed by hand, and the first has already rotted: no build
has produced a 3.5 MB binary since the jq and ast-grep engines landed. A
measured `nix build .\#default` is 5.53 MB.

This is the exact drift this repo already has a rule against — a doc must STATE
a fact by reading it, never by repeating it — and the machinery is already
here. It is the one fact class `www/generate-facts.sh` does not cover.

== Why it is not a one-token fix

The stale number sits in a sentence carrying three other measured figures.
Correcting one makes it inconsistent with its neighbours, and the neighbours
were measured under conditions nobody wrote down: the release profile, but on
which host, at which toolchain, against which baseline? `rust` cannot even be
priced as a delta against the default build, because it *is* the default build.

So the fix is a measurement, not an edit.

== Shape

- A script that measures every flavour the docs talk about — the grammar-free
  baseline, `default`, `lang-all`, and each grammar as its own delta against
  the baseline — and commits the result with the toolchain and host it was
  taken under.
- Not a `just check` arm: it links the binary thirty times under LTO. It runs
  on demand, and `outdatty` schedules the re-run when Cargo.toml or Cargo.lock
  moves under a recorded measurement.
- `www/generate-facts.sh` republishes it into `www/generated/`, cross-checking
  the recorded grammar set against the crate's own `lang-` features — so a
  grammar added without a re-measure fails the docs build naming it, rather
  than rendering a table that quietly omits it.
- The prose then reads the fact. Cargo.toml and the schema descriptions cannot
  read a file, so they stop quoting numbers and point at the source instead.
