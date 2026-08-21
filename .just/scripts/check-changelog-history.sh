#!/usr/bin/env bash
# .just/scripts/check-changelog-history.sh — the body of the
# `check-changelog-history` recipe, and of `just changelog-waive`.
#
# History gate: for every `v*` tag reachable from HEAD, the `## [x.y.z]` section
# in the working CHANGELOG.md must be byte-identical to the one the tag shipped
# (`git show <tag>:CHANGELOG.md`). A changelog is the one document whose past is
# immutable — a section for a released tag is a historical record, so an edit to
# it is either a mistake or a deliberate rewrite, and only the second has a
# reason to exist.
#
# It was the first. `4c57277` inserted a `### Fixed` block under `## [Unreleased]`
# and wrote over the `## [0.7.0] - 2026-08-17` heading doing it: every line the
# tag shipped silently became part of `Unreleased`, and four later entries were
# appended into what a reader would take for the 0.7.0 list. Ten gates ran green
# over it for four days and thirteen commits, because no gate read this file —
# `outdatty` couples Cargo.toml to it as a *dependent*, which asks that a human
# re-confirmed, not that the content survived, and `linecop` only counts lines.
# See tasks/mmz-a-released-changelog-section-can-vanish-unnoticed.typ.
#
# The deliberate rewrite is what CHANGELOG.waivers is for: a recorded hash of
# the REWRITTEN section, in the `outdatty` spirit — a watermark meaning a human
# looked, not that a tool agreed. `just changelog-waive <version> <reason>`
# records one. It cannot be recorded blind: the hash covers the exact bytes now
# in the file, so a later corruption of a waived section fails the gate again.
#
# Comparison is over the section's own lines with trailing blank lines dropped
# (the `$(...)` below does that), so a section that sat at the end of the file
# when it was tagged and has a successor now compares equal — which is every
# section's history, not an edge case.
#
# Usage:
#   check-changelog-history.sh                        check every reachable tag
#   check-changelog-history.sh --waive <ver> <reason> record a waiver for <ver>
set -eo pipefail

CHANGELOG=CHANGELOG.md
WAIVERS=CHANGELOG.waivers
me=check-changelog-history

fail() {
    echo "$me: $*" >&2
    exit 1
}

# The `## [<version>]` section, read from stdin. Prefix comparison rather than a
# regex, so the version's dots need no escaping — and the closing bracket makes
# the prefix exact, so `## [0.7.0]` cannot match `## [0.7.0-rc1]`.
extract() {
    awk -v pfx="## [$1]" '
        index($0, "## ") == 1 { if (found) exit; found = (index($0, pfx) == 1) }
        found { print }
    '
}

# The section as of a tag. A tag with no CHANGELOG.md at all yields nothing and
# is reported as skipped rather than as a failure — there is no shipped record
# to protect.
section_at_tag() {
    git show "$1:$CHANGELOG" 2>/dev/null | extract "$2" || true
}

digest() {
    printf '%s\n' "$1" | sha256sum | cut -d' ' -f1
}

# The hash recorded for a version, or nothing. Waiver lines are
# `<version> <sha256> # <reason>`; `#` lines and blanks fall out because a
# comment's first field is never a bare version.
recorded_waiver() {
    [ -f "$WAIVERS" ] || return 0
    awk -v v="$1" '$1 == v { print $2; exit }' "$WAIVERS"
}

waiver_versions() {
    [ -f "$WAIVERS" ] || return 0
    awk 'NF && $1 !~ /^#/ { print $1 }' "$WAIVERS"
}

git rev-parse --is-inside-work-tree >/dev/null 2>&1 ||
    fail "not a git work tree; this gate reads released sections out of git"
[ -f "$CHANGELOG" ] || fail "no $CHANGELOG in $(pwd)"

# A shallow clone has the tag refs but not the tagged trees, so `--merged HEAD`
# resolves nothing and the gate would pass over an empty set — the same silent
# green this file exists to stop. Fail instead, naming the fix: the CI checkout
# needs `fetch-depth: 0`.
[ "$(git rev-parse --is-shallow-repository)" = "false" ] ||
    fail "shallow clone: released sections are unreadable here. Check out with \`fetch-depth: 0\`."

