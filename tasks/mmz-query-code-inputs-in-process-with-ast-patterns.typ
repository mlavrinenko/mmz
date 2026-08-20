#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "mmz: query code inputs in-process with AST patterns",
  priority: framework("ice", confidence: 0.5, ease: 2.0, impact: 6.0),
  tags: ("config", "efficiency"),
  links: (
    related("mmz-query-json-inputs-in-process-instead-of-shelling-out.typ")[the
      same argument for JSON; deliberately a separate feature],
    related(
      "mmz-an-ast-probe-hashes-the-whole-matched-node-not-its-captures.typ",
    )[the
      known limit of what shipped],
  ),
  status: done(
    2026,
    8,
    21,
  )[Shipped as \`ast:\` + \`lang:\`, +1.71 MiB (the jq engine was +1.41 MiB). Grammar cost was measured per language rather than estimated — all 27 come to 40 MB linked, so each is a cargo feature and the default is \`lang-rust\` alone. Verified end to end: a Rust probe stays fresh across comment, import and private-body edits and busts on a signature change; a non-default grammar (\`--features lang-python\`) behaves the same; a grammar-less build refuses loudly instead of aborting on ast-grep's \`unimplemented!()\`. Two open questions were answered against the code rather than as designed — a bare metavariable is legal (over-declaration is the safe direction), and a malformed pattern error-recovers rather than failing, so \`Pattern::has\_error\` is what makes it refuse. Match order is kept, not normalised: sorting would hide a real edit. The whole-node limit the task's own example implies is filed separately.],
)

== Summary

A scope names whole files, and a probe reaches part of one only by shelling out
to something that can parse it. Bundling an AST matcher would let a rule depend
on a *structural* slice of a source file — a function, a type, an impl block —
without a subprocess and without a regex pretending to be a parser.

```yaml
probes:
  public-api:
    file: src/lib.rs
    ast: 'pub fn $NAME($$$ARGS) -> $RET'
```

== Why it is worth its own task

The motivation is shared with in-process JSON querying — a spawn per probe is
both the cost and the risk — but almost nothing else is. JSON has one obvious
data model and a settled query language; code has one grammar per language, and
bundling a matcher means bundling grammars. ast-grep is the natural candidate
and it carries tree-sitter grammars per language, which is a materially
different dependency conversation from a JSON selector.

Shipping them together would let the harder half hold the easy one hostage.

== Why it might be worth doing anyway

It is the case where the spawn disappears completely rather than halving:
reading a file and matching it needs no `run:` line at all, so such a probe has
no ambient-tool dependency and no shell quoting to get wrong.

It also reaches inputs nothing currently can. "This gate depends on the public
API of `lib.rs`, not on its comments" is not expressible today — the closest
approximation is hashing the whole file, which is the over-declaration the
recipe-body probes were introduced to escape in the first place.

== Open

- Which grammars ship, and what a manifest naming a language mmz was not built
  with does. It must be a loud error, never a silent empty match — the
  `jq -e` lesson.
- Binary size and build time. Tree-sitter grammars are not small, and mmz is a
  tool people `cargo install`.
- Whether a pattern matching nothing is an error (consistent with
  `allow_empty`) and whether match *order* is normalized, since a digest over
  an unordered match set has the same latent instability `jq -S` was needed
  for.
- Whether this is mmz's job at all, or an argument for a probe protocol that
  lets someone else's matcher plug in.
