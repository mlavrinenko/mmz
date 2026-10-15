#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "Print one summary line from mmz --import",
  tags: ("cli", "tooling"),
  priority: framework("ice", confidence: 0.9, ease: 8.0, impact: 5.0),
  status: wip(2026, 10, 15),
)

== Summary

\`mmz --import \<dir>\` prints one line per imported rule, then \`mmz: imported N records from \<dir>\`. The worktrunk \`pre-remove\` hook runs it on every land, so a run importing 31 records puts 31 rule names into an agent's context for nothing. The count also reads "imported 1 records". mmz has no verbosity flag, so the per-rule list goes: print the summary line only, with a correct plural ("1 record", "N records"). Done when the output is one line and the CLI reference, the 0.12.0-era CHANGELOG wording and the tests match.
