#!/usr/bin/env sh
# Assert every data file parses and carries the fields the report needs.
# `jq -e` exits non-zero on a null/false result, so a missing field fails here
# rather than producing a plausible-looking empty report downstream.
set -e

jq -e 'all(.[]; has("id") and has("customer") and (.total | type == "number"))' \
  data/orders.json >/dev/null
jq -e 'all(.[]; has("id") and has("name"))' data/customers.json >/dev/null

echo "validate: $(jq length data/orders.json) orders, $(jq length data/customers.json) customers OK"
