#import "../utils/page.typ": page
#import "../utils/ui.typ": callout, listing, schema-url, transcript
#import "../utils/site.typ": PKG_VERSION, u

#let meta = (
  route: "/quickstart/",
  label: "Quickstart",
  title: "Quickstart",
  summary: "Install mmz, scaffold a manifest, and watch a command skip itself.",
)
#metadata(meta) <page-meta>

#show: page.with(..meta)

= Install

```bash
cargo install mmz
```

Or download a pre-built binary from the
#link("https://github.com/mlavrinenko/mmz/releases/latest")[latest release]. With
Nix, `nix run github:mlavrinenko/mmz` runs it without installing anything.

Check what you got:

#transcript("version.txt")

The count in the brackets is the languages this build can parse with an
#link(u("/code/"))[`ast:` probe]. A grammar is not small, so which ones a binary
carries is settled when it is built, and each release publishes two:

#table(
  columns: 2,
  table.header([Build], [Parses]),
  [`mmz-<target>` — also what `cargo install mmz` and a bare `nix run` give
    you],
  [Rust, the one grammar the suite exercises],

  [`mmz-full-<target>` — also `nix build github:mlavrinenko/mmz#full`],
  [every
    language mmz has a grammar for, at about eight times the size],
)

Anything in between is a `cargo install mmz --features lang-python,lang-go`
away, and a probe naming a language your build lacks fails saying so, naming
the flag. Nothing is ever parsed as the wrong grammar to cover for it.

= Scaffold a manifest

Run `mmz --init` in the root of the project you want to memoize:

#transcript("init.txt")

Two files, both under one directory. The manifest is tracked; the `.gitignore`
beside it ignores the cache, so your project gains one entry and its root
`.gitignore` stays untouched.

#listing("init-config.yaml", lang: "yaml")

The `$schema` line is pinned to the `v#PKG_VERSION` tag — the version of mmz
that wrote it, not `main` — so a project keeps validating against the schema its
mmz was built for even when two projects pin different versions. Editors that
honour `yaml-language-server` will complete and validate the file as you type.

= Declare a rule

A rule is a command plus the inputs it depends on. Inputs are named scopes of
globs, declared once and referenced by as many rules as need them:

```yaml
scopes:
  rust: ["**/*.rs", "Cargo.toml", "Cargo.lock", "rust-toolchain.toml"]

commands:
  - name: cargo test
    inputs: [rust]
```

`name` is both the matcher and the cache identity. It matches by token prefix,
so this rule covers `cargo test` and `cargo test --workspace` alike, but not
`cargo build`.

#callout("warn")[
  The one thing to get right: a rule's scopes must cover every file any matching
  invocation could depend on. Too broad costs a re-run; too narrow skips work
  that needed doing. When in doubt, widen the scope.
]

= Run it twice

Wrap the command wherever memoization is wanted — a Justfile recipe line, a
shell, a git hook. The first run does the work:

#transcript("run-cold.txt")

The second does not:

#transcript("run-warm.txt")

Nothing ran. `mmz` resolved the rule's inputs, hashed them, found the digest
identical to the one the last successful run recorded, printed the `on_hit`
note, and exited 0.

Touch any file in the `data` or `bin` scopes — or upgrade `jq`, which the
manifest's probe pins — and the next run does the work again.

= See what it is standing on

#transcript("status.txt")

`--status` is read-only: it resolves and compares, and runs nothing. When a rule
is stale and you want to know _which_ input moved, `--status=json` carries every
resolved input, its hash, and what the record saw.

= Next

- #link(u("/concepts/"))[Concepts] — the model, and the correctness contract you
  are taking on.
- #link(u("/inputs/"))[Inputs] — scopes, the gitignore filter, and probes for
  the inputs that are not files.
- #link(u("/gating/"))[Gating with tags] — using `--is-fresh` to require that a
  check already passed.
- #link(u("/manifest/"))[Manifest reference] — every key, generated from the
  schema.
