#!/usr/bin/env sh
# Total shipped revenue per customer -> out/report.txt (a declared output).
set -e
mkdir -p out

jq -r --slurpfile customers data/customers.json '
  ($customers[0] | INDEX(.id)) as $by_id
  | map(select(.status == "shipped"))
  | group_by(.customer)
  | .[]
  | "\($by_id[.[0].customer].name)\t\(map(.total) | add)"
' data/orders.json > out/report.txt

echo "report: wrote out/report.txt ($(wc -l < out/report.txt) rows)"
