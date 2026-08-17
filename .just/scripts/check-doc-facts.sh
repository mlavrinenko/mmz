#!/usr/bin/env bash
# .just/scripts/check-doc-facts.sh — the body of the `check-doc-facts` recipe.
# Every path below is repo-root-relative, because the recipe that invokes it
# runs at the repo root.
#
# Doc-facts gate: the same set-difference idiom as check-doc-coverage.sh, over
# the facts the generators derive rather than over a hand-maintained list. Three
# arms:
#
#   - every property in the config JSON Schema needs a www/utils/config-notes.typ
#     entry, so a manifest key cannot ship with a schema description and no
#     prose;
#   - every `[group("gate")]` recipe needs a www/utils/gate-notes.typ entry, so a
#     gate cannot join `just check` without CONTRIBUTING.md saying what it is
#     for;
#   - every www/content/*.typ page needs a route matching its filename AND a NAV
#     entry in www/utils/site.typ, so a page cannot be unreachable from the
#     sidebar and a NAV entry cannot 404.
#
# Each arm reads its Typst dict's keys by EVALUATING the module through a
# target/-rooted driver, never by grepping quoted strings — a regex over source
# cannot tell a key from a word in a comment. The driver lives under target/
# because `typst query --root .` refuses a source outside the project root.
#
# It regenerates its own inputs (bash www/generate.sh, which ends by calling
# generate-facts.sh and then generate-site-pages.sh) rather than trusting
# whatever www/generated/ already holds, so this gate never depends on
# `just docs::gen` having run first. Inside `just check` that is nearly free:
# docs::check already ran generate.sh and both share the same lock, so the
# captures are already there.
set -eo pipefail

# Hold the www/generated/ lock across the regenerate AND the typst queries
# below, not just the regenerate: www/utils/gate-notes.typ reads the generated
# `just` module, so a driver querying it is a reader of that directory, and the
# concurrent docs::check arm `rm -rf`s it. Same fd-held lock and same
# MMZ_GENERATED_LOCK handshake as www/generate.sh — and the reason that script's
# OWN acquisition is conditional on the handshake rather than unconditional:
# without it, generate.sh taking a second lock on the file this fd already holds
# would deadlock against this process.
mkdir -p target
exec 9>target/www-generated.lock
flock 9
export MMZ_GENERATED_LOCK=1
bash www/generate.sh

tmp=$(mktemp -d -p target)
trap 'rm -rf "$tmp"' EXIT

