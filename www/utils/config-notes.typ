// Hand-written prose for every property in the config JSON Schema. The manifest
// reference renders the schema's own type, default and description straight
// from `mmz --schema`, and these notes beside them; `just check-doc-facts`
// asserts the two key sets match exactly, in both directions, so a new manifest
// key cannot ship with a schema description and no prose, and a removed one
// cannot leave prose behind.
//
// Keyed exactly as the reference renders them, so a key's note is findable by
// the name a reader sees: top-level keys bare, a command rule's keys under
// `commands[].`, a probe's under `probes[].`, an object-form scope's under
// `scopes[].`.
//
// These notes deliberately do NOT restate the schema description — that is
// rendered from the schema. What belongs here is what the schema cannot say:
// when to reach for the key, what it costs, and which failure it prevents.

#let config-notes = (
  scopes: [
    Declared once, referenced by many rules, so a shared input path lives in one
    place. Globs follow the common convention: `*` stays within a directory,
    `**` crosses them.

    A scope is the unit of over-declaration, and over-declaring is the safe
    direction — a scope that is too broad costs an unnecessary re-run, while one
    that is too narrow buys a wrongly-fresh rule. When in doubt, widen it.
  ],
  "scopes[].globs": [
    The object form's pattern list, required and non-empty. An object without it
    is a manifest error rather than a scope that quietly matches nothing.
  ],
  "scopes[].gitignore": [
    The per-scope override, and the one thing the object form exists for. Keep it
    at the scope that names the artifact: flipping the manifest-level setting
    instead would drag every sibling scope through `target/` and any other
    ignored tree, which is both slow and a source of spurious cache busts.
  ],
  probes: [
    How a rule depends on something that is not a whole file. Read the trade
    before reaching for one — a wrong scope costs time, a wrong probe can lie —
    and note that mmz validates a probe's exit status, never its meaning.
  ],
  "probes[].run": [
    Executed by `sh -c` from the project root with stdin closed, so a probe
    waiting on input fails instead of hanging a gate. Resolved once per mmz
    invocation however many rules name it.
  ],
  "probes[].allow_empty": [
    Opt in only when empty output is genuinely a valid state. The default exists
    because empty stdout is almost always a selector that matched nothing, and
    that is the cheapest bug in this whole surface to catch.
  ],
  commands: [
    Ordered rules; the first token-prefix match wins, so specific rules go before
    general ones. The cache identity is the matched rule, not the argv, which is
    what makes rule granularity a design decision rather than an accident.
  ],
  "commands[].name": [
    Both the matcher and the cache key. Split a rule or narrow its matcher when
    one rule conflates invocations with genuinely different inputs.
  ],
  "commands[].inputs": [
    Scope names, probe names, or both — one namespace, so a reader never has to
    guess which kind a name is, and an entry that is neither is refused at load.
    A rule whose only input is a probe still has inputs: it is memoized, not
    `no-inputs`.
  ],
  "commands[].outputs": [
    Declare these for a rule that _produces_ something. A verdict command
    (`fmt --check`, a linter) leaves nothing behind and needs none; a producer
    command's record can be undone without touching an input, and this is what
    lets mmz notice.
  ],
  "commands[].match": [
    `exact` only ever narrows a rule, so it cannot cause a wrongful skip: an
    invocation it no longer matches falls through to the no-match case. Reach for
    it when trailing arguments change the real work — `cargo test` and
    `cargo test --release` as separate identities.
  ],
  "commands[].tags": [
    What lets one manifest hold a gating subset alongside memoized commands a
    gate should ignore, instead of splitting the manifest per concern.
  ],
  "commands[].on_hit": [
    Per-rule override of the global note. Set it to the empty string to silence
    one noisy rule without silencing the rest.
  ],
  gitignore: [
    On by default because an input set should never contain build output.
    Explicitly listed literal paths are always kept, `.git` is never traversed,
    and symlinks are not followed.
  ],
  cache_dir: [
    Derived, throwaway state — never commit it. `--init` writes a `.mmz/.gitignore`
    that covers the default location; point this elsewhere and it is on you to
    ignore the new one.
  ],
  strict: [
    The fail-closed switch. Omit the key and mmz errors rather than guessing on
    both cases; relax a case and the matching invocation runs unmemoized instead
    of stopping. Relaxing `no_inputs` is the riskier of the two: a rule that
    resolves to no files is usually a scope that stopped matching, not a rule
    that genuinely has no inputs.
  ],
  on_hit: [
    A skip is invisible unless something says so, and a silent gate is one nobody
    trusts. `{cache:<field>}` pulls a field straight from the record that caused
    the hit, so the note can name what it is standing on.
  ],
)
