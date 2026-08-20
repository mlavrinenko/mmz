<!-- Generated from docs/src/contributing/generated-docs.typ by `just docs md`. Do not edit; edit the source. -->

# Generated documentation

Almost nothing in this repository’s documentation is typed where you read it. `README.md`, `AGENTS.md`, `CONTRIBUTING.md` and this file are rendered from Typst sources; the docs site’s reference pages are generated from the binary’s own schema and help text; every transcript is a real run captured at build time.

This file explains the machinery, and what you owe it when you add to it.

## Why

Every fact restated by hand is a fact that will eventually be wrong, and the failure is silent — a reader trusts documentation precisely when they cannot check it. This repo had three copies of the flag list (the binary, the README, `www/index.html`) and two of the manifest keys.

`outdatty.yaml` couples sources to dependents, so an edit at least _flags_ the copies. But flagging is not deriving: a human still reconciles, and the 500-line README was where that quietly stopped happening.

So: **a doc states a fact by reading it**. If a fact exists in a file the build can read, the doc reads it. What stays hand-written is judgement — why a key exists, when to reach for it, what it costs — which has no machine source and is the only thing worth a reviewer’s attention.

## The two render paths

They share the generated facts and nothing else. Keeping them straight is the one thing that trips people up.

|  | Docs site | Root Markdown |
| --- | --- | --- |
| Sources | `www/content/*.typ` | `docs/src/*.typ` |
| Renderer | tola (HTML) | typlite (Markdown) |
| Entry | `#show: page.with(..meta)` | a `<mmz-md>` metadata block |
| Output | `www/public/mmz/` | the path each source declares |
| Gate | `just docs check` | `just docs md-check` |

A source cannot be both. A content page carries a tola `page` show rule that typlite cannot parse; a docs/src source is plain Typst that tola has no route for. Shared prose belongs in a helper module both can import — `gate-notes.typ` is the working example, imported by the site and by CONTRIBUTING.md’s source.

## The generators

Three scripts, run in a fixed order, all writing to the gitignored `www/generated/`:

1. **`www/generate.sh`** builds `mmz` release and captures its live output: the help text, both JSON Schemas, the version, a `--init` in an empty directory, and a dozen transcripts from running the real binary against a throwaway copy of `examples/demo`.
2. **`www/generate-facts.sh`** derives the rest from files: the crate map from `cargo metadata`, the gate table and the `just` module from `just --dump`, the caps from `.linecop.yaml`.
3. **`www/generate-site-pages.sh`** compiles every content page to read back its own `<page-meta>` block, producing the route → label/title/summary manifest that the sidebar, the README’s page list and every cross-page link resolve against.

The order is not negotiable, and each script’s header says why. In short: `generate.sh` wipes the directory on entry, so the facts must be restored inside the same critical section; and the page manifest is a full page compile, so it needs both the facts and the captures already in place.

A capture must also be the same bytes on every build, and the way to get that is to pin the input rather than to correct the output. `generate.sh` exports `MMZ_NOW`, so a record’s `ran_at` and the ages in a `--status` table are the binary’s own stdout and still identical run to run. Reach for post-processing only when nothing can be pinned instead: a normalized capture is a place where the docs and the binary are allowed to disagree.

The fixture’s location is the one input that cannot be pinned that way. The mutating captures run against a copy of `examples/demo` under `$TMPDIR`, so a command that prints a path — `--status=json` names the manifest — carries that build’s `mktemp` directory verbatim, and `/tmp/tmp.BIg78u4JqB/demo/.mmz/config.yaml` is neither reproducible nor readable. A capture that names it is marked `rel` at its call site in `generate.sh`, which rewrites the copy’s path to a project-relative one; every other byte is stdout as written.

That marking is a convention, and a convention no gate reads is a leak waiting for the next path-printing capture. `.just/scripts/check-capture-paths.sh` runs last inside `generate.sh` and fails the build on any generated file still naming a temp directory — matching both `mktemp`’s own naming and, exactly, the two directories this run created and passed down. It is the one reader in the repo that distrusts a capture’s bytes: `just docs check` builds the site and validates its links, `just docs md-check` diffs Markdown against its source, and neither can see inside stdout, because stdout is the thing being trusted.

All three serialize on `target/www-generated.lock`, because `just check` runs its arms in parallel and more than one of them is inside that directory at once. A caller that already holds the lock exports `MMZ_GENERATED_LOCK=1` so the nested calls skip their own acquisition rather than deadlocking against their own process tree.

