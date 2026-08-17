#import "../utils/page.typ": page
#import "../utils/ui.typ": callout
#import "../utils/site.typ": u

#let meta = (
  route: "/comparison/",
  label: "Comparison",
  title: "Comparison",
  summary: "Where mmz sits among build systems, task runners and compiler caches — and which of them you should reach for instead.",
)
#metadata(meta) <page-meta>

#show: page.with(..meta)

// The heatmap rubric is deliberately uniform, mmz included: green = has the
// capability, amber = partial or indirect, grey = none. A column that scored
// itself green on a row it does not really own would make the whole table
// unreadable as evidence.
#let cap(level, body) = html.elem(
  "span",
  attrs: (class: "cap cap-" + level),
  body,
)
#let yes(body) = cap("green", body)
#let part(body) = cap("amber", body)
#let no(body) = cap("grey", body)

`mmz` is not a build system, and most of what follows is about which tool you
should be using instead of — or alongside — it.

#html.elem(
  "div",
  attrs: (class: "matrix"),
  table(
    columns: 6,
    table.header(
      [],
      [mmz],
      [make / ninja],
      [Bazel / Buck],
      [Turborepo / Nx],
      [sccache / ccache],
    ),

    [Unit of work],
    [a command you name],
    [a file target],
    [a declared action],
    [a package task],
    [a compiler invocation],

    [Declares deps],
    [you do, as globs],
    [you do, per target],
    [you do, exhaustively],
    [inferred from the graph],
    [the compiler does],

    [Ordering], no[none], yes[graph], yes[graph], yes[graph], no[none],
    [Replays output],
    no[exit code only],
    no[—],
    yes[full artifacts],
    yes[logs + artifacts],
    yes[object files],

    [Remote cache],
    no[never],
    no[—],
    yes[first-class],
    yes[first-class],
    yes[optional],

    [Language-aware],
    no[none],
    no[none],
    yes[rulesets],
    part[JS-centric],
    yes[C/C++/Rust],

    [Setup cost],
    yes[one YAML file],
    part[a Makefile],
    no[a build system
      migration],
    part[a workspace config],
    yes[a wrapper binary],

    [Wrong-answer risk],
    part[an under-declared scope],
    part[a missing
      prerequisite],
    yes[sandboxed],
    part[an inferred edge],
    yes[hash of the
      real inputs],
  ),
)

= When to use something else

== You need ordering

If the answer to "what runs first?" is anything but "my task runner already
knows", `mmz` is the wrong tool. It has no graph and will not grow one — that is
`make`, `ninja`, `just`, or your CI's own dependency syntax. `mmz` goes _on_ one
of those, one line at a time.

== You need the artifacts back

`mmz` caches an exit code, never output. A hit on `cargo build` means the build
was not re-run — it does not conjure `target/` back into existence. If you want a
cache that restores artifacts across machines, that is Bazel, Turborepo, or a
language-specific cache, and it costs the modelling those tools require.

Declared #link(u("/outputs/"))[outputs] are the deliberate half-measure here:
`mmz` checks that an artifact still exists, so a `cargo clean` voids the record
instead of leaving it fresh. It never stores or restores one.

== You want compilation caching

`sccache` and `ccache` operate one compiler invocation below where `mmz` sits,
and they know how to hash a compiler's real inputs because they are
language-aware. The two compose fine: `mmz` skips the whole `cargo test`, and
`sccache` speeds up the runs it does not skip.

= What mmz is actually for

The gap those tools leave: a project-level check that is expensive, that you run
constantly, and whose real dependency is a set of files you can name.

- A test suite, linter or formatter check on a Justfile line.
- An expensive verification a pre-push hook wants to _require_ rather than run —
  see #link(u("/gating/"))[Gating with tags].
- A slow generation step whose artifact must still be there —
  see #link(u("/outputs/"))[declared outputs].

In every case the value comes from the same trade: you declare the inputs by
hand, in one readable file, and get memoization without adopting a build system.

#callout("note")[
  The trade cuts both ways, and it is the whole risk surface. `mmz` cannot see a
  dependency you did not declare, and a wrongly-fresh rule is silent. A tool that
  sandboxes or traces its inputs makes that impossible instead — at a setup cost
  `mmz` exists precisely to avoid. Choose accordingly, and read
  #link(u("/concepts/"))[the correctness contract] before you do.
]
