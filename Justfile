set quiet
set unstable
set script-interpreter := ["bash", "-eo", "pipefail"]

# Locally built mmz, used to memoize check's own arms (dogfood). mmz is the
# crate under development here, so it is not on the dev shell's PATH the way it
# is in a consuming project — the `_mmz` recipe keeps this copy current, and
# `.mmz/config.yaml` declares which recipes it memoizes.
mmz := "target/debug/mmz"

# Typst sources the formatter owns: the docs-site pages and helpers, the
# docs/src Markdown sources, and the MindTape task artifacts. Kept as one list
# so `fmt` and `fmt-check` can never format different sets.
TYPST_FMT_PATHS := "www/content www/templates www/utils docs/src tasks"

# `just --fmt` does not descend into module files, so the formatter gate points
# `--justfile` at each one explicitly.
JUST_FMT_PATHS := ".just/docs.just"

# Docs site generation, build, serve, and the docs-md/docs-check gates. The path
# is spelled out because the module file does not sit beside this one: a bare
# `mod docs` only looks for `docs.just`/`docs/mod.just` next to the file
# declaring it.
mod docs '.just/docs.just'

[doc("List available recipes")]
[group("workflow")]
default:
    @just --list

[doc("Run fixes, then run the full check gate")]
[group("workflow")]
fix-check: eject fmt clippy-fix check

# Every arm is `just memo <gate>`, so a no-op re-run skips each gate whose
# declared inputs are unchanged. Gate membership is derived from the
# `[group("gate")]` tags below and cross-checked against THIS dependency list
# (www/gates.jq): a gate tagged but not listed here, or listed but not tagged,
# fails the docs build naming both sides. The list's order is also the gate
# table's row order in CONTRIBUTING.md, so reordering is a deliberate edit here
# rather than a side effect of where a recipe sits in the file.

[doc("Run every check-gate sub-recipe in parallel")]
[group("workflow")]
[parallel]
check: (memo "fmt-check") (memo "clippy") (memo "test") (memo "machete") (memo "check-file-size") (memo "outdatty-check") (memo "check-doc-coverage") (memo "check-doc-facts") (memo "docs::check") (memo "docs::md-check")

# Wrap a just recipe in mmz: `just memo fmt-check` runs `mmz just fmt-check`, so
# the rule named `just fmt-check` in .mmz/config.yaml decides whether the recipe
# body runs at all. The double hop is what lets .mmz/config.yaml name RECIPES
# rather than mirror their command lines by hand — the mirror that used to drift
# every time a recipe grew a flag.
#
# `just test <filter>` deliberately bypasses this: a filtered test run is a
# different command from the gate's full-suite run, and memoizing it under the
# gate's identity would record a pass the gate never earned.

[doc("Run a just recipe through mmz memoization")]
[group("workflow")]
[no-exit-message]
memo *ARGS: _mmz
    {{ mmz }} just {{ ARGS }}

# Build the mmz binary `check` memoizes its own arms with (dogfood).
_mmz:
    cargo build -q --bin mmz

[doc("Check formatting without writing changes (CI-friendly)")]
[group("gate")]
fmt-check:
    cargo fmt --all -- --check
    typstyle --check {{ TYPST_FMT_PATHS }}
    just --fmt --check
    for f in {{ JUST_FMT_PATHS }}; do just --justfile "$f" --fmt --check; done

[doc("Format Rust, Typst and the Justfiles")]
[group("dev")]
fmt:
    cargo fmt --all
    typstyle --inplace {{ TYPST_FMT_PATHS }}
    just --fmt
    for f in {{ JUST_FMT_PATHS }}; do just --justfile "$f" --fmt; done

[doc("Run clippy only")]
[group("dev")]
[group("gate")]
clippy:
    cargo clippy --workspace --all-targets -q -- -D warnings

[doc("Auto-fix clippy warnings")]
[group("dev")]
clippy-fix:
    cargo clippy --fix --workspace --all-targets --allow-dirty --allow-staged -- -D warnings

[doc("Run the test suite")]
[group("dev")]
[group("gate")]
[positional-arguments]
test *ARGS:
    cargo test --workspace "$@"

