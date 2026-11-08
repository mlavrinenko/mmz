#!/usr/bin/env sh
# `jq -e` fails on null/false, so a missing field fails here.
set -e

jq -e 'all(.[]; has("id") and has("customer") and (.total | type == "number"))' \
  data/orders.json >/dev/null
jq -e 'all(.[]; has("id") and has("name"))' data/customers.json >/dev/null

echo "validate: $(jq length data/orders.json) orders, $(jq length data/customers.json) customers OK"
