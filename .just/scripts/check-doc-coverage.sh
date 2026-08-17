#!/usr/bin/env bash
# .just/scripts/check-doc-coverage.sh — the body of the `check-doc-coverage`
# recipe. Every path below is repo-root-relative, because the recipe that
# invokes it runs at the repo root.
#
# The gate: every action `mmz --help` advertises must carry a hand-written
# www/utils/cli-notes.typ entry. The CLI reference page renders the help text
# verbatim AND the notes beside it, so an action with no note is an action the
# docs mention only inside a terminal dump — which is how `--tag` shipped
# documented in one place and invisible in the other.
#
# The action list is read out of the BINARY, never out of a list kept beside it:
# mmz's CLI is hand-rolled (a `USAGE` const in src/main.rs), so `--help` is the
# only machine-readable surface there is, and it is the one a user reads. Parsed
# from the `Usage:` block alone — the exit-code table below it is indented the
# same way but names no actions — and filtered to tokens that actually look like
# one (`--flag` or `<command>`), which is what drops the `exit 0 …` continuation
# line of the wrapped `--is-fresh` entry.
#
# The notes dict is read by EVALUATING it through a typst driver, never by
# grepping quoted strings out of the file: the keys are Typst code, and a regex
# over source has no way to know whether a string it found is a key, a value, or
# a word in a comment.
set -eo pipefail

mkdir -p target
tmp=$(mktemp -d -p target)
trap 'rm -rf "$tmp"' EXIT

cargo build -q --bin mmz
actions=$(./target/debug/mmz --help \
    | awk '/^Usage:/{f=1;next} /^$/{if(f)exit} f' \
    | awk '{print $2}' \
    | grep -E '^(--|<)' \
    | sort -u)

printf '#import "/www/utils/cli-notes.typ": cli-notes\n#metadata(cli-notes.keys()) <cli-notes-keys>\n' \
    >"$tmp/cli-keys.typ"
documented=$(typst query --root . "$tmp/cli-keys.typ" '<cli-notes-keys>' \
    --field value --one | jq -r '.[]' | sort -u)

missing=$(comm -23 <(echo "$actions") <(echo "$documented"))
if [ -n "$missing" ]; then
    echo "check-doc-coverage: undocumented CLI actions (no www/utils/cli-notes.typ entry):" >&2
    while IFS= read -r name; do echo "  - $name" >&2; done <<<"$missing"
    exit 1
fi

# The other direction: a note for an action the binary no longer advertises.
# Left unchecked it is a paragraph describing a flag that has been renamed or
# removed — worse than a missing note, because it reads as current.
stale=$(comm -13 <(echo "$actions") <(echo "$documented"))
if [ -n "$stale" ]; then
    echo "check-doc-coverage: cli-notes entries for actions mmz --help does not list:" >&2
    while IFS= read -r name; do echo "  - $name" >&2; done <<<"$stale"
    exit 1
fi

echo "check-doc-coverage: $(wc -l <<<"$actions") CLI actions, all documented"
