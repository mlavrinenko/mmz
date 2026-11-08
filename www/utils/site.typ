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

// The sidebar's and README's group titles, in reading order. Membership and
// order within a group live on the page: every `<page-meta>` declares `group`
// and `order`, and www/generate-site-pages.sh refuses a page whose group is
// not here or whose order collides with a sibling's. Adding a page therefore
// edits the page, not a separate list.
#let GROUPS = ("Start here", "Writing a manifest", "Reference", "More")

// The page manifest — route -> {label, title, summary, group, order, home?} —
// derived from every page's own `<page-meta>` block by
// www/generate-site-pages.sh, never hand-written here. Empty only during that
// script's bootstrap pass (see its header): compiling a page imports
// layout.typ -> this file -> this artifact, so the artifact has to exist, even
// empty, before the query that fills it.
#let PAGES = json("../generated/site-pages.json").pages
