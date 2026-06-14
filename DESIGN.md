# mmz design

mmz is a memoized command runner. Prefix any command with `mmz`; when the
command matches a rule and that rule's declared inputs are byte-for-byte
unchanged since the command last succeeded, mmz skips execution and exits 0.
Otherwise it runs the command, streams its output, and records the new state on
success.

It is not a build system: no task ordering, no dependency graph between rules,
no output/artifact tracking, no remote cache. It answers one question per
invocation — are this rule's inputs unchanged since it last passed?

## Usage

```
mmz <command> [args...]   memoize a command
mmz --init                write a starter mmz.yaml
mmz --status              show each rule's freshness
mmz --schema              print the mmz.yaml JSON Schema
mmz --version
mmz --help
```

```yaml
# mmz.yaml — nearest one, searching upward from the working directory
scopes:
  rust:  ["**/*.rs", Cargo.toml, Cargo.lock, rust-toolchain.toml]
  truth: ["truth/**", lib/spec.typ]
commands:
  - name: "cargo run -q -- check"   # token-prefix matcher + cache identity
    inputs: [rust, truth]
  - name: "cargo clippy"
    inputs: [rust]
# strict: [no_match, no_inputs]     # the default; omit for both, [] to relax all
```

```bash
mmz cargo clippy --workspace        # memoized; skipped when rust inputs unchanged
chronic mmz cargo clippy --workspace # wrappers go outside mmz
```

Prefix mmz wherever memoization is wanted — a Justfile recipe line, a parallel
string, a shell, a git hook. mmz receives a clean argv vector (not a
shell-expanded string), so matching is robust and there is no nesting blind
spot. No integration with `just` or any runner.

## Manifest

- `scopes`: named glob sets, defined once and referenced by many commands, so a
  shared input path is declared in one place. Globs follow the common
  convention — `*` stays within a directory, `**` crosses directories.
- `commands`: an ordered list of rules. Each rule has a `name` (the matcher and
  the cache identity) and `inputs` (scope names whose globs, unioned, are the
  rule's input set).
- `gitignore` (default true): glob expansion skips git-ignored paths, so build
  artifacts never enter an input set. Explicitly listed literal paths are always
  kept. The `.git` directory is never traversed; symlinks are not followed.
- `strict` (default: all): the runtime cases mmz errors on rather than falling
  back — `no_match` (no rule matches) and `no_inputs` (a matched rule resolves to
  zero files). Omit for both; list a subset to relax the rest; `[]` to relax all.

Validation (at load): command names are non-empty and unique; every referenced
scope is defined; `strict` names are known. A failure here is fatal (see
Strictness) — a misconfigured manifest must surface, not silently skip.

## Matching

The matcher is a token list (the `name`, split on whitespace). It matches when
it is a prefix of the invoked argv tokens: `cargo test` matches `cargo test` and
`cargo test --workspace`, not `cargo build` and not the bare `cargo`. Matching
is on whole tokens, so `car` does not match `cargo`.

Rules are tried in manifest order; the first match wins. The author orders
specific rules before general ones. (Decided over longest-prefix: ordering is
explicit and predictable, and the operator decides how to match.)

No rule matches → mmz errors (exit 3) by default; relax `no_match` to run the
command unmemoized instead. Running a bare command without `mmz` is always
unmemoized; there is no `--no-memo`.

## Cache identity and key

The cache identity is the matched rule (its `name`), not the full argv. The
operator controls granularity through how specifically rules are written: a
coarse `cargo run` rule memoizes every `cargo run …` invocation as one unit; if
that conflates commands with different real inputs, the author splits the rule
or narrows the matcher. This is a deliberate choice to keep the operator in
control of matching.

A record is trusted (a skip) only when all of these match the current state:

- `status == ok` — a failed run is never fresh, so a still-broken command
  re-runs even with unchanged inputs.
- `input_digest` — blake3 over the sorted `(relative-path, content-hash)` list
  of the rule's resolved inputs. A rename, deletion, edit, or membership change
  all shift it.
- `format` and `algorithm` — a record from an incompatible mmz invalidates.
- `command` — guards against a slug collision.

Toolchain sensitivity is modeled as ordinary inputs: add `rust-toolchain.toml`
or `flake.lock` to a scope and a toolchain bump busts the cache. mmz trusts file
content, not ambient environment.

## Correctness contract

The governing asymmetry, because the failure is silent:

- Under-declaring a rule's inputs → mmz skips a command that should have run →
  false green. Dangerous.
- Over-declaring inputs → an unnecessary re-run. Harmless.

A rule's scopes must be a superset of every file any matching invocation could
depend on. When in doubt, broaden the scope. When a rule's args can widen its
real inputs (`--root <dir>` pulling in a different subtree), broaden the scope or
split the rule.

## Strictness

mmz fails closed by default: rather than silently running a command it cannot
memoize, it errors, so a misconfiguration surfaces instead of disabling
memoization unnoticed. The cases:

- no `mmz.yaml` found → error (exit 4). Always strict — no config to consult,
  and running mmz outside a manifest is a mistake.
- unparseable or invalid manifest → error (exit 4). Always strict, same reason.
- no matching rule → error (exit 3) unless `no_match` is relaxed, then run
  unmemoized.
- a matched rule resolving to zero files → error (exit 3) unless `no_inputs` is
  relaxed, then run unmemoized every time (never a skip-forever trap).

Only the last two are configurable; the first two have no manifest to read a
`strict` list from. A cache read that fails is a miss (re-run, never a wrongful
skip); a cache write that fails after a run is logged, never fatal — the command
already ran and its exit code stands.

What mmz never does is skip a command whose inputs it has not confirmed
unchanged. The asymmetry it guards is silent under-skipping, not loud refusal.

## State

Records live in a gitignored `.mmz/` directory, one YAML file per rule, named
`<readable-slug>-<short-hash>.yaml`. One file per rule means lock-free
concurrent writes when rules run in parallel. The state is derived and
throwaway — it is not committed and carries no review value.

```yaml
format: 1
algorithm: blake3
command: cargo clippy
input_digest: 105613a8…
status: ok
ran_at: 1781463976
```

## Exit codes

- Skip (fresh) → 0.
- Ran → the command's exit code, propagated (out-of-range or signal death → 1).
- Usage error (empty invocation, unknown option, `--init` over an existing
  manifest) → 2.
- Strict refusal (no matching rule, or a matched rule with no inputs) → 3.
- Manifest missing or invalid → 4.
- Internal error → 70.
- Spawn failure → 127.

## Open / deferred

- Inputs source is the working tree (filesystem); mmz does not depend on git.
- An `expect_in` drift guard — verifying a rule is actually wrapped where the
  author expects (e.g. in a Justfile) — is deferred past 0.1.0.
- A mtime fast-path could skip re-hashing unchanged files, but mtime is unsafe
  as the freshness signal: coarse filesystem granularity, mtime-preserving
  copies (`cp -p`, rsync, tar), and clock skew can all miss a real change — the
  dangerous direction. If added it must be a re-hash-skip stat cache (size,
  mtime, ctime, inode, plus a racy-timestamp guard), never the freshness
  decision; blake3 is fast enough that this stays deferred.

## Non Goals

`mmz` follows the Unix philosophy — one thing, done right — so these stay out of
scope:

- Task orchestration: no execution order or dependency graph; use a task runner.
- Output replay: only the exit code is cached, never stdout, stderr, or artifacts.
- Automatic dependency tracing: no strace; scopes are declared explicitly.
- Remote caching: state is strictly local and throwaway.
- Deep runner integration: no plugins or hooks; mmz is a dumb CLI prefix.
