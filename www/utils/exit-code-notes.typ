// Hand-written prose for every exit code `mmz --help` documents. The CLI
// reference renders this as the exit-code table; `just check-doc-facts` parses
// the codes back out of the binary's own help text and asserts every one has an
// entry here, so a new code cannot ship without a line explaining it.
//
// Keyed by the code as a string, because a Typst dict key is a string and the
// gate compares against text parsed out of `--help`.
//
// `meaning` is the one-line cell. `detail` is optional and carries what the help
// text's cramped two-column layout has no room for — above all, whether mmz
// wrote a record before exiting, which is the difference between "try again" and
// "fix the manifest".

#let exit-code-notes = (
  "0": (
    meaning: [Fresh and skipped, or the wrapped command ran and succeeded.],
    detail: [
      Also the exit for a satisfied `--is-fresh` gate, and for the read-only
      actions (`--status`, `--schema`, `--dump-config`, `--version`, `--help`,
      `--prune`).
    ],
  ),
  "1": (
    meaning: [`--is-fresh`: the targeted rule — or some rule in the gated set —
      is not fresh.],
    detail: [
      Reserved for the gate verdict, never for an error. Every offending rule is
      named on stderr with the reason it would re-run.
    ],
  ),
  "2": (
    meaning: [Usage error: an empty invocation, an unknown option, `--init` over
      an existing manifest, or a `MMZ_NOW` that is not a Unix epoch.],
    detail: [
      The clock case is the one that is not about argv: a malformed pin is
      refused rather than ignored, so the misconfiguration surfaces here instead
      of as a stamp nobody can reproduce.
    ],
  ),
  "3": (
    meaning: [Strict refusal: no rule matched, or a matched rule resolved to zero
      files.],
    detail: [
      Both cases are relaxable per project through `strict`, and then the
      invocation runs unmemoized instead of stopping.
    ],
  ),
  "4": (
    meaning: [The manifest is missing or invalid.],
    detail: [
      Never relaxable. `mmz` will not memoize against a manifest it could not
      read or could not validate — and the unreadable case names the file,
      because a permission bit on a config is not an internal error. Shape is checked here, at load, even for a
      probe no rule names — a probe declaring both `run:` and `file:` is refused
      before anything runs, rather than resolved by a precedence rule.
    ],
  ),
  "5": (
    meaning: [The command succeeded without producing a declared output; nothing
      was recorded.],
    detail: [
      The missing path is named. Recording the run anyway would leave a rule that
      quietly never hits again.
    ],
  ),
  "6": (
    meaning: [A probe did not produce a value mmz can trust; nothing was
      recorded.],
    detail: [
      The probe is always named. A `run:` line that exits non-zero or cannot be
      spawned comes with its exit code and stderr; a `file:` that is missing or
      unreadable comes with the path; and a `json:` selector reports bytes that
      were not one JSON value, a program that would not compile or that raised,
      or — the case worth the code on its own — a selection that measured
      nothing. An `ast:` pattern lands here the same way: one that matched no
      node, one the grammar could only recover into an error node, bytes that
      are not UTF-8, or a `lang:` this build has no grammar for — which is the
      one failure under this code whose fix is a rebuild, so the message names
      the cargo feature. None of them reach the hasher, so no digest is ever
      computed from partial output.
    ],
  ),
  "7": (
    meaning: [`--is-fresh`: the selection holds no rule, so the gate would have
      asserted nothing.],
    detail: [
      Three ways to reach it: the manifest declares no `commands:`, a `--tag`
      filter no rule carries (a typo, a rename, a rule that quietly lost its
      `tags:` entry), or a selected rule that fans over a scope resolving to no
      files. All three would otherwise exit 0 — a pass over an empty set, which
      reads exactly like a build that ran. The message names the tags and lists
      the ones the manifest does declare. Never relaxable, and distinct from
      `1` on purpose: a hook branching on `$?` can tell a stale build from a
      gate pointed at nothing.
    ],
  ),
  "8": (
    meaning: [A file mmz needed could not be read or written; nothing was
      recorded.],
    detail: [
      The path is always named, rendered the way `--status` renders it. Three
      kinds of file reach this code — an input a rule declared, the scaffold
      `mmz --init` writes, and the cache a run records into or `--prune`
      sweeps — and they share one fact: the filesystem refused. That is a
      condition of the tree rather than a bug in mmz, which is why it is not
      `70`. A manifest that cannot be read is the exception, and lands at `4`
      with the rest of the manifest failures.

      An input the walk resolved and something else removed before the hasher
      opened it says so in as many words, rather than reporting the errno: that
      race is what gating a parallel runner sits in front of by construction,
      and "no such file" alone reads like an accident. Either way no record is
      written and the wrapped command never runs, so re-running once the tree
      has settled is safe.
    ],
  ),
  "70": (
    meaning: [Internal error.],
    detail: [Worth reporting: nothing a manifest can say should produce one.],
  ),
  "127": (
    meaning: [The wrapped command could not be spawned.],
    detail: [
      The conventional shell code for it, so a caller inspecting `$?` reads the
      same number it would have from `sh`.
    ],
  ),
)
