#import "../utils/page.typ": page
#import "../utils/ui.typ": callout, listing, transcript
#import "../utils/cli-ref.typ": (
  action-prose, action-table, actions, exit-code-table, states,
)
#import "../utils/site.typ": u

#let meta = (
  route: "/cli/",
  label: "CLI reference",
  title: "CLI reference",
  summary: "Every action mmz accepts, every exit code it returns, and the JSON it can be asked for — generated from the binary's own help text.",
)
#metadata(meta) <page-meta>

#show: page.with(..meta)

// The action list, the exit-code list and the help panel below all come from
// the same `mmz --help` capture (www/generate.sh); the prose beside each comes
// from www/utils/cli-notes.typ and www/utils/exit-code-notes.typ, which
// `just check-doc-coverage` and `just check-doc-facts` require to cover exactly
// the actions and codes the binary advertises.

#action-table

= Exit codes

`mmz` returns the wrapped command's own exit code when it runs one, so these are
the codes that mean something happened before, instead of, or around that.

#exit-code-table

Distinguishing them matters for the two that write no record — 5 and 6 — because
they are the cases where a run happened and deliberately left no claim behind.

= Rule states

`--status` and `--status=json` report one of these per rule:

#list(..states.enum.map(s => raw(s)))

#states.description

= Actions in detail

#for action in actions {
  action-prose(action)
}

= JSON output

`--status=json` reports every rule with its resolved inputs and their hashes,
plus what the record saw, so a disagreement is diffable rather than guessable:

```bash
mmz --status=json | jq '.rules[] | select(.state != "fresh")'
```

Its shape is itself schema-documented — `mmz --status=json-schema` prints the
JSON Schema, so a consumer can validate what it parses instead of pattern-matching
on field names.

#callout("note")[
  This is the form to reach for when a rule is stale and the question is _which
  input moved_. The resolved set and the recorded set are both in there.
]

= The full help text

Everything above is generated from this:

#transcript("help.txt")
