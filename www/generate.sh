#!/usr/bin/env bash
# www/generate.sh — capture live `mmz` output for the docs site.
#
# Runs the REAL `mmz` binary against the examples/demo fixture and writes each
# capture VERBATIM under www/generated/. The docs pages inject these via
# `terminal(read("../generated/<slug>.txt"))`, so a transcript can never drift
# from the binary — regenerated on every build.
#
# Why a fixture rather than this repo: the interesting captures are MUTATING
# ones (a run that writes a record, an edit that busts it, a `--prune` that
# sweeps it), and a docs build must never touch the tree it documents. Every
# mutating command below runs against a throwaway copy under $TMPDIR, so the
# committed fixture stays pristine and the site is reproducible from a clean
# checkout.
#
# Determinism: everything a capture carries that is not a function of the repo
# is pinned rather than corrected afterwards. The clock comes from `$MMZ_NOW`
# below, so a record's `ran_at` and `--status`'s ages are the binary's own
# output and still identical build-to-build. The fixture's temp location cannot
# be pinned that way, so a capture naming it is marked `rel` at the call site and
# every other byte is stdout as written — a convention no comment can enforce, so
# check-capture-paths.sh at the end fails the build on a capture that still leaks.
#
# `just docs::gen` invokes this; docs::build / docs::serve / docs::check run it
# first.
#
# Two tola behaviours bite whoever edits this file; "Working within tola" in
# docs/src/contributing/generated-docs.typ has both.
set -eo pipefail

# The clock every capture is taken against: 2026-08-17T17:00:00Z. mmz resolves it
# once per invocation, so a record's `ran_at` and a `--status` table's ages come
# from this line, not from when the build ran. A run and the `--status` after it
# share the pin, hence every AGE reads `0s ago`; an aged record would run
# `MMZ_NOW=$((PINNED + 7200))` for that one command.
export MMZ_NOW=1786986000
# Pinned off too: a forced `just check` exports MMZ_DISABLE=1, and the captures
# must show mmz skipping, not a second cold run.
export MMZ_DISABLE=0

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$SCRIPT_DIR/.." && pwd)"
FIX="$REPO/examples/demo"
OUT="$SCRIPT_DIR/generated"
MMZ="$REPO/target/release/mmz"

# The release build runs BEFORE the lock, deliberately: every reader of
# www/generated/ (docs/generate-md.sh too) holds the lock for its whole run, so
# time spent inside it blocks `just check`'s parallel arms. A cold release build
# is minutes; the wipe-and-repopulate it precedes is seconds.
(cd "$REPO" && cargo build -q --release --bin mmz)

# Serialize every writer of www/generated/ (see www/generate-facts.sh for the
# others). `just check` runs its arms in parallel: docs::check runs this script
# while docs::md-check and check-doc-facts regenerate the facts inside it, so the
# `rm -rf` below would delete facts from under a concurrent reader, failing
# correct work with an intermittent "file not found (…crate-map.json)".
#
# Taken on a file descriptor, not by re-exec'ing under `flock`, which would rerun
# the build above; fd 9 holds it until exit. MMZ_GENERATED_LOCK tells the nested
# generate-*.sh calls the lock is already held so none deadlocks against this
# process, and makes THIS acquisition conditional too, since
# docs/generate-md.sh calls this script while holding the lock itself.
mkdir -p "$REPO/target"
if [ -z "$MMZ_GENERATED_LOCK" ]; then
  exec 9>"$REPO/target/www-generated.lock"
  flock 9
  export MMZ_GENERATED_LOCK=1
fi

rm -rf "$OUT"
mkdir -p "$OUT"

# --- project-independent captures (no fixture needed) -----------------------
"$MMZ" --version >"$OUT/version.txt"
# The full help text. The CLI reference renders this verbatim in a terminal
# panel AND parses its usage block for the action list that check-doc-coverage
# gates, so the one string is both the shown text and the checked surface.
"$MMZ" --help >"$OUT/help.txt"
# The config JSON Schema — the exact bytes `mmz --schema` prints. The manifest
# reference renders its properties and descriptions into a table, and
# check-doc-facts asserts every property is surfaced, so a newly added manifest
# key cannot silently vanish from the docs.
"$MMZ" --schema >"$OUT/config-schema.json"
# The --status=json schema, same discipline, for the CLI reference's JSON section.
"$MMZ" --status=json-schema >"$OUT/status-schema.json"

# --- `mmz --init` in an empty project ---------------------------------------
# What a project actually gets on day one: the command's own output, and the two
# files it writes, verbatim. The "wrote <abs>" path is normalized to
# project-relative so the capture is machine-independent.
INIT_DIR="$(mktemp -d)"
(cd "$INIT_DIR" && "$MMZ" --init) | sed "s#$INIT_DIR/##g" >"$OUT/init.txt"
cp "$INIT_DIR/.mmz/config.yaml" "$OUT/init-config.yaml"
cp "$INIT_DIR/.mmz/.gitignore" "$OUT/init-gitignore.txt"
rm -rf "$INIT_DIR"

