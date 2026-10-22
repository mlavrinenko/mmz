#import "@local/mindtape:0.2.0": *
#import "/.mindtape/lib.typ": *

#show: task.with(
  title: "Force every wrapped run with MMZ_DISABLE=1",
  kind: "feat",
  priority: ice(
    impact: 6,
    confidence: 0.9,
    ease: 7,
  ),
  difficulty: 3,
  tags: ("cli", "cache"),
  estimate: (src-files: 5),
  x-goals: (
    product: (
      no-false-green: helps[a record someone distrusts is re-verified with one variable, without deleting the cache],
      exact-inputs: neutral,
      cheap-gates: neutral,
      agent-drivable: helps[an agent forces a full re-run with one documented variable instead of guessing at the cache layout],
      composable-manifest: neutral,
      ready-binaries: neutral,
      docs-teach-judgement: neutral,
    ),
    process: (
      mmz-gates-mmz: helps[`MMZ_DISABLE=1 just check` re-runs every arm of this repo's gate and re-records it],
      docs-read-facts: neutral,
      rules-not-memory: neutral,
      cheap-delegation: neutral,
    ),
  ),
  assignee: "agent",
  status: wip(2026, 10, 23),
)

== Problem

+ Run `just check` in this repo, so every arm records a pass.
+ Suspect a record and want the whole gate to run again without editing any input.
+ 🔴 There is no switch. Every arm hardcodes `mmz just <gate>`, so the only way is deleting `.mmz/cache` by hand, which also drops the records of rules outside the gate.


== TO-BE

- 🟢 With `MMZ_DISABLE=1`, a wrapped run never skips: it runs the command and records its outcome as usual, so a forced `just check` still lets `mt done` pass.
- 🟢 Matching, `strict`, probes and `outputs` behave as without it; only the skip is gone.
- 🟢 `--is-fresh` and `--status` ignore it, since they run nothing.
- 🟢 Unset, empty or `0` is off; any other value is refused with exit 2 naming the variable, as a malformed `MMZ_NOW` is.

```bash
MMZ_DISABLE=1 just check     # every arm runs and re-records
MMZ_DISABLE=yes mmz cargo test   # exit 2: MMZ_DISABLE takes 1 or 0
```


== Changes

- `tests/it/cli_disable.rs` and its `mod` line in `tests/it/main.rs`: the red test first, a fresh rule re-running under `MMZ_DISABLE=1` and a bad value exiting 2.
- `src/disable.rs`: read and validate the variable once, beside how `src/clock.rs` reads `MMZ_NOW`.
- `src/engine.rs`: resolve it in `run` with the clock, and skip the freshness check in `memoized` when it is on.
- `src/error.rs` and `src/main.rs`: an `InvalidDisable` error mapped to exit 2, and the variable in the help's Environment block.
- `src/lib.rs`: the module.
- `www/content/cli.typ`: the Environment section names two variables, not one.


== Alternatives

- Write no record under the switch. Then a forced `just check` leaves the gate unfresh, and `mt done` refuses after a full passing run.
- Make `--is-fresh` fail under it. A session that exports it could then never close a task.
- A `--force` flag. Each `just check` arm hardcodes its argv, so a flag would need every recipe to pass it through.
- Read `CI`. mmz's inputs already cover tool pins, so a CI runner's records are as true as a laptop's.


== Out of scope

- A per-rule switch. A rule that should never skip is a rule that should not be wrapped.


== Dogfood

`docs/src/contributing/gates.typ` names `MMZ_DISABLE=1 just check` as the way to force this repo's full gate.

