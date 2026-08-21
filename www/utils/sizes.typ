// www/utils/sizes.typ — what an mmz binary weighs, and what each grammar adds
// to it. Read from www/generated/sizes.json, never typed into a page.
//
// The figures used to be hand-written prose, and by the time anyone checked,
// the page was claiming a 3.5 MB binary that no build had produced in two
// releases. The whole "a grammar is not small" argument rests on these numbers,
// so an argument built on a remembered one is worse than no number at all.
//
// The chain behind this file: www/measure-sizes.sh links the binary once per
// flavour and commits www/sizes.yaml; www/generate-facts.sh cross-checks that
// measurement against the crate's own `lang-` features and formats the bytes
// into the strings below. Nothing here does arithmetic — a page that divided
// bytes itself would be four pages rounding four ways.
//
// Not read from www/sizes.yaml directly, for the same reason site.typ does not
// read Cargo.toml: the site builds under `--root www`, and the cross-check that
// makes the measurement trustworthy happens on the way through the generator.

#let SIZES = json("../generated/sizes.json")

// The three whole binaries, each `(bytes, text)`:
//   `none`    no grammar at all — not a build anyone ships, but the baseline
//             every per-grammar cost below is a difference against
//   `default`  what `cargo install mmz` and the plain release asset give you
//   `full`     `--features lang-all`, the `mmz-full-<target>` asset
#let builds = SIZES.builds

// What every grammar costs together — measured as `full` minus `none`, not as
// the sum of the per-grammar entries. Each single-grammar build pays for the
// shared tree-sitter runtime, so adding them up counts it twenty-seven times.
#let all-grammars = SIZES.all_grammars

// Grammar count, and the extremes of the ranking. Derived rather than re-read
// by a human: "the cheapest is json" is a claim about an ordering, and an
// ordering is exactly what a re-measurement is allowed to change.
#let count = SIZES.count
#let cheapest = SIZES.cheapest
#let dearest = SIZES.dearest

// The full ranking, cheapest first — `(name, bytes, text)` per grammar, for a
// page that wants to price a specific language rather than the extremes.
#let grammars = SIZES.grammars