## Adding a docs/src source

Two things, and the generator does the rest — it globs `docs/src/**/*.typ` and reads each source’s own declaration, so there is no list to add to:

```typ
#import "../lib.typ": doc-title, fact, just

#metadata((
  output: "docs/contributing/my-topic.md",
)) <mmz-md>

#doc-title[My topic]
```

Then `just docs md` renders it, and `just docs md-check` will fail from then on if the committed output drifts. Add the output path to `.linecop.yaml`’s excludes — a generated Markdown file’s line count is not a meaningful size proxy, because typlite emits one line per paragraph rather than hard-wrapping.

## Adding a content page

```typ
#import "../utils/page.typ": page

#let meta = (
  route: "/my-page/",
  label: "My page",
  title: "My page",
  summary: "One sentence; it becomes the meta description and the lede.",
)
#metadata(meta) <page-meta>

#show: page.with(..meta)
```

One binding feeds both consumers — the metadata block the manifest is built from, and the show rule — so a page cannot disagree with itself about its own route.

Then add the route to `NAV` in `www/utils/site.typ`. `just check-doc-facts` fails if you do not: a page missing from NAV is unreachable in the sidebar, and a NAV entry naming no page is a 404 in it. The route must also match the filename (`my-page.typ` serves `/my-page/`, `index.typ` serves `/`), which is what catches a `<page-meta>` block pointed at some other page’s route.

## Adding a derived fact

Write it into `www/generated/` from `www/generate-facts.sh` through `emit` (JSON) or `emit_raw` (anything else). Both write via a temp file and an atomic rename, and both refuse to install a malformed or empty artifact — a partial fact file is worse than none, because the reader gets a plausible-looking prefix instead of an error.

Then decide whether it needs a coverage gate. The existing ones follow one pattern: derive the key set from the source, read the hand-written notes dict’s keys back through a `typst query` driver, and fail on a set difference in _either_ direction. Both directions matter — a missing note means an undocumented thing, and an extra note means prose describing something that no longer exists, which reads as current and is worse.

Read the notes dict by evaluating it, never by grepping quoted strings out of the file. The keys are Typst code, and a regex cannot tell a key from a word in a comment.

## Working within tola

Two behaviours of the SSG bite whoever edits a generator:

- **`tola serve` caches its file index at startup.** A brand-new `www/generated/` file is invisible to an already-running serve, though pre-existing ones read fine. Restart serve after adding a capture; `just docs check` goes through `tola build`, which re-scans and is unaffected.
- **`tola validate` statically flags an `image("<path>")` string literal** as a broken link when the target is not a copied asset. Everything the generators write is under `generated/`, which is never copied to `public/`, so a build-time image must be inlined via `image(read(path, encoding: none), …)` rather than by path. No such image exists today; the rule is written down before the first one.

## Working within typlite

`docs/src/lib.typ`’s header lists every constraint, verified against the pinned toolchain. The three that actually bite:

- **Headings shift down one level.** `=` renders as `##`. `doc-title` is the only path to a `#`, and it works by emitting an `<h1>` that `docs/generate-md.sh` demotes with an anchored `sed` — attribute-free and line 1 only, so a centered `<h1 align="center">` header survives untouched.
- **Never break a backtick span across a source line.** Typst keeps the newline inside the span and typlite emits it literally, which corrupts a table row mid-cell. Hard-wrap the prose around a code span, never through one.
- **`<p>`, `<div>` and `<span>` are silently stripped**, attributes and all. Nothing appears and nothing errors. `h1`–`h6`, `img`, `br` and `blockquote` do survive.

Never hand-type an absolute docs-site URL in a source. Use `www-link(route)`, which resolves against the generated page manifest and fails the render naming an unknown route — nothing else in this repo resolves a URL sitting in generated Markdown, so a renamed page would otherwise ship a 404 only a reader ever finds.

## What is deliberately not derived

- **The design invariants**: fail-closed, the under/over-declaration asymmetry, existence-not-hashing for outputs. These change with the design, not with a version bump, and no source proves them.
- **Which dependencies are worth naming** in AGENTS.md’s architecture list. That is editorial judgement; only the versions are read.
- **The comparison page’s rubric.** Scoring another tool is an argument, not a fact, and pretending otherwise by deriving it from something would only hide the judgement.

## Related

- [For AI agents](https://mlavrinenko.github.io/mmz/agents/) — the same pipeline’s output, from the reader’s side.
- `docs/contributing/gates.md` — how the gates that run all this are wired.
