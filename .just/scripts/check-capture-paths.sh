#!/usr/bin/env bash
# .just/scripts/check-capture-paths.sh — fail if a generated docs capture names
# a build-time temporary directory.
#
# www/generate.sh runs the REAL binary against a throwaway copy of
# examples/demo under $TMPDIR, so any capture of a command that prints a path
# carries THAT build's temp directory verbatim — a byte sequence no other
# checkout reproduces and no reader can act on. The site is meant to be
# reproducible from a clean checkout; `/tmp/tmp.BIg78u4JqB/demo/.mmz/config.yaml`
# is neither reproducible nor readable.
#
# Nothing else catches this. docs::check builds the site and validates its
# links; docs::md-check diffs generated Markdown against its Typst source.
# Neither reads a capture's bytes, because a capture IS stdout and stdout is the
# thing being trusted. This script is the one reader that distrusts it.
#
# Usage: check-capture-paths.sh <dir> [literal-path...]
#
#   <dir>           directory whose files are scanned (www/generated)
#   [literal-path]  a temp directory THIS build actually created, passed down by
#                   the caller
set -eo pipefail

dir="${1:?usage: check-capture-paths.sh <dir> [literal-path...]}"
shift

# Two passes, because neither alone is honest.
#
# The literals are the caller's own mktemp directories. That pass is exact: it
# cannot miss a leak, and a $TMPDIR pointing somewhere this script would never
# think to look cannot defeat it.
#
# The shape pass knows only what an mktemp name looks like, so it is a guess —
# but it is the guess that covers the files the caller did not write and
# therefore could not pass down (the derived facts, the page manifest), plus
# anything left behind by a run under a different temp directory.
literals=()
for p in "$@"; do literals+=(-e "$p"); done

shapes=()
for base in "${TMPDIR:-/tmp}" /tmp; do
    base="${base%/}"
    # `.` must not sit directly after the opening `[`: `[.` opens a collating
    # symbol and the class then never closes.
    esc="$(printf '%s' "$base" | sed 's#[][(){}|.*^$+?\\]#\\&#g')"
    shapes+=(-e "$esc/tmp\.[A-Za-z0-9]+")
done

hits=""
if [ "${#literals[@]}" -gt 0 ]; then
    hits="$(grep -rnF "${literals[@]}" -- "$dir" || true)"
fi
hits="$(printf '%s\n%s\n' "$hits" "$(grep -rnE "${shapes[@]}" -- "$dir" || true)" | grep -v '^$' | sort -u || true)"

if [ -n "$hits" ]; then
    echo "check-capture-paths: a capture under $dir names a build-time temp directory:" >&2
    printf '%s\n' "$hits" | sed 's/^/  /' >&2
    cat >&2 <<'EOF'

A capture is the binary's stdout written verbatim, so a command that prints an
absolute path bakes this build's $TMPDIR into the site. Normalize that capture
where it is taken in www/generate.sh — `rel <slug>` rewrites the fixture copy's
path to a project-relative one — rather than shipping the leak in the bytes.
EOF
    exit 1
fi

echo "check-capture-paths: $(find "$dir" -type f | wc -l) generated file(s) free of temp paths"
