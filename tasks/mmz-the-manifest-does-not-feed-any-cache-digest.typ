#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "mmz: the manifest does not feed any cache digest",
  priority: framework("ice", confidence: 0.6, ease: 5.0, impact: 4.0),
  tags: ("cache", "config"),
  links: (
    related("mmz-config-composition-via-imported-fragments.typ")[noticed while
      settling that design]
  ),
  status: proposed(2026, 8, 20),
)

== Summary

A rule's input digest is computed over its resolved files plus its probes'
stdout. The manifest that declares the rule is not part of it. So editing a
rule and leaving its inputs alone does not void that rule's record: the record
was written against the old declaration and still reads as fresh under the new
one.

== What actually goes stale

Not `inputs:` — adding or widening a scope changes the resolved file set, so
the digest moves on its own. The exposure is the rule fields that change what a
pass *means* without changing what it reads:

- `outputs:` — adding a declared artifact should void a record written before
  anyone was checking for it. Today it does not, and the rule reads fresh with
  the artifact never having been verified once.
- `match:` — flipping `prefix` to `exact` narrows what the record covers.
- `tags:` — a rule newly tagged `gate` inherits a record that was never
  measured as a gate, and `--is-fresh --tag gate` accepts it.

`on_hit` is cosmetic and does not belong in a digest.

== Why file it now

Pre-existing, and orthogonal to composition — but composition makes it easier
to hit. A generated fragment is rewritten by a tool rather than edited by a
person, so "the declaration changed but the inputs did not" stops being a rare
hand-edit and becomes a thing that happens on every regeneration.

== Shape of a fix, not yet chosen

Fold a digest of the rule's own *declaration* into its input digest — its
normalized `outputs`, `match` and `tags`, deliberately not its `name` (which is
already the record key) and not `on_hit`. Cheap, and it makes the record say
what it was measured against.

The cost is the reason this is not obviously right: every rule's digest changes
once, so the first run after upgrading re-runs every gate in every project that
uses mmz. That is a correct one-time invalidation and it is still a bad day, so
it wants a release note and probably a version bump that says so.

== Before implementing

Confirm the claim rather than trusting this task: write the test first. Record
a pass, add an `outputs:` entry naming a path that does not exist, and check
whether the rule still reports fresh. If it does not, this task is wrong and
should be cancelled with the finding written down.
