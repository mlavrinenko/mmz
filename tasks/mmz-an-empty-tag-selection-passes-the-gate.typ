#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "mmz: an empty tag selection passes the gate",
  priority: framework("ice", confidence: 0.9, ease: 7.0, impact: 6.0),
  tags: ("gating", "cli"),
  links: (
    related("mmz-the-manifest-does-not-feed-any-cache-digest.typ")[turned up
      while disconfirming that one],
  ),
  status: proposed(2026, 8, 20),
)

== Summary

`mmz --is-fresh --tag <tag>` exits 0 when no rule carries `<tag>`. The gate is
vacuously true over an empty selection, so a typo'd tag, a renamed tag, or a
rule that quietly lost its `tags:` entry reads as a passing build rather than
as a question nobody asked.

== Reproduction

A manifest whose only rule is tagged `gate`, with a pass recorded:

```console
$ mmz --is-fresh --tag gate
$ echo $?
0
$ mmz --is-fresh --tag gats     # one letter off
$ echo $?
0
```

`--status` is worse, because it answers with a sentence that is not true:

```console
$ mmz --status --tag gats
no rules defined in /tmp/x/.mmz/config.yaml
```

Rules are defined. None carries that tag. The message is `status::report`'s
empty-report line, written for a manifest with no `commands:` at all, and it
does not know it is now reporting on a filter.

== Why it matters here specifically

This repo spends that exit code. `.mindtape/config.toml`'s `[[on.flip]]` gates
closing a task on `target/debug/mmz --is-fresh --tag gate`, so the tag being
wrong and the build being green are the same observation from where `mt` is
standing. Every gate rule dropping its `gate` tag — one bad merge of a
`conf.d/` fragment — closes tasks against a build nobody ran. That is the false
green the tool exists to refuse, reached through the door rather than the wall.

It is also out of character. Every other way of naming nothing is loud:
`no_match` errors (exit 3), `no_inputs` errors (exit 3), a probe printing
nothing errors (exit 6). All three are the same shape of mistake — a selector
that resolved to nothing — and only this one is silent.

== Shape of a fix, not yet chosen

Refuse an empty tag selection: a new error (`Error::NoTaggedRules`, naming the
tags and listing the tags the manifest does declare) for `--is-fresh --tag` and
`--status --tag`, with its own exit code and a note in
`www/utils/exit-code-notes.typ`. Untagged `--is-fresh` over a manifest with no
rules at all is the same question and deserves the same answer.

Two things to settle before writing it:

- Whether an empty selection is an error or a distinct exit code. It is a
  misconfiguration, not a stale build, and telling those apart from a hook's
  `||` branch is the whole point of a separate code.
- Whether `--status --tag` should error or just stop claiming "no rules
  defined". A report that renders an empty table is defensible; a report that
  states a falsehood is not.

== Test first

A CLI test under `tests/`, in the shape the suite already uses: record a pass,
gate on a tag no rule carries, and assert the run is refused rather than
exiting 0. The current behaviour is untested in either direction, which is how
it survived.
