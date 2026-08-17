#import "../utils/page.typ": page
#import "../utils/ui.typ": callout
#import "../utils/site.typ": PKG_VERSION, u

#let meta = (
  route: "/library/",
  label: "Rust library",
  title: "Rust library",
  summary: "mmz ships as a library crate as well as a binary; the binary is a thin wrapper over it.",
)
#metadata(meta) <page-meta>

#show: page.with(..meta)

= Why a library

The binary is argument parsing, logging setup, and a mapping from library errors
to exit codes. Everything else — manifest loading, glob resolution, hashing,
probe execution, record I/O — is the library, so a tool that wants memoization as
a step rather than as a prefix does not have to shell out to get it.

#raw("mmz = \"" + PKG_VERSION + "\"", lang: "toml", block: true)

= Memoize one invocation

```rust
let argv = vec!["cargo".to_owned(), "test".to_owned()];
std::process::exit(mmz::run(&argv, std::path::Path::new("."))?.into());
```

`mmz::run` does what the prefix does: find the nearest manifest above `cwd`,
match `argv` to a rule, resolve and hash its inputs, and either return the
recorded exit code without running anything or run the command and record the
result on success.

= The rest of the surface

#table(
  columns: 2,
  table.header([Item], [What it is for]),
  [`mmz::run`], [Memoize one invocation; returns its exit code.],
  [`mmz::status`], [The freshness report `--status` renders, as data.],
  [`mmz::prune`], [Sweep records whose rule no longer exists.],
  [`mmz::freshness`], [The `--is-fresh` verdicts, one per rule or expansion.],
  [`mmz::Manifest`], [Load and validate a manifest without running anything.],
  [`mmz::clock`], [The one "now" a run stamps and renders; `MMZ_NOW` pins it.],
  [`mmz::error::Error`], [The error enum every entry point returns.],
)

#callout("note")[
  Errors are a `thiserror` enum, not opaque strings, so a caller can match on the
  case it cares about — a missing manifest, a probe failure, a strict refusal —
  rather than parsing a message. The binary's own exit-code mapping is exactly
  such a match; see #link(u("/cli/"))[the exit codes].
]

= What the library does not do

The same boundaries the CLI has, for the same reasons: no orchestration, no
output replay, no dependency tracing, no remote cache. `mmz::run` runs one
command and answers one question about it.

Full API documentation is on
#link("https://docs.rs/mmz")[docs.rs].
