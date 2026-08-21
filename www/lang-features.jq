# www/lang-features.jq — which grammars this crate can be built with, read out
# of `cargo metadata --format-version 1 --no-deps`.
#
# Its own file because two programs need the answer and they must agree:
# www/measure-sizes.sh builds one binary per entry, and www/generate-facts.sh
# checks the committed measurement against this list. Two copies of the rule
# would be two places for "what counts as a grammar feature" to drift, and the
# symptom would be a docs build failing to explain itself.
#
# `lang-all` is excluded because it is the aggregate rather than a grammar; it
# is priced separately, as a whole build. Sorted here so neither caller has to
# remember to, and so a diff of either's output is about a grammar changing
# rather than about map ordering.
[
  .packages[]
  | select(.name == "mmz")
  | .features
  | keys[]
  | select(startswith("lang-"))
  | select(. != "lang-all")
  | ltrimstr("lang-")
]
| sort
