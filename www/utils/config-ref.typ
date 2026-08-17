// Renders the manifest reference from the config JSON Schema.
//
// `mmz --schema` is the schema's only author: it is embedded in the binary, it
// is what `.mmz/config.yaml`'s `$schema` line points at, and it already carries
// a type, a default and a prose description for every key. So the reference
// page STATES those by reading them, and adds only what the schema cannot say —
// the hand-written note from config-notes.typ, which `just check-doc-facts`
// requires for every key the schema declares.
//
// The key namespacing here (`commands[].name`, `probes[].run`,
// `scopes[].globs`) is the contract the gate enforces; it is spelled once, in
// `entries` below, and both the page and the gate read the same shape.

#import "ui.typ": capture
#import "config-notes.typ": config-notes

#let SCHEMA = json("../generated/config-schema.json")

// One reference entry: the key as a reader writes it, plus everything known
// about it from either side.
#let _entry(key, prop) = (
  key: key,
  type: prop.at("type", default: "—"),
  default: prop.at("default", default: none),
  description: prop.at("description", default: ""),
  note: config-notes.at(key),
)

// The object-form scope's own properties, reached through the `oneOf` branch
// that is an object. A scope written as a bare array has no properties of its
// own, so only the object branch contributes keys.
// Parenthesised deliberately: in Typst code mode a newline ENDS an expression,
// so a chain broken across lines with leading dots binds only its first term —
// here that silently made `_scope-object` the whole schema, and the object's key
// namespacing came out as `scopes[].scopes`. The parens keep the chain one
// expression.
#let _scope-object = (
  SCHEMA
    .properties
    .scopes
    .additionalProperties
    .oneOf
    .filter(b => b.at("type", default: "") == "object")
    .at(0)
)

// Every documented key, grouped the way the page presents them. Order within a
// group follows the schema's own key order, which is the order the manifest is
// written in.
#let groups = (
  (
    title: "Top level",
    anchor: "top-level",
    lede: [The seven keys a manifest can declare.],
    entries: SCHEMA
      .properties
      .keys()
      .map(k => _entry(k, SCHEMA.properties.at(k))),
  ),
  (
    title: "A command rule",
    anchor: "commands",
    lede: [Each entry of the `commands:` array.],
    entries: {
      let props = SCHEMA.properties.commands.items.properties
      props.keys().map(k => _entry("commands[]." + k, props.at(k)))
    },
  ),
  (
    title: "A probe",
    anchor: "probes",
    lede: [Each entry of the `probes:` map.],
    entries: {
      let props = SCHEMA.properties.probes.additionalProperties.properties
      props.keys().map(k => _entry("probes[]." + k, props.at(k)))
    },
  ),
  (
    title: "A scope, object form",
    anchor: "scope-object",
    lede: [A scope spelled as an object rather than a bare array of patterns.],
    entries: {
      let props = _scope-object.properties
      props.keys().map(k => _entry("scopes[]." + k, props.at(k)))
    },
  ),
)

// A default, rendered as YAML would spell it. `none` means the schema declares
// no default, which is not the same as a default of null: it means the key is
// simply absent unless written.
#let _default(value) = {
  if value == none { [—] } else if type(value) == bool {
    raw(if value { "true" } else { "false" })
  } else if type(value) == str { raw("\"" + value + "\"") } else {
    raw(repr(value))
  }
}

// The summary table for one group: what a reader scans before reading prose.
#let summary-table(group) = table(
  columns: 3,
  table.header([Key], [Type], [Default]),
  ..group
    .entries
    .map(e => (raw(e.key), raw(e.type), _default(e.default)))
    .flatten(),
)

// The prose for one key: the schema's own description, then the note. Two
// voices on purpose — the first is the contract the binary enforces, the second
// is the judgement a reader needs to apply it.
#let entry-prose(entry) = {
  heading(level: 2, raw(entry.key))
  par(entry.description)
  entry.note
}