if [ "${1:-}" = "--waive" ]; then
    version=${2:?usage: check-changelog-history.sh --waive <version> <reason>}
    shift 2
    reason=$*
    [ -n "$reason" ] || fail "a waiver must say why: --waive <version> <reason>"
    git rev-parse -q --verify "refs/tags/v$version" >/dev/null ||
        fail "no tag v$version — only a RELEASED section can be waived"

    tagged=$(section_at_tag "v$version" "$version")
    work=$(extract "$version" <"$CHANGELOG")
    [ "$work" != "$tagged" ] ||
        fail "the $version section already matches v$version; nothing to waive"

    hash=$(digest "$work")
    if [ ! -f "$WAIVERS" ]; then
        cat >"$WAIVERS" <<'EOF'
# CHANGELOG.waivers — deliberate rewrites of released changelog sections.
#
# Written by `just changelog-waive <version> <reason>`, read by
# `just check-changelog-history`. Each line is `<version> <sha256> # <reason>`,
# where the hash covers the section's CURRENT bytes: it records that a human
# looked at this rewrite, so a later corruption of the same section is a
# mismatch again rather than a hole the first waiver left open.
EOF
    fi
    tmp=$(mktemp)
    awk -v v="$version" '!(NF && $1 == v)' "$WAIVERS" >"$tmp"
    printf '%s %s # %s\n' "$version" "$hash" "$reason" >>"$tmp"
    mv "$tmp" "$WAIVERS"
    echo "$me: waived $version against its current bytes ($hash)"
    echo "$me: commit $WAIVERS with the change it covers"
    exit 0
fi

[ -z "${1:-}" ] || fail "unknown argument: $1"

ok=0
waived=0
skipped=0
failed=0
waived_versions=""

while IFS= read -r tag; do
    [ -n "$tag" ] || continue
    version=${tag#v}
    tagged=$(section_at_tag "$tag" "$version")
    if [ -z "$tagged" ]; then
        echo "$me: $tag shipped no [$version] section — nothing to compare" >&2
        skipped=$((skipped + 1))
        continue
    fi

    work=$(extract "$version" <"$CHANGELOG")
    if [ "$work" = "$tagged" ]; then
        ok=$((ok + 1))
        continue
    fi

    recorded=$(recorded_waiver "$version")
    if [ -n "$recorded" ] && [ "$recorded" = "$(digest "$work")" ]; then
        waived=$((waived + 1))
        waived_versions="$waived_versions $version"
        continue
    fi

    failed=1
    if [ -z "$work" ]; then
        echo "$me: the [$version] section $tag shipped is GONE from $CHANGELOG" >&2
    elif [ -n "$recorded" ]; then
        echo "$me: [$version] differs from $tag, and its $WAIVERS entry was recorded against other bytes" >&2
        waived_versions="$waived_versions $version"
    else
        echo "$me: [$version] differs from what $tag shipped" >&2
    fi
    diff -u --label "$tag:$CHANGELOG" --label "$CHANGELOG (working tree)" \
        <(printf '%s\n' "$tagged") <(printf '%s\n' "$work") >&2 || true
    echo "  restore it with: git show $tag:$CHANGELOG" >&2
    echo "  or, if the rewrite is deliberate: just changelog-waive $version '<why>'" >&2
done <<<"$(git tag --list 'v*' --merged HEAD --sort=version:refname)"

# A waiver naming a section that now matches its tag again is a waiver nobody
# reads, and the next edit to that section would land under it unreviewed. The
# same set-difference the doc gates make: an entry with nothing to cover fails.
for version in $(waiver_versions); do
    case " $waived_versions " in
    *" $version "*) continue ;;
    esac
    echo "$me: $WAIVERS records $version, which needs no waiver — the section matches its tag, or no reachable tag ships it" >&2
    failed=1
done

[ "$failed" -eq 0 ] || exit 1
echo "$me: $ok released section(s) match their tags ($waived waived, $skipped without one)"
