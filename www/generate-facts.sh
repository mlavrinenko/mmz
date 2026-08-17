#!/usr/bin/env bash
# www/generate-facts.sh — derive the drift-prone doc facts from their sources.
#
# The root docs restate a handful of facts that already have a machine-readable
# source: the crate version and its dependency versions, the `just check` gate
# table, and the linecop caps. Every one of them has rotted in some repo before
# — a README quoting a version two releases old, a CONTRIBUTING.md quoting a
# clippy invocation the Justfile had since grown flags for. This script is the
# fix at the root: read the source once, write the fact to www/generated/
# (gitignored, a build artifact like generate.sh's captures), and let a doc
# STATE the fact by reading the file, never by repeating it by hand.
#
# www/generate.sh calls this at its END, inside the flock it already holds: that
# script `rm -rf`s the whole generated/ dir on entry, and the wipe and the
# restore have to be ONE critical section or a concurrent reader sees the gap
# between them.
#
# Four artifacts, three sources:
#   - crate-map.json    <- `cargo metadata` + rust-toolchain.toml (see
#                          www/crate-map.jq)
#   - gates.json        <- `just --dump --dump-format json`, for the
#                          `[group("gate")]` tags and `check`'s own dependency
#                          list (see www/gates.jq)
#   - just.typ          <- the same `just --dump` (one shell-out, two
#                          artifacts): a Typst module, one function per public
#                          recipe, so a doc names a recipe through code
#   - linecop-caps.json <- .linecop.yaml, via yq
#
# Deliberately out of scope: the design invariants (fail-closed, the
# under/over-declaration asymmetry, existence-not-hashing for outputs). Those
# change with the design, not with a version bump — no source here proves or
# restates them.
set -eo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$SCRIPT_DIR/.." && pwd)"
OUT="$SCRIPT_DIR/generated"
mkdir -p "$OUT"

cd "$REPO"
mkdir -p target

# Same lock as www/generate.sh, same reason: `just check` runs its arms in
# parallel and more than one of them can be inside this directory at once.
# Re-exec under flock unless a caller already holds it — www/generate.sh exports
# the variable before calling this script, so the wipe-then-restore there stays
# ONE critical section instead of two.
if [ -z "$MMZ_GENERATED_LOCK" ]; then
  export MMZ_GENERATED_LOCK=1
  exec flock "$REPO/target/www-generated.lock" "$0" "$@"
fi

# Every artifact below is written through a temp file and moved into place, so a
# reader holding no lock (a Typst source mid-render) sees the previous complete
# file or the next one, never a truncated or absent one. `mv` within one
# filesystem is atomic; a redirect straight onto the destination is not, and it
# would also leave a zero-byte file behind when the producing jq program calls
# halt_error (the gate cross-check in www/gates.jq does).
#
# The parse check is what makes that guarantee hold under a FAILING producer,
# not just a slow one: a jq that dies mid-stream still closes the pipe, so the
# temp file exists and is partial. Installing a partial fact file is worse than
# installing none — the reader gets a plausible-looking JSON prefix instead of
# an error.
emit() {
  local name="$1" tmp="$OUT/.$1.tmp.$$"
  cat >"$tmp"
  if ! jq -e . "$tmp" >/dev/null 2>&1; then
    rm -f "$tmp"
    echo "generate-facts.sh: refusing to install malformed $name" >&2
    return 1
  fi
  mv -f "$tmp" "$OUT/$name"
}

# Same temp-file-plus-atomic-mv discipline for an artifact that is not JSON —
# `jq -e .` has nothing to parse. just.typ's producer builds the whole file as
# ONE jq string before printing anything, so a halted run writes nothing rather
# than a truncated prefix; refusing empty content is the raw equivalent of
# emit()'s parse check.
emit_raw() {
  local name="$1" tmp="$OUT/.$1.tmp.$$"
  cat >"$tmp"
  if [ ! -s "$tmp" ]; then
    rm -f "$tmp"
    echo "generate-facts.sh: refusing to install empty $name" >&2
    return 1
  fi
  mv -f "$tmp" "$OUT/$name"
}

