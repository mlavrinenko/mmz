#import "../utils/page.typ": page
#import "../utils/ui.typ": callout
#import "../utils/site.typ": u

#let meta = (
  route: "/matching/",
  label: "Matching",
  title: "Matching and parametric rules",
  summary: "How an invocation is matched to a rule, how the cache identity follows from that, and how one rule can fan over a scope's files.",
)
#metadata(meta) <page-meta>

#show: page.with(..meta)

= Token-prefix matching

A rule's `name` is split on whitespace into tokens. It matches when those tokens
are a leading slice of the invoked argv:

#table(
  columns: 3,
  table.header([Rule], [Invocation], [Matches?]),
  [`cargo test`], [`cargo test`], [yes],
  [`cargo test`], [`cargo test --workspace`], [yes — prefix],
  [`cargo test`], [`cargo build`], [no],
  [`cargo test`], [`cargo`], [no — not a full prefix],
  [`cargo`], [`cargo test`], [yes — a broader rule],
)

Matching is on whole tokens, so `car` does not match `cargo`. And `mmz` receives
a real argv vector rather than a shell-expanded string, which is why this stays
robust: no quoting to second-guess, no nesting blind spot.

= Order decides

Rules are tried in manifest order and the first match wins. Order specific rules
before general ones:

```yaml
commands:
  - name: cargo test --doc     # narrower: must come first
    inputs: [rust, docs]
  - name: cargo test
    inputs: [rust]
```

Reversed, the second rule would never be reachable — `cargo test` matches
everything the narrower rule would have.

= Identity follows the rule

The cache identity is the matched rule's `name`, not the argv. `cargo test` and
`cargo test --workspace` therefore share one record when one rule matches both.

That is the granularity knob. If two invocations that match one rule genuinely
depend on different things, they should not share a record — split the rule, or
narrow its matcher.

== `match: exact`

```yaml
commands:
  - name: cargo test
    match: exact
    inputs: [rust]
```

Now only the bare `cargo test` matches; `cargo test --release` falls through to
the next rule, or to the `no_match` case.

#callout("note")[
  `exact` only ever _narrows_ a rule, so it can never cause a wrongful skip. An
  invocation it no longer matches is an error under the default `strict`, or runs
  unmemoized when that case is relaxed — never a silent hit on the wrong record.
]

= Parametric rules

A single `{scope}` macro in a rule's `name` fans it over that scope's files: one
per-file cache record per matched file, without hand-listing each as its own
rule.

```yaml
scopes:
  lint-targets: ["src/**/*.rs"]
  rust-pins:    ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml"]

commands:
  - name: "ruff check {lint-targets}"   # {scope} macro ⇒ parametric rule
    inputs: [rust-pins]                 # shared pins added to every record
```

`ruff check {lint-targets}` stands for `ruff check src/a.rs`,
`ruff check src/b.rs`, … — each a distinct cache identity. A record's inputs are
the `inputs` pins plus its own bound file, so editing `src/a.rs` busts only that
file's record.

The macro is one whitespace token but may sit inside one
(`--file={lint-targets}`). `mmz` stays a prefix — you drive the loop:

```bash
for f in src/**/*.rs; do mmz ruff check "$f"; done
```

The bound file must be a member of the scope, gitignore-filtered like any other
resolution, so an off-list path falls through to the no-match case rather than
inventing a record. Two rules resolving to the same expanded identity is an
error, not a silently picked winner.

== How the other commands see a fan

- `--status` enumerates one row per expanded file.
- `--is-fresh` gates one verdict per expansion. A bare `--is-fresh` over a
  parametric rule passes only when _every_ expansion is fresh; a targeted
  `--is-fresh -- <command> <file>` gates the one expansion `<file>` binds to.
- `--prune` drops a record once its file leaves the tree.

== When the fan is honest

#callout("warn")[
  Per-file scoping is only honest when the command genuinely depends on that one
  file plus the pins.
]

A per-file lint, formatter or typechecker qualifies. A whole-crate command like
`cargo mutants -f {scope}` does not: it compiles the file's siblings, so a
sibling edit can leave a file's record wrongly fresh. Use the fan there knowing
you are trading correctness for speed — and that
#link(u("/concepts/"))[the asymmetry] says which way that trade cuts.
