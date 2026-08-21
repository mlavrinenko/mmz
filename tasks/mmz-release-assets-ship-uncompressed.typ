#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "mmz: release assets ship uncompressed",
  priority: framework("ice", confidence: 0.9, ease: 7.0, impact: 5.0),
  tags: ("build", "efficiency"),
  links: (
    depends-on("mmz-the-release-ships-a-rust-only-binary.typ")[the flavour that
      made the size worth compressing],
    related("mmz-verify-the-lang-all-release-build-on-every-target.typ")[the run
      that proves the archives extract],
  ),
  status: done(
    2026,
    8,
    21,
  )[Archives, not raw binaries: \`.tar.gz\` per unix target, \`.zip\` for Windows, each flat and holding the binary under its plain name beside \`LICENSE-MIT\` — so an extract leaves a runnable \`mmz\` in the current directory, and the licence finally travels with the binary it covers. \`SHA256SUMS\` over the set, because an archive is where "did this download intact" stops being answerable by running the file.

    Measured rather than estimated: the \`full\` flavour goes 45.4 MB -> 5.7 MB, eight to one, well past the three-to-four the estimate assumed — a parse table is a large array of small integers and gzip eats it. The default flavour manages 5.5 MB -> 2.0 MB.

    The rename was the whole cost and the download counts settled it: every asset of every release from v0.1.1 to v0.7.0 sits at 0-1 downloads, there is no install script, and the docs link the releases page rather than any file. Releases up to v0.7.0 keep their raw assets.

    Packaging is one bash step for every runner rather than a unix/windows pair. The Windows half archives through \`7z\` and is unproven until a real run — Git Bash ships GNU tar, which cannot write a zip, so there is no fallback in the step. That is the sibling verify task, not this one.],
)

== Summary

Every release asset is a raw binary. That was defensible while the only one was
about 2 MB; the `full` flavour lands near 45 MB, and there are five targets of
it. A tree-sitter parse table is a large const array of small integers, which
is close to the best case for a general-purpose compressor.

== The cost, which is the whole question

Compressing means archiving, and archiving renames the asset. Anything pinned
to `releases/latest/download/mmz-x86_64-unknown-linux-gnu` breaks at the next
release. That is the only reason this was left out of the flavour split.

The download counts settle it: every asset of every release from v0.1.1 to
v0.7.0 sits at 0 or 1, and this repo publishes no install script and documents
no asset name — `www/content/quickstart.typ` links the releases page, nothing
deeper. There is no pinned consumer to break, and the rename gets cheaper never.

== Shape

- `.tar.gz` on unix, `.zip` on Windows, one archive per existing asset, named
  for the binary it holds. Old releases keep their raw assets; the change is
  from v0.8.0 forward.
- The archive holds the plain binary name (`mmz`), so an extract puts a runnable
  file on `PATH` without a rename.
- `SHA256SUMS` alongside, since an archive is the point at which "did this
  download intact" stops being answerable by running the file.