# --- crate-map.json ---------------------------------------------------------
# The derivation lives in www/crate-map.jq (argued in place there); what stays
# here is the metadata call and the one fact that program cannot read for
# itself. `--no-deps` is deliberately NOT passed: the resolved dependency
# versions are the point.
CHANNEL="$(sed -n 's/^channel = "\(.*\)"/\1/p' rust-toolchain.toml | head -1)"
cargo metadata --format-version 1 \
  | jq -f "$SCRIPT_DIR/crate-map.jq" --arg channel "$CHANNEL" \
  | emit crate-map.json

# --- gates.json + just.typ ---------------------------------------------------
# One `just --dump` shell-out, two artifacts. gates.json is the gate table
# CONTRIBUTING.md renders; just.typ is the module every doc source calls a
# recipe through, so a rename fails the render naming the missing member instead
# of rotting silently in prose.
JUST_DUMP="$(just --dump --dump-format json)"
jq -f "$SCRIPT_DIR/gates.jq" <<<"$JUST_DUMP" | emit gates.json

# The header ships verbatim inside the artifact so its explanation travels with
# the file. What is NOT in the shipped header: the identifier collision check —
# two recipes mangling to the same `-`-joined identifier fail the jq program
# below with halt_error, naming both, rather than one silently shadowing the
# other.
JUST_TYP_HEADER="$(cat <<'HDR'
// www/generated/just.typ — generated by www/generate-facts.sh from `just
// --dump --dump-format json`; do not edit, edit the Justfile.
//
// A Typst MODULE, not a dict — Typst reads a dict field call as a method call
// and errors "type dictionary has no method". One function per PUBLIC recipe
// (private ones, `_name`, are excluded from the docs surface), module recipes
// included. Each renders the invocation as a raw span: `#just.check()` ->
// `just check`, `#just.docs-check()` -> `just docs check`; extra positional
// args join onto the command, so `#just.test("-- --nocapture")` ->
// `just test -- --nocapture`.
//
// Import as a module: `#import "/www/generated/just.typ" as just`.
// docs/src/lib.typ re-exports it so a docs/src source imports one place;
// www/content/*.typ (a different render path) imports this file directly.
//
// Identifier = the invocation path with `::` mangled to `-`.
#let mk(name) = (..args) => raw(
  "just " + name + args.pos().map(a => " " + a).join(""),
)
HDR
)"
jq -r --arg header "$JUST_TYP_HEADER" '
  def all_recipes:
    ((.recipes // {}) | to_entries | map(.value))
    + ((.modules // {}) | to_entries | map(.value) | map(all_recipes) | add // []);

  [all_recipes[] | select(.private == false)
    | {name: (.namepath | gsub("::"; " ")), id: (.namepath | gsub("::"; "-"))}] as $recipes
  | ($recipes | group_by(.id) | map(select(length > 1))) as $dups
  | if ($dups | length) > 0 then
      ("just.typ: two recipes mangle to the same identifier:\n"
        + ($dups | map("  " + .[0].id + ": " + (map(.name) | join(", ")))
           | join("\n")))
      | halt_error(1)
    else
      $header + "\n\n"
        + ($recipes | sort_by(.id)
            | map("#let " + .id + " = mk(\"" + .name + "\")")
            | join("\n"))
        + "\n"
    end
' <<<"$JUST_DUMP" | emit_raw just.typ

# --- linecop-caps.json --------------------------------------------------------
# The exact `limits`/`overrides` .linecop.yaml carries — the source
# CONTRIBUTING.md quotes caps from. yq is a jq-over-YAML wrapper, so this stays
# consistent with the rest of the docs tooling.
yq '{limits, overrides}' .linecop.yaml | emit linecop-caps.json

# Assert this script's own four artifacts by NAME, rather than counting whatever
# the output directory happens to hold. The glob this replaced also caught
# www/generate.sh's captures and generate-site-pages.sh's manifest, so its count
# depended on which of the three had run yet — a log line that changed between
# two identical runs. Naming them also turns "a producer silently wrote nothing"
# into a failure here instead of into a Typst read error three steps later.
#
# `emit` runs at the end of a pipeline and therefore in a SUBSHELL, so it cannot
# accumulate this list itself; the names live here, where they are also the
# script's declared contract.
FACTS=(crate-map.json gates.json just.typ linecop-caps.json)
for fact in "${FACTS[@]}"; do
  if [ ! -s "$OUT/$fact" ]; then
    echo "generate-facts.sh: $fact was not written" >&2
    exit 1
  fi
done
echo "generate-facts.sh: ${#FACTS[@]} fact files under $OUT"
