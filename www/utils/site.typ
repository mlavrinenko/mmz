// Site-wide constants for the mmz docs. What lives here is exactly what no
// single page owns: the site URL and its path prefix, the repo link, and the
// nav tree's group names and reading order.
//
// What deliberately does NOT live here: any page's label, title or summary.
// Those belong to the page, which carries them in its own `<page-meta>` block;
// www/generate-site-pages.sh reads all of them back into PAGES below. A second
// copy here would be a place for a page to disagree with itself.

#let REPO = "https://github.com/mlavrinenko/mmz"

// The crate version, read from Cargo.toml through the generated crate map
// rather than hand-typed: `mmz --init` pins a schema URL to the `v<version>`
// tag of the mmz that wrote it, so the docs quote a version constantly and a
// bump must propagate without a human reconciling it.
//
// Not read from Cargo.toml directly: the site builds under `--root www`, and
// Typst refuses a read that escapes the root. www/generate.sh writes this
// artifact before anything compiles a page.
#let PKG_VERSION = (
  json("../generated/crate-map.json").at("package").at("version")
)

// The site's public URL, read out of the tola config instead of retyped beside
// it: "keep in sync with tola.toml" is a comment, not a mechanism, and this
// string is what every absolute link into the site is built from (see
// docs/src/lib.typ's `www-link`, via www/generated/site-pages.json).
//
// Typst resolves a relative path against the FILE, not the root, so this one
// read works under both roots this module is evaluated with — `--root www` for
// the site build and for www/generate-site-pages.sh's page queries, `--root .`
// for that same script's site-meta driver. Trimming the trailing slash is what
// lets `SITE_URL + route` compose cleanly for a route that already opens with
// one.
#let TOLA = toml("../tola.toml")
#let SITE_URL = TOLA.site.info.url.trim("/", at: end)

// GitHub Pages serves this as a project page, under the path component of that
// URL ("/mmz") — derived here rather than hand-typed a second time. u() builds
// internal page links; assets stay source-relative (/assets/...), since tola
// applies the prefix to those itself on build.
#let _url-path(url) = {
  let parts = url.split("://").last().split("/")
  if parts.len() < 2 { "" } else { "/" + parts.slice(1).join("/") }
}
#let PREFIX = _url-path(SITE_URL)
#let u(path) = PREFIX + path

#let SITE = (
  title: "mmz",
  tagline: "Memoized command runner. One question per run: is this work still done?",
)

// Grouped sidebar: group title, and its members' reading order as ROUTES. The
// one fact no single page owns — spreading it across twelve files as `order:`
// integers would make the sidebar's shape invisible and let two pages claim
// one slot.
#let NAV = (
  (title: "Start", items: ("/", "/quickstart/")),
  (
    title: "Guide",
    items: (
      "/concepts/",
      "/inputs/",
      "/code/",
      "/matching/",
      "/outputs/",
      "/gating/",
      "/composition/",
    ),
  ),
  (title: "Reference", items: ("/manifest/", "/cli/", "/library/")),
  (title: "More", items: ("/agents/", "/comparison/")),
)

// The page manifest — route -> {label, title, summary, home?} — derived from
// every page's own `<page-meta>` block by www/generate-site-pages.sh, never
// hand-written here. Empty only during that script's bootstrap pass (see its
// header): compiling a page imports layout.typ -> this file -> this artifact,
// so the artifact has to exist, even empty, before the query that fills it.
#let PAGES = json("../generated/site-pages.json").pages

// Resolve one page by route. Three outcomes, not two:
//
//   - PAGES is EMPTY (the bootstrap pass, before a single page has been
//     queried): return `none`. Panicking here would make the bootstrap pass
//     itself un-compilable, which is the chicken-and-egg
//     www/generate-site-pages.sh exists to break.
//   - PAGES is non-empty and has the route: return its entry.
//   - PAGES is non-empty and lacks the route: panic naming it. A route is a
//     string typed by hand in a page header, in NAV above, or in doc prose;
//     once the manifest is real an unknown one is always a mistake, never a
//     pass still warming up.
#let page-meta(route) = {
  if PAGES.len() == 0 { return none }
  if type(route) == str and route in PAGES { return PAGES.at(route) }
  panic(
    "unknown docs-site route: "
      + repr(route)
      + " — known routes: "
      + PAGES.keys().join(", "),
  )
}
