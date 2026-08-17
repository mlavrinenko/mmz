# www/crate-map.jq — the jq program www/generate-facts.sh runs over `cargo
# metadata --format-version 1` to build crate-map.json. Kept as `jq -f` rather
# than inline so the shell script stays under its linecop cap and so the
# derivation itself is reviewable on its own.
#
# Two things come out of here, and only two:
#
#   - `package`: the root package's own manifest facts (name, version, edition,
#     rust-version, description, license, repository, keywords, categories).
#     `version` is the load-bearing one: `mmz --init` pins the `$schema` URL it
#     writes to the `v<version>` tag of the mmz that wrote it, so the docs quote
#     a version on the quickstart page, in the manifest reference and in the
#     README snippet. Every one of those reads this field.
#   - `deps`: the runtime dependency graph, name -> resolved version. The
#     architecture prose in AGENTS.md names a handful of these by hand — which
#     ones is an editorial judgement with no machine source — but never their
#     VERSIONS, which are read from here. A dep that leaves the graph resolves
#     to a missing key and fails the render naming it, rather than silently
#     leaving a stale claim behind.
#
# `--argjson` inputs carry what `cargo metadata` cannot see for itself: the
# toolchain channel (rust-toolchain.toml) and the linecop caps are separate
# artifacts, so nothing but the resolve graph is read here.

.packages as $pkgs
| .resolve.root as $root
| ($pkgs | map(select(.id == $root)) | .[0]) as $pkg
# Dev-dependencies are deliberately excluded: they are the test harness, not the
# shipped artifact, and no doc names one. `kind: null` is cargo's spelling for a
# normal dependency (a build dep reports "build", a dev dep "dev").
| ($pkg.dependencies | map(select(.kind == null)) | map(.name)) as $direct
| {
    package: {
      name: $pkg.name,
      version: $pkg.version,
      edition: $pkg.edition,
      rust_version: $pkg.rust_version,
      description: $pkg.description,
      license: $pkg.license,
      repository: $pkg.repository,
      keywords: $pkg.keywords,
      categories: $pkg.categories,
    },
    channel: $channel,
    # Resolved versions for the direct runtime deps only. The name is the key so
    # a doc reads `deps.blake3` rather than filtering an array by name at every
    # call site; a dep that leaves the manifest leaves this dict, and `.at()` on
    # the missing key panics the render instead of rendering a stale version.
    deps: (
      [$pkgs[] | select(.name as $n | $direct | index($n)) | {key: .name, value: .version}]
      | from_entries
    ),
  }
