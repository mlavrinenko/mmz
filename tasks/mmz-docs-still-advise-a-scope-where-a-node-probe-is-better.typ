#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "mmz: docs still advise a scope where a node probe is better",
  priority: framework("ice", confidence: 0.8, ease: 7.0, impact: 3.0),
  tags: ("docs", "config"),
  links: (
    related("mmz-gate-rules-do-not-declare-the-tools-they-run.typ")[the change
      that made this advice sit at odds with the project's own practice],
  ),
  status: proposed(2026, 8, 20),
)

== Summary

Two pages tell a reader to put `flake.lock` in a *scope* to make a toolchain
bump bust the cache:

- `www/content/concepts.typ` — "put `rust-toolchain.toml` or `flake.lock` in a
  scope and a toolchain bump busts the cache".
- `www/content/agents.typ` — "The toolchain pins, if a toolchain change should
  re-run it — `rust-toolchain.toml`, `flake.lock`".

Both are true statements about how mmz behaves, and neither is a claim about
this repo's own wiring, which is why they were left alone when the gates moved
to per-node probes. But this project just concluded that the whole-file spelling
is the wrong one at any real lockfile size, and its docs still recommend it.

== Why it is worth fixing rather than leaving

The advice is not wrong, it is *unqualified*. A small lockfile with three inputs
is fine in a scope. This repo's has over a hundred nodes, and hashing all of them
meant a transitive `nixpkgs-lib` bump re-ran clippy and the full test suite —
which is exactly the over-busting a reader following this advice will hit, at the
point where they have enough inputs for it to matter and no hint that a finer
spelling exists.

The docs are also the only place a reader meets the choice. `probes[].file` and
`probes[].json` document the mechanism; nothing connects it to the toolchain
question these two pages raise.

== Shape of the fix

Keep the scope advice as the simple default, add the node probe as what to reach
for when the lockfile is large or when one input's tools are wanted without the
rest:

```yaml
probes:
  tola-tools:
    file: flake.lock
    json: '.nodes["tola"]["locked"]["narHash"]'
```

Note the two are not equivalent in kind: a tool that is its own flake input is
pinned exactly by its node, while a tool out of nixpkgs shares one node with
everything else from nixpkgs and the probe is a proxy. `.mmz/conf.d/10-rust.yaml`
argues this at length and the docs should not repeat it — link it, or restate it
in one sentence.

== Watch the line cap

`www/content/inputs.typ` sits at 244 lines against a 250-line Typst cap in
`.linecop.yaml`. If the cross-reference lands there rather than on the two pages
above, something else in that file has to give.
