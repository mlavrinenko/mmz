#!/usr/bin/env bash
# www/generate-site-pages.sh — derive www/generated/site-pages.json from every
# www/content/*.typ page's own `<page-meta>` block.
#
# Each page carries
# `#metadata((route, label, title, summary, group, order, home)) <page-meta>`
# and spreads that same dict into its show rule, so the page is the single
# source of its own route, label, title, summary, group and order. This script
# reads all of them back into one manifest: it validates the group against
# www/utils/site.typ's GROUPS, refuses a duplicate order within a group, and
# writes the pages in that order. The sidebar renders from it, docs/src/lib.typ's
# `www-link` resolves an absolute docs URL against it, and the README's page
# list reads its groups and summaries. A page renamed, re-summarised or moved
# therefore lands everywhere with no second edit.
#
# Called by www/generate.sh, AFTER its generate-facts.sh call, inside the lock
# that script already holds. The ordering is not negotiable: querying a page's
# `<page-meta>` block is a FULL COMPILE, which imports layout.typ -> site.typ ->
# crate-map.json (PKG_VERSION) and every capture generate.sh itself writes.
# Without those, a page reading a capture at the top level dies before its
# metadata is ever reached.
#
# Reentrant locking, the same handshake generate-facts.sh uses: a caller holding
# target/www-generated.lock exports MMZ_GENERATED_LOCK=1 so this script skips its
# own acquisition instead of deadlocking against a lock its own process tree
# already owns. Run standalone, it takes the lock itself.
set -eo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$SCRIPT_DIR/.." && pwd)"
OUT="$SCRIPT_DIR/generated"
mkdir -p "$OUT" "$REPO/target"

if [ -z "$MMZ_GENERATED_LOCK" ]; then
  exec 9>"$REPO/target/www-generated.lock"
  flock 9
  export MMZ_GENERATED_LOCK=1
fi

# Same temp-file-plus-atomic-mv-plus-parse-check discipline as
# generate-facts.sh's own emit() (argued in place there).
emit() {
  local name="$1" dest="$OUT/.$1.tmp.$$"
  cat >"$dest"
  if ! jq -e . "$dest" >/dev/null 2>&1; then
    rm -f "$dest"
    echo "generate-site-pages.sh: refusing to install malformed $name" >&2
    return 1
  fi
  mv -f "$dest" "$OUT/$name"
}

# Bootstrap pass. Compiling ANY page imports layout.typ -> site.typ, and
# site.typ's PAGES reads this very file, so it has to exist — even empty —
# before the query below can compile the first page. Only seeded when absent: a
# real build already has a real manifest on disk, and re-seeding it empty here
# would make every page compile against a momentarily-empty PAGES for no reason.
if [ ! -f "$OUT/site-pages.json" ]; then
  jq -n '{pages: {}}' | emit site-pages.json
fi

workdir="$(mktemp -d -p "$REPO/target")"
trap 'rm -rf "$workdir"' EXIT

