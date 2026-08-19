// Hand-written prose for every action `mmz --help` advertises. The CLI
// reference renders the help text verbatim from the binary AND these notes
// beside it; `just check-doc-coverage` asserts the two sets match exactly, in
// both directions, so a new flag cannot ship undocumented and a removed one
// cannot leave a paragraph describing it behind.
//
// Keyed by the action token exactly as the `Usage:` block spells it — that is
// the string the gate parses out of `--help`, and the string a reader sees.
//
// `summary` is one line, for the reference table. `detail` is optional and
// carries whatever a reader has to know that the help text has no room for:
// the exit contract, the failure mode, the reason a flag exists at all.

#let cli-notes = (
  "<command>": (
    summary: [The whole point: run a command, or skip it when nothing it
      depends on has moved.],
    detail: [
      #raw("mmz") receives a clean argv vector, not a shell-expanded string, so
      matching is on whole tokens and there is no nesting blind spot. Other
      wrappers go outside it (`chronic mmz cargo test`). There is no runner
      integration and no `--no-memo`: a bare command without `mmz` is simply
      unmemoized.

      On a hit, nothing runs and the exit code is 0. On a miss the command runs
      with its stdout and stderr streamed through untouched, and a record is
      written only if it exits 0 — a failing command never becomes a claim that
      the work is done.
    ],
  ),
  "--": (
    summary: [Force the rest of the line to be the wrapped command, for a
      command whose own name begins with a dash.],
    detail: [
      Without it, a leading `-…` token is read as an mmz option and rejected.
      The separator is consumed, so the rule matched is the command after it.
    ],
  ),
  "--init": (
    summary: [Scaffold `.mmz/config.yaml` and `.mmz/.gitignore` in the current
      directory.],
    detail: [
      Refuses to overwrite an existing manifest (exit 2). Everything mmz needs
      lives under the one `.mmz/` directory: the tracked config, plus a
      `.gitignore` that ignores the cache — so a project gains one entry and its
      root `.gitignore` stays untouched.
    ],
  ),
  "--status": (
    summary: [Show every rule's freshness and the age of its record, as a
      table.],
    detail: [
      Read-only: it resolves inputs and compares digests, and runs no wrapped
      command. `--tag`/`-t` narrows it to rules carrying every listed tag.
    ],
  ),
  "--status=json": (
    summary: [The same report as JSON, with every resolved input, its hash, and
      what the cached record saw.],
    detail: [
      This is the form to reach for when a rule is stale and the question is
      _which input moved_: the resolved set and the recorded set are both in
      there, so a diff is a `jq` away.
    ],
  ),
  "--status=json-schema": (
    summary: [Print the JSON Schema for `--status=json`, so a consumer can
      validate what it parses.],
  ),
  "--is-fresh": (
    summary: [Assert freshness without running anything: exit 0 when fresh,
      exit 1 when not.],
    detail: [
      The inverse of wrapping. `mmz <command>` runs a stale command;
      `mmz --is-fresh -- <command>` refuses it. With no command it gates every
      rule at once; with `--tag` it gates the tagged subset. A non-fresh gate
      names each offender and why, then prints one hint to re-run the listed
      commands under mmz — a standalone run is not observed, so it leaves the
      rule exactly as stale as it found it.
    ],
  ),
  "--prune": (
    summary: [Delete cache records whose rule no longer exists in the
      manifest.],
    detail: [
      Renaming or removing a rule orphans its record; nothing else collects
      them, because mmz never deletes state it cannot prove is unclaimed.
    ],
  ),
  "--schema": (
    summary: [Print the JSON Schema for `.mmz/config.yaml`.],
    detail: [
      The same bytes the manifest reference on this site is generated from, and
      the same schema the `$schema` line `--init` writes points at.
    ],
  ),
  "--schema=fragment": (
    summary: [Print the JSON Schema for a file named in a manifest's
      `imports:` list.],
    detail: [
      Narrower than `--schema`: a fragment may not set `cache_dir`,
      `gitignore`, `strict` or `on_hit`, so validating it against the config
      schema instead would accept documents mmz rejects. Point a fragment's
      own `# yaml-language-server: $schema=…` line here.
    ],
  ),
  "--dump-config": (
    summary: [Print the manifest mmz actually assembled — its effective
      policy plus every scope, probe and command — with the source file
      behind each one.],
    detail: [
      Leads with the source list in load order, so the import graph is
      visible before the entries it fed are, then the effective `gitignore`,
      `cache_dir`, `strict` and `on_hit` (defaulted ones marked, since they
      can only come from the root manifest), then each scope, probe and
      command annotated with its own file. Read-only, and prints the merged
      model only *after* validation — it is not a debugging aid for a
      manifest that fails to merge; that error already names both files.
    ],
  ),
  "--dump-config=json": (
    summary: [The same dump as JSON: a `policy` object plus a `source` on
      every scope, probe and command.],
    detail: [
      Aimed at a gate hook: a generator that emits a fragment can assert the
      fragment it wrote is the one actually in effect, not merely present on
      disk.
    ],
  ),
  "--version": (summary: [Print the version.]),
  "--help": (summary: [Print the usage text, including the exit-code table.]),
)