# --- the fixture, verbatim ---------------------------------------------------
# Copied under generated/ so the pages can `read()` them: Typst refuses a read
# that escapes the build root, and the site builds under `--root www`.
cp "$FIX/.mmz/config.yaml" "$OUT/demo-config.yaml"
cp "$FIX/bin/validate.sh" "$OUT/demo-validate.sh"
cp "$FIX/data/orders.json" "$OUT/demo-orders.json"

# --- mutating commands against a throwaway copy ------------------------------
# Never the fixture itself: these write cache records and build artifacts.
COPY="$(mktemp -d)/demo"
cp -r "$FIX" "$COPY"
cd "$COPY"

# A run command against the copy in place -> OUT/<slug>.txt. stderr is folded
# into the capture because the cache-hit note goes there and it is half the
# story: the reader needs to see the skip announced, not just its absence.
gen_run() {
  local slug="$1"; shift
  "$MMZ" "$@" >"$OUT/$slug.txt" 2>&1
}

# Rewrite the fixture copy's absolute path out of a capture whose command names
# the manifest by path: the copy lives under $TMPDIR, so those bytes differ on
# every build. Opt-in per capture, the ONLY post-processing any capture gets, and
# check-capture-paths.sh below refuses one that needed it and went without.
rel() { sed -i "s#$COPY/#./#g" "$OUT/$1.txt"; }

# The headline pair: the same command twice. The first runs and streams the
# script's own output; the second prints the on_hit note and exits 0 having run
# nothing.
gen_run run-cold ./bin/validate.sh
gen_run run-warm ./bin/validate.sh
# The producer rule, so a record with declared outputs exists for --status below.
gen_run run-report ./bin/report.sh

gen_run status --status
gen_run status-tag --status --tag report
gen_run status-json --status=json
rel status-json

# A tag no rule carries — `gate` with one letter changed. The gate refuses it
# (exit 7, hence the `|| true`) instead of passing over an empty selection,
# while the report answers the same question and exits 0. Both captures are of
# the SAME typo, because the pair is the point: one asserts, one describes.
gen_run is-fresh-empty-tag --is-fresh --tag gats || true
gen_run status-empty-tag --status --tag gats
rel status-empty-tag

# A stale gate: edit an input, then ask. `--is-fresh` exits 1 here, which is the
# whole point, so the `|| true` keeps `set -e` from treating the documented
# failure as a script error.
printf '\n' >>data/orders.json
gen_run is-fresh-stale --is-fresh || true
# Re-record so the following captures start from a fresh state again.
"$MMZ" ./bin/validate.sh >/dev/null 2>&1
"$MMZ" ./bin/report.sh >/dev/null 2>&1

# A voided record: the inputs are untouched, but the artifact the run promised
# is gone. This is the case `outputs:` exists for, and the reason it reports
# `missing-output` rather than `stale`.
rm -rf out
gen_run status-missing-output --status
gen_run is-fresh-missing-output --is-fresh || true
"$MMZ" ./bin/report.sh >/dev/null 2>&1

# One cache record, verbatim — what a record actually claims, byte for byte.
# `ran_at` used to be the one field that differed every build and was rewritten
# here; it now comes from the `$MMZ_NOW` pin above, so the file is copied
# untouched.
RECORD="$(find .mmz/cache -name 'bin-report-sh-*.yaml' | head -1)"
cp "$RECORD" "$OUT/record.yaml"

# --prune, demonstrated the only honest way: rename a rule so its record is
# genuinely orphaned, then sweep it.
sed -i 's#^  - name: ./bin/report.sh#  - name: ./bin/report.sh --full#' .mmz/config.yaml
gen_run prune --prune

cd "$REPO"
rm -rf "$(dirname "$COPY")"

# The composition page's captures, against their own composed fixture.
COMPOSED_TMP="$(mktemp -d)"
bash "$SCRIPT_DIR/generate-composition.sh" "$MMZ" "$REPO/examples/composed" "$OUT" "$COMPOSED_TMP"
rm -rf "$COMPOSED_TMP"

# Restore the derived facts inside the critical section that deleted them: split
# across two lock acquisitions, a reader could see www/generated/ with no facts.
bash "$SCRIPT_DIR/generate-facts.sh"

# Then the page manifest, after generate-facts.sh and inside the SAME lock: a
# page's `<page-meta>` query is a full compile, which reads crate-map.json
# (PKG_VERSION) and every capture written above. Its own file to keep this short.
bash "$SCRIPT_DIR/generate-site-pages.sh"

# Last, over everything the three scripts wrote: no capture may name a directory
# this build invented. The two mktemp roots are passed down by value — gone from
# disk by now — so the check is exact about THIS run, not a guess at mktemp names.
bash "$REPO/.just/scripts/check-capture-paths.sh" "$OUT" "$(dirname "$COPY")" "$INIT_DIR" \
  "$COMPOSED_TMP"

echo "generate.sh: $(find "$OUT" -type f | wc -l) files under $OUT"