# --- manifest keys -----------------------------------------------------------
# Namespaced exactly as the reference renders them, so a key's note is findable
# by the name a reader sees: top-level keys bare, a command rule's keys under
# `commands[].`, a probe's under `probes[].`, an object-form scope's under
# `scopes[].`. Reading all four levels is what stops `allow_empty` or a scope's
# `gitignore` override — both real keys a user writes — from counting as
# documented merely because their parent is.
keys=$(jq -r '
    (.properties | keys[])
  , (.properties.commands.items.properties | keys[] | "commands[]." + .)
  , (.properties.probes.additionalProperties.properties | keys[] | "probes[]." + .)
  , (.properties.scopes.additionalProperties.oneOf[]
      | select(.type == "object") | .properties | keys[] | "scopes[]." + .)
' www/generated/config-schema.json | sort -u)

printf '#import "/www/utils/config-notes.typ": config-notes\n#metadata(config-notes.keys()) <config-notes-keys>\n' \
    >"$tmp/config-keys.typ"
documented_keys=$(typst query --root . "$tmp/config-keys.typ" '<config-notes-keys>' \
    --field value --one | jq -r '.[]' | sort -u)

missing_keys=$(comm -23 <(echo "$keys") <(echo "$documented_keys"))
if [ -n "$missing_keys" ]; then
    echo "check-doc-facts: undocumented manifest keys (no www/utils/config-notes.typ entry):" >&2
    while IFS= read -r name; do echo "  - $name" >&2; done <<<"$missing_keys"
    exit 1
fi
stale_keys=$(comm -13 <(echo "$keys") <(echo "$documented_keys"))
if [ -n "$stale_keys" ]; then
    echo "check-doc-facts: config-notes entries for keys the schema does not declare:" >&2
    while IFS= read -r name; do echo "  - $name" >&2; done <<<"$stale_keys"
    exit 1
fi

# --- exit codes --------------------------------------------------------------
# Parsed out of the binary's own help text, the same way www/utils/cli-ref.typ
# parses it to render the table: inside the `Exit codes:` block, a code is any
# 1-3 digit run followed by the column gap. That shape survives the block's
# two-column layout, and it matches no digits in the surrounding prose (verified
# against every line of the current help text).
codes=$(awk '/^Exit codes:/{f=1;next} /^$/{if(f)exit} f' www/generated/help.txt \
    | grep -oP '\b\d{1,3}(?=\s\s)' | sort -un)
printf '#import "/www/utils/exit-code-notes.typ": exit-code-notes\n#metadata(exit-code-notes.keys()) <exit-code-notes-keys>\n' \
    >"$tmp/exit-code-keys.typ"
documented_codes=$(typst query --root . "$tmp/exit-code-keys.typ" '<exit-code-notes-keys>' \
    --field value --one | jq -r '.[]' | sort -un)
missing_codes=$(comm -23 <(echo "$codes") <(echo "$documented_codes"))
if [ -n "$missing_codes" ]; then
    echo "check-doc-facts: undocumented exit codes (no www/utils/exit-code-notes.typ entry):" >&2
    while IFS= read -r code; do echo "  - $code" >&2; done <<<"$missing_codes"
    exit 1
fi
stale_codes=$(comm -13 <(echo "$codes") <(echo "$documented_codes"))
if [ -n "$stale_codes" ]; then
    echo "check-doc-facts: exit-code-notes entries for codes mmz --help does not document:" >&2
    while IFS= read -r code; do echo "  - $code" >&2; done <<<"$stale_codes"
    exit 1
fi

# --- gates -------------------------------------------------------------------
gates=$(jq -r '.gates[].name' www/generated/gates.json | sort -u)
printf '#import "/www/utils/gate-notes.typ": gate-notes\n#metadata(gate-notes.keys()) <gate-notes-keys>\n' \
    >"$tmp/gate-keys.typ"
documented_gates=$(typst query --root . "$tmp/gate-keys.typ" '<gate-notes-keys>' \
    --field value --one | jq -r '.[]' | sort -u)
missing_gates=$(comm -23 <(echo "$gates") <(echo "$documented_gates"))
if [ -n "$missing_gates" ]; then
    echo "check-doc-facts: undocumented gates (no www/utils/gate-notes.typ entry):" >&2
    while IFS= read -r name; do echo "  - $name" >&2; done <<<"$missing_gates"
    exit 1
fi

# --- pages, part one: filename vs. the route each PAGE ITSELF declares --------
# site-pages.json is keyed by the route a page's own `<page-meta>` block
# declares, not by its filename, so this diff is "file vs. the page's own claim
# about its route". generate-site-pages.sh already refuses two pages declaring
# the SAME route; this catches one declaring the WRONG one — a valid route that
# simply is not this file's. The file->route rule is tola's own layout
# (index.typ serves "/", every other <stem>.typ serves "/<stem>/"), applied here
# rather than read from anywhere, because the whole point is to catch the file
# and its declared route disagreeing.
routes=$(jq -r '.pages | keys_unsorted[]' www/generated/site-pages.json | sort)
page_routes=$(for f in www/content/*.typ; do
    stem=$(basename "$f" .typ)
    if [ "$stem" = "index" ]; then echo "/"; else echo "/$stem/"; fi
done | sort)
unlisted=$(comm -23 <(echo "$page_routes") <(echo "$routes"))
if [ -n "$unlisted" ]; then
    echo "check-doc-facts: content pages whose declared route matches no filename-derived route:" >&2
    while IFS= read -r route; do echo "  - $route" >&2; done <<<"$unlisted"
    exit 1
fi
orphaned=$(comm -13 <(echo "$page_routes") <(echo "$routes"))
if [ -n "$orphaned" ]; then
    echo "check-doc-facts: filename-derived routes with no page declaring them:" >&2
    while IFS= read -r route; do echo "  - $route" >&2; done <<<"$orphaned"
    exit 1
fi

# --- pages, part two: the manifest vs. www/utils/site.typ's NAV ---------------
# NAV carries only group titles and reading order now, so the two CAN disagree:
# a page missing from NAV is invisible in the sidebar, and a NAV entry naming no
# real page's route 404s the moment it is clicked.
printf '#import "/www/utils/site.typ": NAV\n#metadata(NAV.map(g => g.items).flatten()) <nav-routes>\n' \
    >"$tmp/nav-routes.typ"
nav_routes=$(typst query --root . "$tmp/nav-routes.typ" '<nav-routes>' \
    --field value --one | jq -r '.[]' | sort)
missing_nav=$(comm -23 <(echo "$routes") <(echo "$nav_routes"))
if [ -n "$missing_nav" ]; then
    echo "check-doc-facts: pages with no www/utils/site.typ NAV entry (unreachable in the sidebar):" >&2
    while IFS= read -r route; do echo "  - $route" >&2; done <<<"$missing_nav"
    exit 1
fi
dead_nav=$(comm -13 <(echo "$routes") <(echo "$nav_routes"))
if [ -n "$dead_nav" ]; then
    echo "check-doc-facts: NAV entries naming no page (a 404 in the sidebar):" >&2
    while IFS= read -r route; do echo "  - $route" >&2; done <<<"$dead_nav"
    exit 1
fi

echo "check-doc-facts: $(wc -l <<<"$keys") manifest keys, $(wc -l <<<"$codes") exit codes, $(wc -l <<<"$gates") gates, $(wc -l <<<"$routes") pages, all documented"