# The suite against every grammar, which is what the release's `full` binary
# carries. Deliberately NOT a `[group("gate")]` arm: it puts twenty-seven C
# compiles in front of a recipe people run in a loop, and `just check` is that
# recipe. It runs on its own CI job instead, the way `cover` and `crap` do.
#
# What it buys: `ast_lang_tests.rs` asserts every TABLE entry really parses, and
# the table it walks is `cfg`-gated down to whatever the build enabled — so on
# default features that claim covers one language out of twenty-eight. A binary
# whose selling point is the other twenty-seven has to have run them.

[doc("Run the test suite with every grammar compiled in")]
[group("dev")]
test-lang-all:
    cargo test --workspace --features lang-all

# Re-measure what each grammar costs a linked binary, into www/sizes.yaml — the
# file every binary-size figure in the docs is read from. Thirty release builds,
# each a full LTO link, so about half an hour: deliberately not a gate and not a
# `check` arm, the way `cover` and `test-lang-all` are not.
#
# What asks for it instead is `just outdatty-check`, whose `binary-size` group
# fails once Cargo.toml or Cargo.lock has moved under the recorded measurement.
# That is the right trigger, because it is a human who judges whether a
# dependency bump moved the number enough to be worth half an hour.

[doc("Re-measure binary size per grammar into www/sizes.yaml")]
[group("dev")]
measure-sizes:
    bash www/measure-sizes.sh

[doc("Check for unused dependencies")]
[group("gate")]
machete:
    cargo machete

[doc("Fail if any file exceeds its linecop cap")]
[group("gate")]
check-file-size:
    linecop

[doc("Fail if a source changed without its dependents reconfirmed")]
[group("gate")]
outdatty-check:
    outdatty check

# Every action `mmz --help` advertises must carry a hand-written
# www/utils/cli-notes.typ entry, so a new flag cannot ship undocumented. The
# arms, and why the gate parses the binary's own help text rather than a list,
# are in .just/scripts/check-doc-coverage.sh.

[doc("Fail if a CLI action has no hand-written note")]
[group("gate")]
check-doc-coverage:
    bash .just/scripts/check-doc-coverage.sh

# Every derived fact the generators write must have hand-written prose behind
# it: a manifest key with no note, a gate with no note, a page with no NAV
# entry. See .just/scripts/check-doc-facts.sh.

[doc("Fail if a derived doc fact has no hand-written prose")]
[group("gate")]
check-doc-facts:
    bash .just/scripts/check-doc-facts.sh

[doc("Re-confirm dependency groups into outdatty.lock")]
[group("dev")]
outdatty-update *ARGS:
    outdatty update {{ ARGS }}

[doc("Build the project")]
[group("dev")]
build *ARGS:
    cargo build --workspace -q {{ ARGS }}

[doc("Run coverage with tarpaulin (also writes target/coverage/lcov.info)")]
[group("dev")]
cover:
    cargo tarpaulin --workspace --skip-clean

# Gate complex, undertested functions via the CRAP metric. Needs the lcov `just
# cover` writes. Threshold 30 is a sane greenfield default; tune per repo.

[doc("Gate complex, undertested functions via the CRAP metric")]
[group("dev")]
crap:
    cargo crap --lcov target/coverage/lcov.info --workspace --exclude 'src/main.rs' --threshold 30 --fail-above

# Eject inline tests from Rust files nearing the linecop limit, so they stay
# under it without losing the inline-test workflow. Runs as part of `fix-check`.

[doc("Eject inline tests from Rust files nearing the linecop cap")]
[group("dev")]
eject PCT='90':
    linecop --baseline {{ PCT }} --format paths | ejectest apply src --files-from - --lenient

[doc("Show the top 20 files by line count")]
[group("dev")]
[script]
file-sizes:
    find . -type f \( -name '*.rs' -o -name '*.md' -o -name '*.typ' \) \
        ! -path './target/*' ! -path './www/generated/*' \
        -exec wc -l {} + | sort -rn | head -20

[doc("Tag a release and push (usage: just release 0.1.0)")]
[group("workflow")]
[script]
release VERSION:
    set -eo pipefail
    cargo_version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
    if [ "{{ VERSION }}" != "$cargo_version" ]; then
        echo "error: requested v{{ VERSION }} but Cargo.toml is $cargo_version; bump Cargo.toml first" >&2
        exit 1
    fi
    just check
    git tag -a "v{{ VERSION }}" -m "v{{ VERSION }}"
    git push origin "v{{ VERSION }}"
