// What mmz is for, as a user meets it. Every open task gives each goal a
// verdict in its `x-goals: (product: ...)` field, checked by `check-goals(...)`
// from .mindtape/lib.typ. How this repo builds mmz is in process-goals.typ.

#let goals = (
  no-false-green: (
    title: "Never skip work that should run",
    acid-test: [Nothing the change adds lets mmz skip a command, or report it
      fresh, unless it just confirmed the rule's declared inputs, outputs and
      ttl; whatever it cannot confirm re-runs or fails closed.],
  ),
  exact-inputs: (
    title: "Inputs say exactly what a rule reads",
    acid-test: [A rule can name the one field, AST node, flake input or subtree
      its verdict depends on, so a byte moving anywhere else leaves it fresh,
      and reading a slice of a file on disk spawns no process.],
  ),
  cheap-gates: (
    title: "Asserting a pass costs a hash",
    acid-test: [`mmz --is-fresh --tag <tag>` answers from records and hashes
      alone and runs no wrapped command, so a hook or `mt flip` can demand it;
      a pass earned in a sibling checkout arrives by `mmz --import`, not a
      re-run.],
  ),
  agent-drivable: (
    title: "Callers read state, not prose",
    acid-test: [A script or agent decides its next step from the exit code and
      `--status=json` alone, and every refusal names the rule, file or tag at
      fault plus a command that would fix it.],
  ),
  composable-manifest: (
    title: "A manifest assembles from parts",
    acid-test: [A tool contributes rules as its own fragment under `imports:`
      without rewriting the hand-written manifest, and `--dump-config` and
      `--status` name the file every scope, probe and rule came from.],
  ),
  ready-binaries: (
    title: "A downloaded binary works as advertised",
    acid-test: [Every release target ships a checksummed archive of each
      flavour, run on that target before upload, and `mmz --version` says how
      many languages it parses.],
  ),
  docs-teach-judgement: (
    title: "Docs say when, why and what it costs",
    acid-test: [Every feature page says when to reach for it, what it costs and
      what to use instead, and reads without a hidden scroll at 375px, checked
      in a browser after its last rewrite.],
  ),
)
