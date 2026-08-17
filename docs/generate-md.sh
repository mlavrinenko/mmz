#!/usr/bin/env bash
# docs/generate-md.sh — render docs/src/*.typ sources to their declared Markdown
# outputs with typlite.
#
# The one job every docs/src source shares: read whatever facts it needs (the
# crate map, the gate table, the linecop caps, the config schema — see
# www/generate-facts.sh and www/generate.sh), write ordinary Typst markup, and
# carry an `<mmz-md>` metadata block naming its own output path. This script
# never hardcodes which source goes where — it globs docs/src/**/*.typ, reads
# each source's own block, and writes there. Adding a fifth or sixth source
# touches this file not at all.
#
# `www/generate.sh` runs FIRST, unconditionally: a source that reads
# `www/generated/*` must never depend on some other recipe having run first in
# this process. That costs a `cargo build --release --bin mmz`, because the page
# manifest a source resolves its docs-site links against is derived by compiling
# every www/content page, which needs generate.sh's own captures. Inside
# `just check` the cost is free — docs::check already ran generate.sh and both
# share target/www-generated.lock.
#
# typlite (nixpkgs' tinymist, `$out/bin/typlite`; see flake.nix for the pin) is a
# render path SEPARATE from the www/content/*.typ docs site — those pages carry a
# tola `page` show rule and templates typlite cannot parse. Every constraint
# typlite imposes on a source (the `<h1>` workaround, the silently-stripped
# `<p>`/`<div>`/`<span>`, the raw-span line-break trap) is argued in
# docs/src/lib.typ's header and in docs/contributing/generated-docs.md — read
# either before adding a source that trips one.
#
# Reading a source's `<mmz-md>` block means actually EVALUATING it, not grepping
# the file for `output:` (a source's Typst code could compute the value, as
# docs/src/readme.typ's derived page list does elsewhere in the same file — a
# regex has no way to know that is even the same kind of expression).
# `typst query` is the read-back tool every other generator here already uses;
# `--target html --features html` is required because every source imports
# docs/src/lib.typ, which calls `html.elem` — that function does not exist in the
# query's evaluation scope without both flags.
#
# A source with no `<mmz-md>` block (docs/src/lib.typ, or any future helper
# module) queries back zero matches — `--one` cannot express "zero or one", so
# this script queries WITHOUT it and checks the match count itself, skipping on
# zero and failing loudly on more than one.
set -eo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$SCRIPT_DIR/.." && pwd)"
# Where rendered outputs land. docs::md-check (.just/docs.just) passes a temp
# directory here so it can diff a fresh render against the committed tree
# without ever writing into it — see that recipe's own comment for why an
# in-place regenerate-then-diff cannot be trusted.
OUT_ROOT="${1:-$REPO}"

cd "$REPO"

# Hold the www/generated/ lock for this whole script, not just the regenerate
# below. Every source rendered here READS that directory — the fact files via
# docs/src/lib.typ's `fact`, and the recipe names via its generated `just`
# module — and www/generate.sh (the docs::check arm, running concurrently under
# `just check`'s parallel arms) `rm -rf`s the directory on entry. Regenerating
# under the lock and then releasing it before the render leaves exactly the
# window that produces "file not found (searched at www/generated/just.typ)" on
# a tree that is perfectly correct. The lock is held until fd 9 closes at exit;
# MMZ_GENERATED_LOCK tells the nested www/generate.sh call (and, through it,
# generate-facts.sh and generate-site-pages.sh) that this process already owns
# the lock, so none of them tries to acquire it again — which would deadlock,
# since this process is waiting on that call to return.
mkdir -p target
exec 9>"target/www-generated.lock"
flock 9
export MMZ_GENERATED_LOCK=1

bash www/generate.sh

BANNER_TMPL='<!-- Generated from %s by `just docs md`. Do not edit; edit the source. -->'

shopt -s globstar nullglob
err="$(mktemp)"
trap 'rm -f "$err"' EXIT

rendered=0
for src in docs/src/**/*.typ; do
  [ "$(basename "$src")" = "lib.typ" ] && continue

  if ! matches="$(typst query --root "$REPO" --target html --features html \
      "$src" '<mmz-md>' --field value 2>"$err")"; then
    echo "docs/generate-md.sh: typst query failed on $src:" >&2
    cat "$err" >&2
    exit 1
  fi
  n="$(jq 'length' <<<"$matches")"
  [ "$n" -eq 0 ] && continue
  if [ "$n" -gt 1 ]; then
    echo "docs/generate-md.sh: $src carries $n <mmz-md> blocks, want at most 1" >&2
    exit 1
  fi

  output="$(jq -r '.[0].output' <<<"$matches")"
  dest="$OUT_ROOT/$output"
  mkdir -p "$(dirname "$dest")"

  body="$(mktemp)"
  typlite --root "$REPO" "$src" "$body"

  # Demote `doc-title`'s HTML h1 back to an ATX `#` heading.
  #
  # typlite maps `=` to `##` and offers no way back to `#` (see docs/src/lib.typ),
  # so `doc-title` goes through `html.elem("h1")` — the one element typlite
  # preserves. That is a rendering workaround, not an authoring intent: every one
  # of these files is read as RAW TEXT far more often than rendered (README in a
  # terminal, AGENTS.md straight into an agent context window), and a raw `<h1>`
  # tag there is worse than the `#` the author wrote.
  #
  # Deliberately anchored and attribute-free: only a whole line that is exactly
  # `<h1>…</h1>` converts. An h1 CARRYING attributes is doing something markdown
  # cannot express (README centers its header with `<h1 align="center">`, which
  # both typlite and the GitHub sanitizer keep) and is left alone. Line 1 only:
  # the title is the first thing typlite emits for these sources. If a source
  # ever puts content ahead of its title, nothing converts and the raw <h1>
  # survives — a visible no-op, never a silent mangling.
  sed -i -E '1s|^<h1>([^<>]*)</h1>$|# \1|' "$body"
  {
    # shellcheck disable=SC2059 # BANNER_TMPL is a fixed, script-local format
    printf "$BANNER_TMPL\n" "$src"
    echo
    cat "$body"
  } >"$dest"
  rm -f "$body"

  rendered=$((rendered + 1))
done

echo "docs-md: rendered $rendered source(s) under $OUT_ROOT"
