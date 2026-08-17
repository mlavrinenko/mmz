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

= Environment

`mmz` reads one variable out of the environment, and only for the times it
writes down:

/ #raw("MMZ_NOW"): pin "now" to a Unix epoch in seconds.

A cache record's `ran_at` is stamped from it, and `--status`'s `AGE` column is
measured against it. Both resolve it once per invocation, so a build that
captures either gets the same bytes every time — and a fixture can show a
genuinely aged record instead of the `0s ago` a just-recorded run always prints.
Unset, `mmz` reads the system clock.

It is deliberately not `SOURCE_DATE_EPOCH`. Dev shells and CI routinely export
that one at the 1980-01-01 zip-epoch floor, and honouring it here would silently
rewrite every stamp in every project that has it set.

#callout("note")[
  A value that is not an epoch is refused with exit 2 by every action that reads
  the clock: a wrapped run stops before the command runs, and both `--status`
  renderings stop before printing. Falling back to the system clock would hide
  the misconfiguration and hand back exactly the non-determinism the pin exists
  to remove.
]

Freshness is untouched by any of this. `mmz` compares digests, never times, so a
pinned clock changes what the output _says_ and never which rules are fresh.

= The full help text

Everything above is generated from this:

#transcript("help.txt")
