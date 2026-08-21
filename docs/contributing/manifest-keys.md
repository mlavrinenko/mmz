<!-- Generated from docs/src/contributing/manifest-keys.typ by `just docs md`. Do not edit; edit the source. -->

# Adding a manifest key

`Manifest` carries `deny_unknown_fields`, so the struct in `src/manifest.rs` is the manifest’s entire surface. Several things downstream are **derived** from that surface rather than discovering it independently, which is why a key added in one place and nowhere else fails a gate instead of shipping half-wired.

The step list lives on `Manifest`’s own doc comment, next to the field you are adding. This page is the part that does not fit in a struct header: why each coupling exists, and which of them will catch you.

## Nothing here is a convention

Every site is enforced by a test or a gate. That is deliberate, and it is the same argument the gate wiring makes: a list in a document rots, and a reader who misses a step gets a green build. A reader who misses a step here gets a red one, naming what is missing.

- The schema is asserted key-by-key against the struct by a plain unit test in `src/schema.rs`, not by a generate-then-diff gate — so drift in either direction fails `cargo test` without new gate wiring.
- The prose notes are asserted against the schema **in both directions** by `just check-doc-facts`. A key cannot ship with a schema description and no prose, and removing a key cannot leave prose behind.
- `exit_for` in `src/main.rs` matches exhaustively over the error enum, so a new error variant does not compile until it has an exit code.

The one thing not enforced is the doc comment on the field itself. Write it anyway; the reference renders the schema’s description, so the doc comment is what the next person editing the struct reads.

## Why the fragment schema is derived, not written

`schema/mmz-fragment.schema.json` is exactly the config schema with the policy keys removed. It is kept that way by assertion rather than by generation, because the interesting property is not “these two files agree” but “the only difference between them is the policy key set” — which is the same claim `compose::check_no_policy_keys` enforces at load time. Two mechanisms, one property, and the test is what stops them drifting apart.

So a non-policy key needs nothing in the fragment schema. A policy key needs the removal to still hold, which the same test checks.

## Policy keys are a second wiring path

A key that governs the whole run cannot live in an imported fragment: with three files importing each other, which fragment’s value governs a probe declared in a fourth is undecidable, and picking a rule would mean picking one nobody can predict from reading a single file. So `compose::POLICY_KEYS` names them, and they are rejected anywhere but the root.

Two details make that list cheap to extend correctly. It is a fixed-size array, so the destructurings in `compose_policy.rs` fail to compile until they are updated — the list cannot silently disagree with its readers. And each policy key is `Option<Option<T>>` on the internal `Document`, which is what lets an explicit `null` be told apart from an absent key: a fragment writing `gitignore:` is **setting** it and must be rejected exactly as `gitignore: true` would be, while the root writing it meaning `false` must not silently resolve to `true`.

`mmz --dump-config` then reports every policy key with a `(default)` marker when the root left it unwritten, which is only possible because that distinction survives the merge.

## The drift gates are asking a question

Editing `src/manifest.rs`, `src/probe.rs` or `src/main.rs` puts an `outdatty` group out of date, and `just outdatty-check` fails naming it. That is not a failure to fix — it is the gate asking whether you looked at the dependents it lists. Review them, then confirm:

```bash
outdatty update --group manifest-schema
```

Confirming without reviewing defeats the only purpose the group has. The groups exist because these dependents cannot be generated: a schema description and a prose note are both judgement, and a gate can only insist that someone exercised it.
