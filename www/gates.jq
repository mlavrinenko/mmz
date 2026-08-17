# www/gates.jq — the jq program www/generate-facts.sh runs over `just --dump
# --dump-format json` to build gates.json. Kept as `jq -f` rather than inline
# so the shell script stays under its linecop cap.
#
# Gate membership and row order both come from the Justfile, from two places
# that must agree:
#
#   - every gate recipe carries `[group("gate")]`, and
#   - `check`'s own dependency list names every gate it runs.
#
# Neither alone is enough. Membership from the group tag alone would let a
# recipe advertise itself as a gate while `check` never runs it; membership
# from `check`'s dependencies alone would let a gate be run with no group tag
# to find it by. Disagreement fails HERE, naming both sides, rather than
# downstream as a mysteriously short gate table.
#
# Row order reads `check`'s dependency list because the JSON dump sorts recipes
# alphabetically and carries no source order. That list is also the one place a
# reordering is a deliberate edit rather than a side effect of where a recipe
# happens to sit in the file.
#
# Each gate carries a derived `command`: the recipe's own `.body` when that is a
# single non-shebang line short enough for a table cell, else `null` — and
# docs/src/contributing.typ falls back to the recipe's `[doc(...)]` string. That
# is what makes the Command column impossible to drift: it is read back off the
# Justfile, never retyped beside it.
#
# `.body` is structured (one array per line, of string or `{{ VAR }}` fragments
# — `[["variable","VAR"]]`); render_line renders both as the Justfile's own
# `{{ VAR }}` syntax rather than dumping raw JSON.

def render_frag:
  if type == "string" then .
  else map(if .[0] == "variable" then "{{ " + .[1] + " }}"
            else "{{ " + (.[0] // "expr") + " }}" end) | join("")
  end;
def render_line: map(render_frag) | join("");

# Quotable = no shebang, exactly one body line, that line renders under 160
# chars, and it is not a bare script invocation.
#
# The last clause is the one worth arguing. A gate whose body is nothing but
# `bash .just/scripts/<name>.sh` is a POINTER, not a command: printing it in the
# Command column would tell a reader nothing the Gate column has not already
# said, and it would mean that extracting a long body into a script silently
# rewrote a rendered table. Such a body reports null and falls back to
# `[doc(...)]`. Deliberately narrow: an interpreter word plus one `.sh` path and
# nothing else — a script invoked WITH arguments still says which arguments, so
# it stays quotable.
def is_script_invocation:
  test("^((bash|sh|zsh) +)?[A-Za-z0-9._/-]+[.]sh$");

def gate_command:
  if .shebang or (.body | length) != 1 then null
  else (.body[0] | render_line) as $line
    | if ($line | length) > 160 or ($line | is_script_invocation) then null
      else $line end
  end;

# Every recipe object, top-level and module, flattened. Each recipe carries its
# own fully-qualified `namepath` (just computes it, e.g. a `docs` module's
# `check` recipe reports "docs::check"), so walking `.modules` recursively is
# all that is needed — a gate that moved into a module would otherwise drop out
# of the table silently.
def all_recipes:
  ((.recipes // {}) | to_entries | map(.value))
  + ((.modules // {}) | to_entries | map(.value) | map(all_recipes) | add // []);

# The Typst IDENTIFIER form of an invocation path: `docs::check` -> `docs-check`.
# Every name this program compares or emits is mangled, so a recipe moving into
# a module changes no rendered byte (see www/generate-facts.sh's just.typ
# producer, which mangles identically).
def mangle: gsub("::"; "-");

(all_recipes) as $recipes
| ($recipes | map({key: (.namepath | mangle), value: .}) | from_entries) as $by_mangled
| ($recipes
    | map(select(.attributes // [] | any(.group? == "gate")))
    | map(.namepath | mangle) | sort) as $gate_group
# `check` runs each gate through the `memo` wrapper recipe, so a dependency is
# either `(memo "<gate>")` — the gate name is the argument — or a plain recipe
# name. Both spellings resolve to the gate's own name here.
| ([.recipes.check.dependencies[]
    | if .recipe == "memo" then .arguments[0] else .recipe end
    | mangle]) as $check_deps_ordered
| ($check_deps_ordered | sort) as $check_deps
| if $gate_group != $check_deps then
    # `(none)` rather than an empty join, and a trailing newline rather than
    # none: `halt_error` writes its string verbatim, so without the newline the
    # caller's own next stderr line runs onto the end of this message and reads
    # as one of the listed names.
    (def side: if length == 0 then "(none)" else join(", ") end;
      "gate group and check dependencies disagree:\n"
      + "  only in [group(\"gate\")]: " + (($gate_group - $check_deps) | side) + "\n"
      + "  only in check dependencies: " + (($check_deps - $gate_group) | side) + "\n")
    | halt_error(1)
  else
    ($check_deps_ordered) as $names
    | {
        gates: [$names[] as $n
          | $by_mangled[$n] as $r
          | {name: $n, doc: $r.doc, command: ($r | gate_command)}],
      }
  end
