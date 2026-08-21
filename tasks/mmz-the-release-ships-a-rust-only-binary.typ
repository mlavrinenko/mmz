#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "mmz: the release ships a rust-only binary",
  priority: framework("ice", confidence: 0.8, ease: 5.0, impact: 7.0),
  tags: ("build", "docs"),
  links: (
    depends-on(
      "mmz-version-does-not-say-which-grammars-the-build-carries.typ",
    )[two
      assets sharing a version need a way to tell them apart],
    depends-on(
      "mmz-ci-never-exercises-the-grammars-a-full-build-would-ship.typ",
    )[a
      shipped grammar set has to be a tested one],
    related("mmz-verify-the-lang-all-release-build-on-every-target.typ")[the
      manual run that proves the new matrix],
  ),
  status: done(
    2026,
    8,
    21,
  )[The matrix is two flavours across the same five targets: \`mmz-\<target>\` on default features and \`mmz-full-\<target>\` on \`--features lang-all\`. The plain name stays with the default build, so the asset a reader reaches for first is the one that matches \`cargo install mmz\`; no \`min\` name was minted, since a second name for the same bytes is a question to answer before downloading. \`nix build .\#full\` is the Nix spelling of the same choice, and an overlay overriding \`cargoBuildOptions\` covers every subset in between — which is the freedom a prebuilt asset cannot offer and the reason no curated middle tier was worth a third matrix leg.

    Each leg verifies its own output before uploading: it runs the binary it just built and matches \`--version\` against the count the flavour implies. Without that, a matrix wiring mistake ships two identical binaries under two names and nothing downstream notices — file size is the only other tell and nobody reads it. Skipped on the aarch64 cross leg, which cannot run what it produced.

    What is proven and what is not: \`lang-all\` compiles and passes its suite on x86\_64-linux (the CI grammar job, run locally at exit 0). The other four targets are unproven until the workflow runs — all twenty-seven tree-sitter scanners are C rather than C++, checked across the vendored crates, so the existing \`gcc-aarch64-linux-gnu\` covers the cross leg, but MSVC going from one grammar to twenty-seven is untested. Filed as the sibling verify task rather than claimed here.

    Assets are still uncompressed. Fine at 3.5 MB, wasteful at 44 MB; left as a follow-up rather than folded in, since it changes asset names for existing install scripts.],
)

== Summary

`cargo build --release` in the release workflow means default features, so
every published binary parses Rust and nothing else. Anyone whose manifest
names another language must have a Rust toolchain and rebuild — and the people
downloading a prebuilt binary are, definitionally, the people who did not want
to build one. CI runners and install scripts are the whole audience for these
assets, and they are exactly who this leaves stuck.

== The decision

Two flavours, not three:

#table(
  columns: 2,
  [`mmz-<target>`],
  [default features (`lang-rust`), \~3.5 MB — the same
    binary `cargo install mmz` gives you],

  [`mmz-full-<target>`],
  [`lang-all`, \~44 MB — every grammar, never a
    rebuild],
)

A middle "popular" tier was considered and rejected. Its promise is a coin
flip: `min` fails predictably and `full` never fails, but a curated set fails
for *some* of your colleagues and not others — which is the trap
#link("https://mlavrinenko.github.io/mmz/code/")[the docs already warn about],
promoted to a supported product. It is also a taste judgement that drifts, it
costs a third five-target matrix leg, and a 10-grammar set still lands most of
the way to `lang-all`'s size. Anyone wanting a specific subset has
`cargo install --features`, which beats any tier we could pick.

No `min` name: the plain asset *is* the default, and a second name for the same
bytes is a question a reader has to answer before downloading.

== Notes

- 5 targets x 2 flavours = 10 build legs. `crates.io` stays on default
  features; nothing about the publish changes.
- Every tree-sitter scanner in the dependency set is C, none C++ — checked
  across all 27 crates — so the existing `gcc-aarch64-linux-gnu` install covers
  the cross leg. `windows-msvc` compiling 27 grammars is the untested surface.
- Assets are uncompressed today. Fine at 3.5 MB, wasteful at 44 MB; worth a
  follow-up, not a blocker.
