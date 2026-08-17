#import "../utils/page.typ": page
#import "../utils/ui.typ": callout, transcript
#import "../utils/site.typ": u

#let meta = (
  route: "/outputs/",
  label: "Declared outputs",
  title: "Declared outputs",
  summary: "A producer command's record can be undone without touching an input. Declaring what a run leaves behind is how mmz notices.",
)
#metadata(meta) <page-meta>

#show: page.with(..meta)

= The problem

A record claims: this command exited 0 while its inputs hashed to H. For a
verdict command that claim holds for as long as H does. For a command that
_produces_ something, the claim carries a side effect — and the effect can be
undone without touching a single input:

```bash
mmz just cover   # runs, records H, writes target/coverage/lcov.info
cargo clean      # artifact gone; sources untouched, H unchanged
mmz just cover   # fresh, skipped — and nothing to read
```

The record is not stale. It is #strong[void]: the run it describes has been
undone. Same story for a fresh clone, a new worktree, or a pruned `target/`.

= The fix

List what the run produces:

```yaml
commands:
  - name: just cover
    inputs: [rust]
    outputs:
      - target/coverage/lcov.info
```

The rule is now fresh only when its inputs still hash the same #strong[and]
every declared output exists. A missing one makes it stale whatever the inputs
say.

This does not replace the inputs. They remain the only evidence that an existing
artifact matches the sources it was built from. Outputs are the second, separate
way a record can stop being valid.

= Existence only

`mmz` never hashes an output.

The input digest already proves that an existing artifact is the one those
inputs produced, so hashing would buy tamper detection alone — catching a
hand-edited artifact. That is a different feature with a different cost, and it
is deliberately left out.

Outputs are literal paths relative to the project root, stat-ed directly and
never walked:

- A glob is a manifest error, rather than a pattern that silently never matches.
- A `{scope}` macro is not substituted here.
- Because nothing is walked, the `gitignore` filter never applies. An artifact
  under an ignored `target/` needs no
  #link(u("/inputs/"))[artifact-scope opt-out], unlike the same path used as an
  _input_.
- A directory counts as present.

= What it looks like when it fires

`--status` reports `missing-output` and names the path in its own column:

#transcript("status-missing-output.txt")

`--is-fresh` fails with the reason, not a generic one:

#transcript("is-fresh-missing-output.txt")

And `--status=json` reports the state as `missing_output`, with the outputs the
recorded run promised under `cached.outputs` — a record remembers what it
declared, so a missing artifact is reported against the run that promised it.

#callout("note")[
  A wrong _reason_ sends a reader to look at the wrong thing. "Inputs changed"
  when the inputs did not change costs more time than no message at all, which is
  why this state is distinct rather than folded into `stale`.
]

= A run that produces nothing

If a wrapped command exits 0 without producing a declared output, that is a hard
error: `mmz` prints the missing path, writes no cache record, and exits 5.

Recording it anyway would leave a rule that quietly never hits again — the exact
failure this feature exists to end. Skipping the record _silently_ would be
worse: a rule that re-runs forever with no explanation.

A run that #emph[fails] is untouched by this. Its own exit code is the story, and
its failure is reported exactly as before.
