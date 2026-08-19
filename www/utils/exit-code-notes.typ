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
      read or could not validate.
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
    meaning: [A probe failed, could not be spawned, or printed nothing; nothing
      was recorded.],
    detail: [
      The probe is named, with its exit code and stderr. A failed probe never
      reaches the hasher, so no digest is computed from partial output.
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
