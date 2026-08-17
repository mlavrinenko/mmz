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
# output and still identical build-to-build. That leaves ONE post-processing
# sed, on absolute paths (the fixture's temp location) to a project-relative
# form; it is marked at its call site, and every other byte in every capture is
# stdout as written.
#
# `just docs::gen` invokes this; docs::build / docs::serve / docs::check run it
# first.
#
# Docs SSG gotchas (tola), learned building this generator:
#   - `tola serve` caches its file index at startup: a brand-new generated/ file
#     is not found by an already-running serve (pre-existing files read fine).
#     Restart serve after adding a capture here; `tola build` (the gate path)
#     always re-scans and is unaffected.
#   - `tola validate` statically flags an `image("<path>")` string literal as a
#     broken link when the target is not a copied asset. Anything built here is
#     under generated/, which is never copied to public/, so a build-time image
#     must be inlined via `image(read(path, encoding: none), …)` rather than by
#     path. (No such image today; the rule is here before the first one.)
set -eo pipefail

# The clock every capture below is taken against: 2026-08-17T17:00:00Z. mmz
# resolves it once per invocation and both stamps and renders from it, so a
# record's `ran_at` and the ages in a `--status` table are fixed by this line
# instead of by when the build ran. A run and the `--status` that follows it
# share the pin, which is why every AGE reads `0s ago`; a capture that wanted to
# show a genuinely aged record would run `MMZ_NOW=$((PINNED + 7200))` for that
# one command.
export MMZ_NOW=1786986000

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$SCRIPT_DIR/.." && pwd)"
FIX="$REPO/examples/demo"
OUT="$SCRIPT_DIR/generated"
MMZ="$REPO/target/release/mmz"

# The release build runs BEFORE the lock is taken, deliberately. Every reader of
# www/generated/ holds this lock for its whole run (docs/generate-md.sh does), so
# anything held inside it is time the rest of `just check`'s parallel arms spend
# blocked. A cold release build is minutes; the wipe-and-repopulate it precedes
# is seconds.
(cd "$REPO" && cargo build --release --bin mmz)

# Serialize every writer of www/generated/ (see www/generate-facts.sh for the
# others). `just check` runs its arms in parallel and more than one of them
# touches this directory: docs::check runs this script, while docs::md-check and
# check-doc-facts regenerate the facts inside it. The `rm -rf` below would then
# delete the facts out from under a concurrent reader — an intermittent "file not
# found (searched at www/generated/crate-map.json)" on work that is perfectly
# correct.
#
# Taken on a file descriptor rather than by re-exec'ing under `flock`, because a
# re-exec would rerun the build above. The lock is held until this script exits
# and fd 9 closes. MMZ_GENERATED_LOCK tells the nested generate-facts.sh and
# generate-site-pages.sh calls at the end of this file that the lock is already
# held, so neither deadlocks waiting for a lock this process owns — and it makes
# THIS script's own acquisition conditional too, since docs/generate-md.sh calls
# this script while already holding the lock across its own subsequent reads.
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

# Restore the derived facts inside the same critical section that deleted them.
# Splitting the wipe and the restore across two lock acquisitions would leave a
# window where www/generated/ exists with no facts in it, which is the exact
# state a concurrent reader must never see.
bash "$SCRIPT_DIR/generate-facts.sh"

# Then the page manifest — after generate-facts.sh, never before, and inside the
# SAME lock: querying a page's `<page-meta>` block is a full compile, which
# imports layout.typ -> site.typ -> crate-map.json (PKG_VERSION) plus every
# capture written above. Its own file because this one is already long enough.
bash "$SCRIPT_DIR/generate-site-pages.sh"

echo "generate.sh: $(find "$OUT" -type f | wc -l) files under $OUT"
