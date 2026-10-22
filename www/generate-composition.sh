#!/usr/bin/env bash
# www/generate-composition.sh — the composition page's captures.
#
# Runs the REAL `mmz` against a throwaway copy of examples/composed, a root
# manifest plus one imported fragment, and writes each capture VERBATIM under
# www/generated/composition-*. Its own fixture rather than more of
# examples/demo, because a composed manifest grows a SOURCE column in
# `--status`, and every other page's status capture is of a single file.
#
# Called by www/generate.sh inside its lock, after the wipe, never on its own:
# it inherits the `$MMZ_NOW` pin from there, so the AGE column is the same on
# every build. The caller makes the temp directory and passes it in, so the
# caller's check-capture-paths.sh run knows the exact path to refuse.
#
# Usage: generate-composition.sh <mmz> <fixture> <out> <tmpdir>
set -eo pipefail

MMZ="${1:?usage: generate-composition.sh <mmz> <fixture> <out> <tmpdir>}"
FIX="${2:?}"
OUT="${3:?}"
COPY="${4:?}/composed"

# What a fragment may say, as a schema. The composition page derives its list
# of root-only policy keys from the gap between this and config-schema.json,
# the same gap schema.rs asserts is exactly compose::POLICY_KEYS.
"$MMZ" --schema=fragment >"$OUT/fragment-schema.json"

cp -r "$FIX" "$COPY"
cd "$COPY"

# A command that must FAIL -> OUT/composition-<slug>.txt, stdout and stderr
# folded since the error goes to stderr. A command that unexpectedly succeeds
# fails the build: the page would otherwise show a dump under an error heading.
gen_fail() {
  local slug="$1"; shift
  if "$MMZ" "$@" >"$OUT/composition-$slug.txt" 2>&1; then
    echo "generate-composition.sh: \`mmz $*\` was meant to fail for $slug" >&2
    exit 1
  fi
}

# Both rules recorded, so --status has a verdict for each.
"$MMZ" ./bin/build.sh >/dev/null 2>&1
"$MMZ" ./bin/lint.sh >/dev/null 2>&1

"$MMZ" --dump-config >"$OUT/composition-dump.txt" 2>&1
"$MMZ" --status >"$OUT/composition-status.txt" 2>&1

# The same scope declared in the root and in the fragment: an error, never
# last-wins. Undone afterwards so the next capture breaks one thing only.
cp .mmz/conf.d/lint.yaml lint.yaml.orig
sed -i 's#^scopes:$#&\n  rust: ["src/**/*.rs"]#' .mmz/conf.d/lint.yaml
gen_fail duplicate-scope --dump-config
mv lint.yaml.orig .mmz/conf.d/lint.yaml

# The import path everyone writes first, root-relative instead of relative to
# the importing file. The error names the path mmz actually resolved.
sed -i 's#^  - conf.d/$#  - .mmz/conf.d/#' .mmz/config.yaml
gen_fail wrong-import --dump-config
