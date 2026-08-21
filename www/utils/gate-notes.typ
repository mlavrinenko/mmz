// Hand-written prose for every `[group("gate")]` recipe in the Justfile. The
// gate table in CONTRIBUTING.md renders each gate's NAME and COMMAND straight
// off `just --dump` (www/gates.jq), and this note beside them;
// `just check-doc-facts` asserts every derived gate has an entry, so a gate
// cannot join `just check` without a line saying what it is for.
//
// Keyed by the gate's identifier — the invocation path with `::` mangled to `-`,
// so a recipe moving into a module changes no rendered byte.
//
// What belongs here is what the recipe body cannot say: which failure the gate
// exists to prevent. Not how to run it (the Command column has that), and not
// what the tool is (its own docs have that).

#let gate-notes = (
  "fmt-check": [
    Formatting is settled by a tool, not in review. Covers Rust, the Typst docs
    sources, and the Justfiles — one gate, so a formatted-by-one-tool tree
    cannot pass while another tool's files drift.
  ],
  clippy: [
    Every lint in `Cargo.toml`'s `[workspace.lints]` is `deny`, so this is not a
    style pass: it is the compile-time half of the correctness contract, and it
    fails the build rather than warning into a log nobody reads.
  ],
  test: [
    The whole suite — inline unit tests plus the CLI integration tests that drive
    the real binary through `assert_cmd`.
  ],
  machete: [
    A dependency nobody imports is still a dependency somebody audits, builds and
    ships. Catches the ones a refactor orphaned.
  ],
  "check-file-size": [
    Caps every file at the limit `.linecop.yaml` sets for its language. The point
    is not tidiness: a file nobody can hold in their head is where the untested
    branch hides.
  ],
  "outdatty-check": [
    Fails when a source changed and the dependents `outdatty.yaml` couples to it
    were not re-confirmed. It cannot check that a doc is _correct_ — only that a
    human looked since the code moved.
  ],
  "check-changelog-history": [
    Fails when a `## [x.y.z]` section stops saying what its `vx.y.z` tag shipped.
    A released section is a historical record, and the edit that swallowed one
    here went unnoticed for thirteen commits. A deliberate rewrite is recorded
    in `CHANGELOG.waivers` by `just changelog-waive`.
  ],
  "check-doc-coverage": [
    Fails when `mmz --help` advertises an action with no hand-written note, or a
    note names an action the binary no longer has. The list is parsed out of the
    binary, so it cannot be satisfied by editing a list.
  ],
  "check-doc-facts": [
    The same set-difference over the derived facts: a manifest key with no prose,
    a gate with no prose, a page unreachable from the sidebar, a sidebar entry
    pointing at no page.
  ],
  "docs-check": [
    Builds the docs site and validates every internal link and asset reference.
    A cross-page link is a string until something resolves it; this is the
    something.
  ],
  "docs-md-check": [
    Fails when a committed `README.md`, `AGENTS.md`, `CONTRIBUTING.md` or
    `docs/contributing/*.md` has drifted from the `docs/src/*.typ` source it is
    rendered from — which is what a hand-edit of a generated file looks like.
    The regenerate goes to a temp directory precisely so a hand-corrupted
    committed file cannot be healed by the check that is supposed to catch it.
  ],
)
