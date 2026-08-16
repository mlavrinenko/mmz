#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "mmz: per-scope gitignore opt-out for artifact paths",
  priority: framework("ice", confidence: 0.9, ease: 7.0, impact: 7.0),
  tags: ("config", "cache"),
  links: related(
    "mmz-outputs-a-missing-declared-artifact-voids-a-cache-record.typ",
  )[the other half of tracking build artifacts, from the producer side],
  status: proposed(2026, 8, 16),
)

== Summary

A scope gains an optional per-scope `gitignore` override, so one scope can name
build artifacts while every other scope keeps the manifest-level default. The
scope value grows an object form:

```yaml
scopes:
  src:                          # unchanged array form, inherits the default
    - "src/**"
  lcov:
    gitignore: false            # this scope only
    globs:
      - "target/coverage/lcov.info"
```

Absent means inherit the manifest-level `gitignore` (default `true`). No
default changes.

== Why

Build artifacts live in git-ignored paths by definition, and `gitignore: true`
filters them out of glob expansion, so a scope naming one is silently empty and
the rule referencing it is fresh forever. Measured in a throwaway repo with
`target/` in `.gitignore`:

```
scopes: { artifact: ["target/**"] }

# gitignore: true (the default)
artifact appears -> mmz --is-fresh exits 0   # invisible

# gitignore: false
artifact appears -> mmz --is-fresh exits 1   # tracked
```

Flipping the manifest-level key is not a fix for the projects that need this.
MindTape's `.mmz/config.yaml` would then walk all of `target/` and the 2000
generated files under `examples/large/` that its own comments say must never
churn a rule. The choice belongs to the scope that names the artifact, not to
the manifest.

The immediate consumer is MindTape's `check-crap` arm, which today runs
unwrapped because a memoized verdict would outlive the lcov it could not
measure. With this key that arm becomes a plain input scope and needs no
further mmz feature.

== Scope

- `src/manifest.rs`: scope values parse as either an array of globs (today) or
  an object with `globs` plus an optional `gitignore`. Reject an object with no
  `globs`, and an empty `globs` list, the same way a malformed scope is
  rejected today.
- `src/resolve.rs`: the ignore filter becomes per-scope rather than one flag
  read once for the whole walk. A rule mixing ignored and non-ignored scopes
  must resolve both correctly in one pass.
- `schema/mmz.schema.json`: the `oneOf` for the two scope forms. Coupled by the
  `manifest-schema` outdatty group, so it moves in the same commit.
- `README.md`, `www/index.html`: the `cli-docs` group. Document the key and say
  plainly what it is for, including the artifact case.
- `src/init.rs`: leave the starter manifest on the array form. The object form
  is the exception, and a starter that leads with it teaches the wrong default.
- Tests: a scope with `gitignore: false` sees an ignored file; a sibling scope
  in the same rule keeps filtering; the array form is unchanged.

== Not in scope

Changing the manifest-level default, and any per-glob (rather than per-scope)
override. One knob, at the level a reader can see it.

== Home

mmz's own backlog. Filed out of a MindTape discussion about whether just's
`[cache]` attribute could replace mmz. It cannot, but the artifact-tracking
half of it is a real gap, and this is the piece both halves need.
