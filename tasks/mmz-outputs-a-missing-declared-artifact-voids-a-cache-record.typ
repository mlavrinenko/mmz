#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "mmz: outputs — a missing declared artifact voids a cache record",
  priority: framework("ice", confidence: 0.9, ease: 6.0, impact: 7.0),
  tags: ("config", "cache"),
  links: related(
    "mmz-per-scope-gitignore-opt-out-for-artifact-paths.typ",
  )[the consumer-side half of the same problem],
  status: done(2026, 8, 16),
)

== Summary

A command rule gains an optional `outputs:` list of literal paths. A rule is
fresh only when its inputs still hash the same AND every declared output
exists. A missing output makes the rule stale no matter what the inputs say.

```yaml
commands:
  - name: just cover
    inputs: [rust]
    outputs:
      - target/coverage/lcov.info
```

Existence only. mmz does not hash outputs.

== Why

mmz's record is a claim: this command exited 0 while its inputs hashed to H.
For a verdict command (`fmt-check`, `clippy`) that claim stays true for as long
as H holds, and nothing in the world can falsify it. For a producer command the
claim carries a side effect, and the effect can be undone without touching a
single input:

```
just cover      # runs, records H, writes target/coverage/lcov.info
cargo clean     # artifact gone; sources untouched, H unchanged
just cover      # mmz: fresh, skipped
just crap       # nothing to read
```

The record is not stale. It is void: the run it describes has been undone. Same
story for a fresh clone, a new worktree, or a pruned `target/`.

This is not "trust the previous result". Inputs stay the only evidence that the
artifact matches the sources; outputs are the second way a record can stop
being valid.

== Why existence and not a hash

The input hashes already prove that an existing artifact is the one those
inputs produced, so hashing outputs buys only tamper detection: catching a
hand-edited artifact. That is a different feature with a different cost, and it
is not what any consumer has asked for. Deliberately left out rather than
overlooked. File it separately if a case appears.

== Scope

- `src/manifest.rs`: `Command.outputs: Vec<PathBuf>`, literal paths only,
  relative to the project root. Reject a glob metacharacter at parse with a
  message saying outputs are literal, so nobody discovers it as a silent
  never-matches.
- `src/freshness.rs`: freshness is inputs-match AND all-outputs-exist. The
  stale reason must name the missing path, not just say "inputs changed" —
  a wrong reason here sends a reader to look at the wrong thing.
- `src/cache.rs`: record the outputs declared at record time, so `--status` can
  report a missing one against the run that promised it.
- `src/status.rs` and `schema/status.schema.json`: surface the missing output in
  both the table and the JSON (`status-schema` outdatty group).
- A successful run that did not produce a declared output is a hard error:
  print the missing path, write no cache record, exit with a dedicated,
  documented code. Never silently skip the record — that is a rule that
  quietly never hits again, the exact failure mode this feature exists to end.
- `schema/mmz.schema.json`, `README.md`, `www/index.html`, `--help`: the new
  key and the new exit code.
- Tests: output present stays fresh; deleted output goes stale with the path
  named; a command that succeeds without writing its output errors and records
  nothing; a rule with no `outputs` behaves exactly as today.

== Independent of the gitignore work

Outputs are stat-ed directly, never glob-walked, so the ignore filter never
applies to them and this does not wait on the per-scope `gitignore` key. Keep
it that way: outputs are paths, not patterns.

== Home

mmz's own backlog. Filed out of a MindTape discussion of just's `[cache]`
attribute, whose `outputs` field is the same idea; just checks existence too.
