#import "../utils/page.typ": page
#import "../utils/ui.typ": callout
#import "../utils/site.typ": u

#let meta = (
  route: "/code/",
  label: "Code inputs",
  title: "Code inputs: matching source with AST patterns",
  summary: "The `ast:` selector — depending on a function, a type or an impl block instead of a whole file — what it hashes, how `capture:` narrows a match to the parts that matter, and which grammars a build carries.",
)
#metadata(meta) <page-meta>

#show: page.with(..meta)

A scope names whole files. A `json:` probe narrows a document. Neither reaches
_part of a source file_, so a rule that depends on the shape of some code has to
hash every byte around it — every comment, every private body, every blank line
somebody's editor added.

`ast:` closes that gap by parsing the file and matching an
#link("https://ast-grep.github.io/")[ast-grep] pattern against the tree:

```yaml
probes:
  wire-types:
    file: src/types.rs
    ast: 'pub struct $NAME { $$$FIELDS }'

commands:
  - name: cbindgen
    inputs: [wire-types]
```

That rule depends on the public struct definitions in `types.rs` and on nothing
else in it — not the imports, not the private types, not a word of the prose
around them. Nothing is spawned, nothing has to be on `PATH`, there is no shell
quoting to get wrong, and no regex is asked to pretend it is a parser.

`$NAME` captures one node, `$$$FIELDS` captures a run of them. The pattern is
written in the target language, so the way to check one is to read it as code.

== A match is a whole node

This is the rule to hold on to, because it decides what a pattern costs you: the
input is every node the pattern matched, entire. A pattern that spans a
function's body depends on that body.

```yaml
ast: 'pub fn $NAME($$$ARGS) -> $RET { $$$BODY }'   # signatures AND bodies
```

There is no pattern that keeps the signature and drops the body, because the
signature is not a node of its own once a body follows it. `capture:`, below, is
the way out of that; everywhere a pattern can already stop at the boundary you
care about, it is the shorter answer and the one to reach for.

What a pattern buys over a scope either way: everything it did not match is free
to move. Comments, imports, private items, other functions, the file growing a
second module — none of it is an input.

= What is hashed

Not the matched text — mmz's own rendering of the matched _tree_:

```
(function_item (visibility_modifier "pub") (fn "fn") (identifier "one") …)
```

That rendering keeps every token exactly, including the operators and keywords
that are anonymous nodes, and drops the whitespace between them, which no parse
records. So:

#table(
  columns: 2,
  table.header[Edit][Effect on the digest],
  [Reflowing a matched construct across lines], [none],
  [Rewording a comment outside the match], [none],
  [Anything the pattern did not match], [none],
  [Renaming a matched item], [busts],
  [Adding a parameter, or a trailing comma], [busts],
  [`a + b` becoming `a - b`], [busts],
  [Changing whitespace _inside_ a string literal], [busts],
  [Reordering two matched declarations], [busts],
  [Editing a body the pattern spans], [busts],
  [Editing a body the pattern spans but `capture:` does not name], [none],
)

This is the same line #link(u("/inputs/"))[`json:` draws] when it sorts object
keys and leaves array order alone: mmz normalises the presentation a renderer
chose, and never the order a document chose. Sorting matches would make two
files differing only in declaration order report one digest — a _narrowing_,
and a probe that cannot see a real edit is the failure this tool exists to
prevent.

#callout("note")[
  "Reflowing" means whitespace only. A formatter that also adds a trailing comma
  has added a token, and a token is content — `rustfmt` does exactly this when it
  breaks an argument list across lines, so the first reflow after a probe is
  written often busts once and then settles.
]

#callout("note")[
  A rendering names node kinds,
  so a grammar bump can move a digest that no edit
  moved. That is a false stale: rules re-run once and settle. Hashing the
  matched text instead would be steady across grammar bumps and blind to
  reformatting — steadier, and wrong in the direction that matters.
]

= Naming the parts that matter

`capture:` narrows each match to the metavariables it names, written without the
`$`:

```yaml
probes:
  public-api:
    file: src/lib.rs
    ast: 'pub fn $NAME($$$ARGS) -> $RET { $$$BODY }'
    capture: [NAME, ARGS, RET]
```

That is the input the page opened wanting: the signatures of the public
functions, and not one token of what they do.

The pattern and the list answer different questions, and reading them as one is
the way to get this wrong:

#table(
  columns: 2,
  table.header[The pattern decides][The list decides],
  [Which constructs match at all], [Which parts of each are hashed],
  [`$$$BODY` is why a function _with_ a body matches],
  [leaving `BODY` out is
    why that body is not an input],
)

So dropping `$$$BODY` from the pattern would not narrow the input — it would
stop matching the functions you meant.

