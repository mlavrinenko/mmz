#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "mmz: an ast probe hashes the whole matched node, not its captures",
  priority: framework("ice", confidence: 0.6, ease: 3.0, impact: 5.0),
  tags: ("config", "efficiency"),
  links: (
    related("mmz-query-code-inputs-in-process-with-ast-patterns.typ")[the
      feature this is the known limit of],
  ),
  status: done(
    2026,
    8,
    21,
  )[Shipped as \`capture:\`, a new optional key on an \`ast:\` probe. It is a new key rather than a change to \`ast:\`: folding it into the pattern string would need a syntax mmz invents, and the pattern already has ast-grep's. The default is unchanged — a match is still a whole node — so the surface grows by one key that a probe not needing it never writes. Every open question was answered against the code. Order is neither the manifest's list order nor the pattern's: the list is sorted before hashing, because it is the \*set\* of parts that matter, so retyping \`\[RET, NAME, ARGS\]\` is not an edit. That sort cannot hide one, unlike sorting match order would, since only two spellings of one set ever normalise together. A name the pattern uses twice binds once, so there was nothing to join or collapse: ast-grep keys its env by name and already requires the two occurrences to have matched identical nodes. An anonymous \`\$\$\$\` or a \`\$\_\` is not nameable at all — ast-grep drops both rather than binding them, so they never reach an env and never appear in \`Pattern::defined\_vars\`. An undefined name is the \`jq -e\` refusal for the third time, and the sharpest form of it yet: it would render an empty \`(\$TYPO)\` in every match and narrow the probe silently, with every match still present, so \`allow\_empty: true\` could not even be blamed. It is raised from the compiled pattern rather than from a match, so it fires on a source with no matches too, and names what the pattern does define. Three further refusals need no grammar and land at load instead: an empty list, a name that could never be a metavariable (\`\$NAME\` copied out of the pattern, a lowercase name, a \`\$\_X\` that ast-grep drops), and a duplicate. The last open question is the one not answered: how often a grammar glues a signature to a body was not measured across a corpus. The judgement made instead was that an opt-in key with an unchanged default costs little to carry, and the docs steer a reader to the pattern wherever a pattern can already stop at the boundary they care about. Verified end to end with the real binary as well as in tests: a probe capturing \`\[NAME, ARGS, RET\]\` off \`pub fn \$NAME(\$\$\$ARGS) -> \$RET { \$\$\$BODY }\` stays fresh across a comment rewrite, a body rewrite and a private-function edit, and busts on an added parameter.],
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
