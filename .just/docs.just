# .just/docs.just — the `docs` module (`mod docs '.just/docs.just'` in the
# Justfile). That declaration spells the path out because this file does not sit
# beside the Justfile: a bare `mod docs` only looks for `docs.just` or
# `docs/mod.just` next to the file declaring it.
#
# `set working-directory := '..'` below is load-bearing, not cosmetic. A module
# recipe runs with its working directory set to the MODULE FILE's directory, not
# the repo root. Every recipe body here shells out with a repo-root-relative
# path (`www/generate.sh`, `docs/generate-md.sh`, …), and
# `[working-directory("www")]` resolves relative to that same directory — so
# without the setting the bodies miss their scripts and the attribute looks for
# `.just/www`. With it, a plain recipe runs at the repo root and
# `[working-directory("www")]` composes on top to give `<repo>/www`.
#
# Invoked as `just docs::<name>` or `just docs <name>` — both work.
# www/generate-facts.sh mangles the `::` invocation path to `-` for
# www/generated/just.typ, so `docs::md` mangles to `docs-md` — the identifier
# every doc source calls via `#just.docs-md()`. That mangling is what keeps this
# module boundary from touching doc prose.
#
# Settings are per-file, not inherited from the Justfile that declares `mod
# docs`, so `set quiet` is redeclared here to keep the docs::check /
# docs::md-check arms as quiet as every other gate. `set unstable := true` is
# what makes this file valid as a STANDALONE justfile: `--fmt` does not descend
# into module files, so the formatter gate points `--justfile` at this file
# directly — and `--fmt` is itself unstable, read from the file it is pointed AT.
# Delete that line and this file silently drops out of `just fmt`/`fmt-check`.

set quiet
set unstable
set working-directory := '..'

# Capture live `mmz` output for the docs (www/generated/*), so the site's
# transcripts, CLI reference and manifest reference cannot drift from the
# binary. Builds mmz release, runs it against a throwaway copy of
# examples/demo, then derives the facts and the page manifest.
#
# generate.sh calls generate-facts.sh and generate-site-pages.sh at its own end,
# in that order and never before: it `rm -rf`s the whole generated/ dir on entry,
# which would erase anything written first, and the page manifest is a full
# compile of every www/content/*.typ page, which needs both the facts and
# generate.sh's own captures. The explicit second call below is a redundant,
# idempotent, lock-protected re-run — it keeps this recipe honest about what it
# produces without depending on generate.sh's trailing call to have been the
# last word.

[doc("Capture live mmz output and derive doc facts")]
[group("docs")]
gen:
    bash www/generate.sh
    bash www/generate-facts.sh

# Build the documentation site (www/ -> www/public/mmz) with tola, then index
# the built HTML with Pagefind (bundle -> public/mmz/pagefind/). gen runs first
# so the injected transcripts reflect the current binary.

[doc("Build the docs site and index it with Pagefind")]
[group("docs")]
[working-directory("www")]
build: gen
    tola build && pagefind --site public/mmz --silent

[doc("Serve the docs locally with hot reload (no search index)")]
[group("docs")]
[working-directory("www")]
serve: gen
    tola serve

# Build the docs, index with Pagefind, then validate internal links + assets.
# Pagefind runs before validate so the /pagefind/ bundle exists on disk.

[doc("Build, index, and validate the docs site")]
[group("docs")]
[group("gate")]
[working-directory("www")]
check: gen
    tola build && pagefind --site public/mmz --silent && tola validate

# Render docs/src/*.typ to the Markdown each source declares (README.md,
# AGENTS.md, CONTRIBUTING.md, docs/contributing/*.md) with typlite. See
# docs/generate-md.sh's header for what that render path can and cannot do, and
# docs/contributing/generated-docs.md for the pipeline this feeds.

[doc("Render docs/src Typst sources to repo-root Markdown")]
[group("docs")]
md:
    bash docs/generate-md.sh

# Freshness gate for md: regenerate into a TEMP directory and diff each rendered
# file there against the COMMITTED copy at its real path — never regenerate in
# place and read `git status`. An in-place regenerate would heal a hand-corrupted
# committed file before the diff ever ran, so the gate would read clean on a fix
# that is no longer actually represented; a fresh temp render diffed against the
# untouched committed tree cannot self-heal that way, so a drifted file fails by
# name every time.

[doc("Fail if generated Markdown has drifted from docs/src")]
[group("docs")]
[group("gate")]
md-check:
    bash .just/scripts/docs-md-check.sh