A capture renders as `($NAME …)` around the rendering of every node it bound, so
one match of the probe above hashes:

```
($ARGS (parameter (identifier "a") (: ":") (primitive_type "u8"))) ($NAME (identifier "one")) ($RET (primitive_type "u8"))
```

The list is sorted before hashing, because it is the _set_ of parts that matter:
retyping `[RET, NAME, ARGS]` is not an edit, and cannot be, since only two
spellings of one set ever normalise together. A multi capture that bound nothing
renders as a bare `($ARGS)`, distinct from every count above it — so a function
losing its last argument is still an edit.

#callout("warn")[
  A name the pattern does not define is a hard error, and this is the refusal
  the key could not ship without. An undefined name binds nothing, so it would
  render as an empty `($TYPO)` in every match and narrow the probe to whatever
  was left — with every match still present, so `allow_empty: true` would find
  nothing to complain about. The message names what the pattern _does_ capture.
]

An anonymous `$$$` or a `$_` binds nothing in ast-grep and cannot be named at
all; give the pattern a `$$$ARGS` if you want that run to be an input.

= Which languages a build can parse

Every grammar is a compile-time choice, because grammars are not small. Measured
against the release profile, the twenty-seven ast-grep ships come to about
40 MB linked, against an mmz binary of 3.5 MB. The cheapest is `json` at 160 KB;
the dearest is `kotlin` at 5.8 MB.

So each is a cargo feature, and a stock install carries `rust` alone — the one
mmz's own suite exercises, since a default set may only promise what is tested:

```bash
cargo install mmz                              # rust
cargo install mmz --features lang-python,lang-go
cargo install mmz --features lang-all          # every grammar, ~40 MB
```

Releases carry that last one prebuilt as `mmz-full-<target>`, for the people who
download a binary precisely because they did not want to build one. Either way
`mmz --version` reports the count, so a binary can be asked what it parses
without having to be made to fail first.

`lang:` says which to use. It is optional beside a `file:` whose extension mmz
recognises, and required beside a `run:` — a command line implies no language,
and guessing one would parse source as the wrong grammar and hash whatever fell
out.

```yaml
probes:
  expanded-api:
    run: cargo expand --lib
    ast: 'pub fn $NAME($$$ARGS)'
    lang: rust
```

#callout("warn")[
  A manifest naming a language your colleague's mmz was not built with fails for
  them. It fails loudly, naming the flag — but unlike a missing `run:` tool, the
  fix is a rebuild rather than an install. Weigh that before pinning an
  unusual grammar in a manifest other people run.
]

= Every way it refuses

A wrong scope costs time; a wrong probe can lie. So nothing here falls back to
matching nothing — each of these is exit 6, no record written:

- A pattern that matched *no node*, the same refusal an empty `json:` selection
  gets. `allow_empty: true` opts in when no match really is a valid input.
- A pattern the grammar could only recover into an error node. tree-sitter
  error-recovers a _pattern_ as readily as a file, so `pub fn $N(` would
  otherwise compile fine and match nothing — indistinguishable from a correct
  pattern over a file with no match, and waived outright by `allow_empty`.
- A `lang:` this build has no grammar for, naming the feature flag that fixes
  it. A language mmz has no grammar for in any build is a different message,
  because it needs a different answer.
- A `capture:` name the pattern does not define, naming what it does. Checked
  from the compiled pattern rather than from a match, so a source with no
  matches at all is still told about the list — the error you can act on.
- A `run:` or an unrecognised extension with no `lang:` to say what to parse.
- Bytes that are not UTF-8.
- `json:` and `ast:` on one probe, or `lang:` or `capture:` without `ast:`.

Three more are refused at load (exit 4), because the manifest alone settles
them: an empty `capture:` list, which would hash nothing once per match; a name
that could never be a metavariable, such as `$NAME` copied straight out of the
pattern; and the same name listed twice.

A file that _parses_ into a tree holding error nodes is deliberately not
refused: source using syntax newer than the bundled grammar is an ordinary
state, not a corrupt one, and refusing it would break a probe on a language's
next release rather than on anything the project did. The case worth catching is
caught anyway — a half-written file stops matching, and that is already a hard
error.

= When not to reach for it

If the file already has a structured view, use it. A tool that prints its own
configuration as JSON is a `json:` probe, and that is both cheaper and steadier
than a rendering pinned to a grammar version. `ast:` is for the inputs that are
code and have no such view.

= Where to go next

- #link(u("/inputs/"))[Inputs] — scopes, probes, and the `json:` selector.
- #link(u("/manifest/"))[Manifest reference] — every key, generated from the
  schema.