# One `<page-meta>` query per page. `--one` makes a page with no block (or more
# than one) a hard failure; typst's own error names the file, so it is echoed
# straight through rather than swallowed. `--target html --features html` is
# required because compiling a page walks layout.typ, which calls `html.elem`.
pages_json="{}"
routes_json="[]"
for page in "$REPO"/www/content/*.typ; do
  stem="$(basename "$page" .typ)"
  if ! meta="$(typst query --root "$REPO/www" "$page" '<page-meta>' \
      --field value --one --target html --features html 2>"$workdir/err")"; then
    echo "generate-site-pages.sh: $page has no <page-meta> block:" >&2
    cat "$workdir/err" >&2
    exit 1
  fi
  route="$(jq -r '.route' <<<"$meta")"
  pages_json="$(jq --argjson meta "$meta" --arg route "$route" \
    '. + {($route): ($meta | del(.route))}' <<<"$pages_json")"
  routes_json="$(jq --arg stem "$stem" --arg route "$route" \
    '. + [{stem: $stem, route: $route}]' <<<"$routes_json")"
done

# Two pages declaring the same route: fail naming both, before installing
# anything. The dict-insert above would otherwise let the second writer win
# silently — this is the check that makes that impossible.
dupes="$(jq -c 'group_by(.route) | map(select(length > 1))' <<<"$routes_json")"
if [ "$(jq 'length' <<<"$dupes")" -gt 0 ]; then
  echo "generate-site-pages.sh: two pages declare the same route:" >&2
  jq -r '.[] | "  " + .[0].route + ": " + (map(.stem) | join(", "))' <<<"$dupes" >&2
  exit 1
fi

# The site URL, path prefix, repo link and group order — read back off site.typ
# through a target/-rooted driver. site.typ resolves its own reads relative to
# ITSELF rather than the root, so `--root .` here and `--root www` above both
# work against the same file. The groups ride along so the pages below can be
# validated against them and written in group/order order rather than the
# glob's filename order.
printf '#import "/www/utils/site.typ": GROUPS, PREFIX, REPO, SITE_URL\n#metadata((url: SITE_URL, prefix: PREFIX, repo: REPO, groups: GROUPS)) <site-meta>\n' \
  >"$workdir/site-meta-driver.typ"
site_meta="$(typst query --root "$REPO" "$workdir/site-meta-driver.typ" '<site-meta>' \
  --field value --one)"

groups_json="$(jq -c '.groups' <<<"$site_meta")"

# Every page must name a known group and an integer order. A missing field or a
# typo'd group is a page that would silently vanish from the sidebar and the
# README, so it is a hard failure naming the route.
group_problems="$(jq -r --argjson groups "$groups_json" '
  to_entries[]
  | .key as $route
  | .value as $page
  | if ($page.group | type) != "string" then
      "\($route): missing group"
    elif ($groups | index($page.group)) == null then
      "\($route): unknown group \($page.group | @json)"
    elif ($page.order | type) != "number" then
      "\($route): missing order"
    elif $page.order < 0 or $page.order != ($page.order | floor) then
      "\($route): order must be a non-negative integer, got \($page.order)"
    else empty end
' <<<"$pages_json")"
if [ -n "$group_problems" ]; then
  echo "generate-site-pages.sh: every page needs a known group and an order:" >&2
  while IFS= read -r problem; do echo "  - $problem" >&2; done <<<"$group_problems"
  exit 1
fi

# Two pages at the same position in one group would sort nondeterministically.
duplicate_orders="$(jq -r '
  [ to_entries[] | {route: .key, group: .value.group, order: .value.order} ]
  | group_by([.group, .order])
  | map(select(length > 1))
  | .[]
  | "\(.[0].group) order \(.[0].order): " + (map(.route) | join(", "))
' <<<"$pages_json")"
if [ -n "$duplicate_orders" ]; then
  echo "generate-site-pages.sh: two pages share an order within a group:" >&2
  while IFS= read -r problem; do echo "  - $problem" >&2; done <<<"$duplicate_orders"
  exit 1
fi

# A group with no page would render an empty heading in the sidebar and README.
empty_groups="$(jq -r --argjson groups "$groups_json" '
  ([ to_entries[].value.group ] | unique) as $used
  | $groups[]
  | . as $group
  | select(($used | index($group)) == null)
  | $group
' <<<"$pages_json")"
if [ -n "$empty_groups" ]; then
  echo "generate-site-pages.sh: groups with no page (remove them from GROUPS or add a page):" >&2
  while IFS= read -r group; do echo "  - $group" >&2; done <<<"$empty_groups"
  exit 1
fi

jq -n --argjson site "$site_meta" --argjson pages "$pages_json" '
  {
    url: $site.url,
    prefix: $site.prefix,
    repo: $site.repo,
    groups: $site.groups,
    pages: (
      $pages
      | to_entries
      | sort_by(. as $entry | [($site.groups | index($entry.value.group)), $entry.value.order])
      | from_entries
    ),
  }
' | emit site-pages.json

echo "generate-site-pages.sh: $(jq 'length' <<<"$pages_json") pages"
