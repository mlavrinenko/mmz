<!-- Generated from docs/src/contributing/gates.typ by `just docs md`. Do not edit; edit the source. -->

# Gates

`just check` runs ten gates. CONTRIBUTING.md lists them; this file argues the machinery around them.

## Why the gate memoizes itself

mmz is a memoized command runner, so its own gate is the obvious place to find out whether the idea survives contact with a real project. It does, and the shape of the win is worth being precise about.

A full cold `just check` compiles the workspace twice (clippy and test use different profiles), runs the suite, formats three languages, builds the docs site, and renders the generated Markdown. A warm one, with nothing changed, resolves each rule’s globs, hashes the files, and skips every arm.

What that buys is not really the seconds. It is that **asserting the gate passed becomes cheap enough to do routinely**. Closing a task runs `mmz --is-fresh --tag gate`, which is a hash comparison rather than a re-run — so “done” can mean “the build passed against this worktree” without anyone waiting for a rebuild at the least convenient moment.

## The double hop

Each arm of `check` is `just memo <gate>`, and `memo` is:

```just
memo *ARGS: _mmz
    {{ mmz }} just {{ ARGS }}
```

So `just check` runs `mmz just clippy`, and the rule that decides whether anything happens is named `just clippy` in `.mmz/config.yaml`.

The indirection earns its keep. The alternative — inlining each gate’s command line into `check` and naming _that_ in the manifest — is what this repo did before, and it meant `.mmz/config.yaml` held a hand-maintained mirror of every recipe body. A recipe growing a flag silently desynchronised the mirror, and nothing failed: the rule still matched, and still memoized the old command’s identity.

Naming the recipe instead means the manifest names something stable. What the recipe _does_ is then an input, not an identity — which is what the per-recipe probes are for.

## Probes on recipe bodies

Every gate rule pins its own recipe body through a probe:

```yaml
probes:
  recipe-clippy:
    run: just --dump --dump-format json | jq -S -e -c '.recipes["clippy"]'
```

The whole `Justfile` used to be a scope on every rule, which meant editing one docs recipe busted clippy and the full test suite. Hashing one recipe’s dumped JSON instead scopes the dependency to what actually changed.

Three details are load-bearing:

- `jq -e` exits non-zero when its selector yields `null`, so a **renamed recipe becomes a loud probe failure** rather than a digest that quietly stops tracking anything. Without `-e`, a rename would leave the rule permanently fresh — the exact silent failure the whole design is built to avoid.
- `jq -S` sorts object keys, so the digest tracks the recipe’s content rather than the key order whichever `just` is on PATH happens to emit. Two just versions render the same recipe into the same JSON with the keys ordered differently; without `-S` that alone reads as every gate stale. It removes the accidental version dependency, not the deliberate one — a `just` whose rendering differs in substance still moves the digest, as it should.
- Probes are resolved once per invocation however many rules name them, but each distinct probe is its own process. Ten recipe probes are ten `just --dump` runs. That is the cost of the granularity, it is paid on every `mmz --is-fresh`, and it is small enough only because `just --dump` is fast.

## Wiring a new gate

A gate is wired in three places, and the build fails if you stop at two:

1. The recipe carries `[group("gate")]` and a `[doc(...)]` string.
2. `check`’s dependency list names it, as `(memo "<name>")`.
3. The right fragment under `.mmz/conf.d/` declares a rule named `just <name>`, tagged `gate`, with the scopes and probe that gate really reads.

`.mmz/config.yaml` itself never grows a rule for this — it holds policy and the `imports:` list, nothing else, since [composition](https://mlavrinenko.github.io/mmz/composition/) landed. Picking (3)‘s fragment is picking which concern the gate belongs to: `.mmz/conf.d/10-rust.yaml` for a gate reading Rust sources or the toolchain, `.mmz/conf.d/20-docs.yaml` for the docs pipeline, `.mmz/conf.d/30-repo.yaml` for a repo-wide script or drift check. A gate that needs a scope another fragment already declares references it by name rather than redeclaring it — a scope lives in exactly one file, and `mmz --dump-config` prints which. A gate that fits none of the three earns its own fragment; the numbering leaves room between and after the existing files.

Miss (1) or (2) and `www/gates.jq` halts naming both sides — membership is derived from the group tag and cross-checked against the dependency list, precisely because either alone can be silently wrong. Miss (3) and the arm fails at the `no_match` strict case, which is the fail-closed behaviour working as intended: mmz refuses to run a command it was asked to memoize but has no rule for.

You also need a `www/utils/gate-notes.typ` entry, or `just check-doc-facts` fails naming the gate — CONTRIBUTING.md’s table has a column that has to say something.

## Getting the scopes right

The gate rules are the highest-stakes rules in the repo, because a wrongly-fresh gate is a green build that proved nothing. The rule of thumb from [the correctness contract](https://mlavrinenko.github.io/mmz/concepts/) applies with extra force here: **over-declare**.

Specifically, a gate’s scopes should include the toolchain pins — both `rust-toolchain.toml` and `flake.lock`, since the dev shell is what actually selects the compiler. A toolchain bump that did not bust the clippy cache would mean shipping against lints nobody ran.

## Running a gate directly

`memo` is a wrapper, not a requirement. Every gate is an ordinary recipe:

```bash
just clippy          # runs, always, and shows you the output
just memo clippy     # runs only if stale; records a pass on success
```

Reach for the bare form while iterating — a memoized run that skips is exactly the wrong thing when you are trying to see an error message. Reach for `just check` when you want the recorded pass that closing a task will assert.

Note that `just test <filter>` deliberately bypasses memoization: a filtered run is a different command from the gate’s full-suite run, and recording it under the gate’s identity would claim a pass the gate never earned.
