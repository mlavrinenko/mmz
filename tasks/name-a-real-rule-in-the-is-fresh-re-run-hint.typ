#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "Name a real rule in the is-fresh re-run hint",
  tags: ("cli",),
  priority: framework("ice", confidence: 0.9, ease: 9.0, impact: 4.0),
  status: proposed(2026, 10, 15),
)

== Summary

\`mmz --is-fresh\` ends a failing run with a hint whose example is a fixed \`mmz just check\`. In a repo with no rule for \`just check\`, mmz refuses that very example (\`no rule matches just check\`). The hint also joins its clauses with an em dash. Done when the example is the first remediable listed rule's own name, so mmz always accepts it, and the dash is gone.
