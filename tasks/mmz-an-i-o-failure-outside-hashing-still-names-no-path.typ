#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "mmz: an i/o failure outside hashing still names no path",
  priority: framework("ice", confidence: 0.9, ease: 7.0, impact: 5.0),
  tags: ("cli",),
  status: proposed(2026, 8, 21),
  links: related(
    "mmz-an-i-o-error-while-hashing-an-input-names-no-path.typ",
  )[the same defect, closed for inputs only],
)

== Summary

0.8.1 closed the hashing hole: an input mmz cannot read now names the file and
exits 8. The audit that fix asked for found the same bare `?`-on-io shape at
four more sites, all still reporting through `Error::Io`'s pathless
`i/o error: {0}` at exit 70. None of them is the reported failure, and each is
a smaller blast radius than the input path was — which is why they were left
out of the patch rather than folded into it.

== The sites

- `src/compose.rs`, `load` — `fs::read_to_string(path)?` and
  `fs::canonicalize(path)?` on the root manifest. `Error::Io`'s doc records
  this as deliberate ("a missing or unreadable root manifest is not a new
  failure mode this feature adds"), so changing it is a decision to revisit,
  not an oversight to correct. Note the exit code is also wrong by the table's
  own terms: a manifest that cannot be read is `4` ("mmz will not memoize
  against a manifest it could not read"), not `70`.
- `src/compose_policy.rs`, `declared_policy_keys` — the second read of the
  same file for `--dump-config`. Same call, same answer.
- `src/init.rs` — `create_dir_all` and two `write`s. `mmz --init` failing with
  a pathless "permission denied" names neither `.mmz/` nor the file it was
  writing.
- `src/cache.rs`, `prune` — `read_dir` and `remove_file` on the cache
  directory. Lowest value of the four: the directory is the manifest's
  `cache_dir` and a reader has one place to look.

`cache::write` is not on the list: it swallows its error into a warn line that
already names the command, by design.

== Shape of a fix

The same one 0.8.1 used, and `ImportNotReadable` and `ProbeFileUnreadable`
before it: a variant that carries the path, rendered through
`Provenance::shorten` so an error names a file the way `--status` and
`--dump-config` do.

Decide per site whether the exit code moves with the message. The manifest
sites arguably belong at `4` rather than `70`, which is a behaviour change and
wants the exit-code table, a docs line and a changelog entry — the same
treatment `8` got.

== Test first

Each site's test is the same shape as `tests/cli_unreadable_inputs.rs`: make
the target unreadable with a mode the walker still lists, assert the message
names it and the code is the documented one. The root-manifest case can also
be staged with a directory where the file should be.
