#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "mmz: an ast probe hashes the whole matched node, not its captures",
  priority: framework("ice", confidence: 0.6, ease: 3.0, impact: 5.0),
  tags: ("config", "efficiency"),
  links: (
    related("mmz-query-code-inputs-in-process-with-ast-patterns.typ")[the
      feature this is the known limit of],
  ),
  status: proposed(2026, 8, 21),
)

== Summary

An `ast:` probe's input is every node its pattern matched, entire. That is a
clean rule and it is the right default, but it puts the motivating example of
the feature slightly out of reach:

```yaml
probes:
  public-api:
    file: src/lib.rs
    ast: 'pub fn $NAME($$$ARGS) -> $RET { $$$BODY }'
```

That probe depends on the signatures *and* the bodies. There is no pattern that
keeps one and drops the other, because a Rust function's signature stops being a
node of its own once a body follows it — `pub fn $N($$$A) -> $R` parses as a
`function_signature_item` and matches trait and `extern` declarations only.

So "this gate depends on the public API of `lib.rs`, not on its bodies" is still
not expressible. What ships today is narrower than a scope and wider than the
example promised.

== The shape a fix would take

ast-grep already captures metavariables. Hashing the *captures* rather than the
matched node would let the pattern say which parts matter:

```yaml
ast: 'pub fn $NAME($$$ARGS) -> $RET { $$$BODY }'
capture: [NAME, ARGS, RET]
```

The rendering machinery is unchanged — a capture is a node, and
`crate::ast_render` already renders one canonically.

== Open

- Whether it is a new key or a change to `ast:`. A key is another entry in a
  surface that just grew two; folding it into the pattern string would need a
  syntax mmz invents, which is worse.
- What a pattern with no captures means under it, and what a named capture that
  the pattern does not define should do. The latter must be a loud error — a
  silently-empty capture set is the `jq -e` failure again, and this feature has
  already had to answer it twice.
- Whether `$$$` (an anonymous multi-capture) is nameable at all.
- Whether capture *order* is the manifest's list order or the pattern's, and
  whether a capture appearing twice in one match joins or collapses.
- Whether this is worth it. A probe that names three captures is more to read
  than one that names a pattern, and the win is confined to constructs whose
  grammar glues a signature to a body. Measure how often that is the case
  before spending the surface on it.
