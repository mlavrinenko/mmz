# www/sizes.jq — the jq program www/generate-facts.sh runs over www/sizes.yaml
# (via yq) to build sizes.json, the artifact every binary-size figure in the
# docs is read from. Kept as `jq -f` rather than inline for the same reason
# crate-map.jq and gates.jq are: the shell script stays under its linecop cap
# and the derivation is reviewable on its own.
#
# Three things happen here, and the third is the point.
#
#   - Bytes become the strings a page prints. A page rendering `bytes / 1e6`
#     itself would be four pages each rounding differently, and Typst is a poor
#     place to argue about a unit.
#   - The comparisons the prose makes are DERIVED, not re-read by a human: which
#     grammar is cheapest, which is dearest, what all of them cost together.
#     "The cheapest is json at 160 KB" was a sentence somebody had to re-rank by
#     hand after every measurement, and would not have been re-ranked.
#   - The recorded grammar set is cross-checked against the crate's own `lang-`
#     features, passed in as `$features`. A grammar added to Cargo.toml without
#     a re-measure, or one dropped from it while sizes.yaml still prices it,
#     fails HERE naming both sides — rather than rendering a table that quietly
#     omits a language mmz can parse.
#
# Deliberately NOT checked: `measured.crate_version` against the current one.
# Every release would then demand a forty-minute re-measure before the docs
# would build, which buys nothing — mmz's own code moves the number by a
# rounding error next to a grammar. The re-measure is scheduled by outdatty's
# `binary-size` group, which asks a human whether the dependency set moved
# enough to matter. What the version IS gets carried through below, so a page
# can say what it was measured against.

# Decimal MB/KB, the convention GitHub's asset listing and every "how big is
# the download" question use. Formatted through integer arithmetic rather than
# by dividing and hoping: `55.3 / 10` prints as 5.529999999999999 often enough
# to matter, and a docs build is not the place to discover which numbers do.
def mb:
  (. / 100000 | round) as $tenths
  | ($tenths / 10 | floor | tostring) + "." + ($tenths % 10 | tostring) + " MB";
def kb: (. / 1000 | round | tostring) + " KB";
def size_text: if . >= 1000000 then mb elif . >= 1000 then kb else (tostring + " bytes") end;
def sized: { bytes: ., text: size_text };

(.grammars | keys | sort) as $recorded
| ($features | sort) as $declared
| ($declared - $recorded) as $unmeasured
| ($recorded - $declared) as $unknown
| if ($unmeasured | length) > 0 or ($unknown | length) > 0 then
    ("sizes.jq: www/sizes.yaml and Cargo.toml's lang- features disagree.\n"
      + "Re-measure with `just measure-sizes`.\n"
      + (if ($unmeasured | length) > 0 then
           "  declared but not measured: " + ($unmeasured | join(", ")) + "\n" else "" end)
      + (if ($unknown | length) > 0 then
           "  measured but not declared: " + ($unknown | join(", ")) + "\n" else "" end))
    | halt_error(1)
  else . end
# The grammar entries, sorted by cost rather than by name: the extremes are what
# the prose quotes, and a sorted list is also the only order in which a table of
# twenty-seven rows says anything.
| ([.grammars | to_entries[] | { name: .key } + (.value | sized)] | sort_by(.bytes)) as $ranked
| {
    measured: .measured,
    builds: (.builds | map_values(sized)),
    # What all the grammars cost together, which is NOT the sum of the entries
    # above: each single-grammar build pays for the shared tree-sitter runtime
    # too, so adding them up double-counts it twenty-seven times. This is the
    # figure the "a grammar is not small" argument rests on, so it is measured
    # as a difference between two real builds.
    all_grammars: (.builds.full - .builds.none | sized),
    grammars: $ranked,
    count: ($ranked | length),
    cheapest: $ranked[0],
    dearest: $ranked[-1],
  }
