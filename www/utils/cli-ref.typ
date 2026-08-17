// Renders the CLI reference from the binary's own help text.
//
// mmz's CLI is hand-rolled — a `USAGE` const in src/main.rs, not a clap
// reflection — so `--help` is the only machine-readable surface there is. It is
// also the surface a user actually reads, which makes it the right one to
// generate from: the page cannot claim an action the binary does not advertise,
// and cannot miss one it does.
//
// The parse below is the SAME parse `.just/scripts/check-doc-coverage.sh` and
// `.just/scripts/check-doc-facts.sh` run. Keeping it identical is the point: the
// gate proves that every action and every exit code this page will try to render
// has a hand-written note, so a lookup here can never fail at build time.

#import "ui.typ": capture
#import "cli-notes.typ": cli-notes
#import "exit-code-notes.typ": exit-code-notes

#let HELP = capture("help.txt")

// The `Usage:` block: from the header to the first blank line after it.
#let _section(text, header) = {
  let out = ()
  let inside = false
  for line in text.split("\n") {
    if line.starts-with(header) {
      inside = true
      continue
    }
    if inside {
      if line.trim() == "" { break }
      out.push(line)
    }
  }
  out
}

// Every action the usage block advertises, in the order it lists them. The
// second whitespace token of each line is the action; the filter drops
// continuation lines (the wrapped `--is-fresh` entry's second line begins with
// prose, not an action).
#let actions = {
  let out = ()
  for line in _section(HELP, "Usage:") {
    let words = line.split(" ").filter(w => w != "")
    if words.len() >= 2 and words.at(0) == "mmz" {
      let token = words.at(1)
      if token.starts-with("--") or token.starts-with("<") {
        if token not in out { out.push(token) }
      }
    }
  }
  out
}

// Every exit code the help documents, low to high. The block is laid out in two
// columns, so a code is any 1-3 digit run followed by the column gap — the same
// regex the gate uses, so the two cannot disagree about what counts as a code.
#let exit-codes = {
  let seen = ()
  for line in _section(HELP, "Exit codes:") {
    for m in line.matches(regex("\\b(\\d{1,3})\\s\\s")) {
      let code = m.captures.at(0)
      if code not in seen { seen.push(code) }
    }
  }
  seen.sorted(key: c => int(c))
}

// The action summary table: what a reader scans before reading any prose.
#let action-table = table(
  columns: 2,
  table.header([Action], [What it does]),
  ..actions.map(a => (raw(a), cli-notes.at(a).summary)).flatten(),
)

// One action's own section: its summary, then whatever detail the note carries.
#let action-prose(action) = {
  let note = cli-notes.at(action)
  heading(level: 2, raw("mmz " + action))
  par(note.summary)
  if "detail" in note { note.detail }
}

// The `--status=json` schema, for the rule-state vocabulary. Rendered rather
// than restated: the enum and its description are both authored in
// src/status.rs, and a state added there (`missing-output` was) reaches this
// page with no edit.
#let STATUS_SCHEMA = json("../generated/status-schema.json")
#let states = STATUS_SCHEMA.properties.rules.items.properties.state

#let exit-code-table = table(
  columns: 2,
  table.header([Code], [Meaning]),
  ..exit-codes
    .map(c => (
      raw(c),
      {
        let note = exit-code-notes.at(c)
        note.meaning
        if "detail" in note { note.detail }
      },
    ))
    .flatten(),
)
