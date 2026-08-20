#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "mmz: the manifest does not feed any cache digest",
  priority: framework("ice", confidence: 0.6, ease: 5.0, impact: 4.0),
  tags: ("cache", "config"),
  links: (
    related("mmz-config-composition-via-imported-fragments.typ")[noticed while
      settling that design],
    related("mmz-an-empty-tag-selection-passes-the-gate.typ")[the one real
      false green the investigation turned up, in the neighbouring surface],
  ),
  status: cancelled(2026, 8, 20),
)

== Summary

A rule's input digest is computed over its resolved files plus its probes'
stdout. The manifest that declares the rule is not part of it. So editing a
rule and leaving its inputs alone does not void that rule's record: the record
was written against the old declaration and still reads as fresh under the new
one.

Disconfirmed — the premise holds and the consequence does not. See _Finding_.

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

== Finding

The named test was run first, exactly as written above, and the rule does *not*
report fresh: it reports `missing-output`, `--is-fresh` exits 1 naming the
artifact, and the wrapped run re-runs. Every one of the three exposures was
then checked the same way against the real binary, and none of them is one.

The premise is true and load-bearing in the other direction. A record carries
an input digest and an exit status, and *nothing else about the rule is ever
read out of it*. Every field this task worried about is re-read off the current
manifest on the invocation that consults the record:

- `outputs:` — `status::verdict` stat-s `rule.outputs` as the manifest spells
  them today, never the `outputs:` list the record stored. An artifact declared
  after the fact voids the record until it exists. (The stored list is carried
  for `--status=json` and for naming the culprit, not for the verdict —
  `src/cache.rs` says so in as many words.)
- `match:` — the flip named here narrows. `sh -c …` stops matching the rule at
  all and hits `strict`'s `no_match` (exit 3, nothing run) rather than being
  handed a record measured under the wider matcher. Narrowing withdraws a
  match; it cannot grant a skip.
- `tags:` — a tag selects which rules a gate consults. It is not a measurement,
  and it does not become one by being absent: the record the newly tagged rule
  inherits is a pass of the same command over the same inputs, which is the
  entire content of a record. Move an input and the tagged rule fails the gate
  like any other.

So folding the declaration into the digest buys nothing that is not already
enforced, and would cost the global one-time re-run this task already called a
bad day. Not a trade — a bill for a feature that is already paid for.

=== The one limit worth stating plainly

Declared outputs are checked present-tense. Declare an artifact that happens to
already be on disk and the rule reads fresh, because a declared output is an
existence check made at read time and mmz never hashes one. That is the
documented contract (the Concepts page's trusted/not-trusted table), not a
residue of this task: the record is not being trusted for it — the filesystem
is, right now.

=== What was kept

- `tests/cli_declaration_changes.rs` pins all five behaviours end to end, so a
  refactor cannot quietly make this task's worry true after the fact.
- The Concepts page's _Live resolution, every time_ section now says the
  manifest is re-read on the same terms as the filesystem, which is the
  sentence whose absence made this task look plausible.

=== Turned up on the way

`--is-fresh --tag <tag>` exits 0 when *no* rule carries the tag — a vacuous
pass, in a surface where this repo's own task-closing gate spends it. Filed
separately as `mmz-an-empty-tag-selection-passes-the-gate.typ`; unlike the
three above, that one reproduces.
