#import "../utils/page.typ": page
#import "../utils/ui.typ": hero
#import "../utils/site.typ": u

#let meta = (
  route: "/",
  label: "Overview",
  title: "mmz",
  summary: "A memoized command runner: prefix any command with mmz and it skips when the inputs you declared are unchanged since it last succeeded.",
  home: true,
)
#metadata(meta) <page-meta>

#show: page.with(..meta)

#hero()

= One question per run

Is this rule's work still done?

That is the whole of it. A rule declares a command and the inputs that command
depends on. `mmz` hashes those inputs, compares the digest against the record
the last successful run left behind, and either skips or runs. There is no task
graph to satisfy, no artifact to replay, no daemon to keep alive.

= What it is not

`mmz` is not a build system, and the boundary is deliberate — see
#link(u("/comparison/"))[Comparison] for the whole rubric.

- #strong[No orchestration.] No execution order, no dependency graph. Your task
  runner already does that; `mmz` is a prefix on one line of it.
- #strong[No output replay.] Only the exit code is cached — never stdout,
  stderr, or artifacts. A declared output is checked for existence, never stored
  or restored.
- #strong[No dependency tracing.] Nothing is inferred by watching syscalls.
  Scopes are declared, and being wrong about them is your risk to manage.
- #strong[No remote cache.] State is local and throwaway. Delete it and the
  worst case is one honest re-run.

= Fail closed

The failure this design fears is the silent one: a command skipped that should
have run. So the asymmetry is baked in.

- Under-declaring a rule's inputs skips work that should have happened — a false
  green, and dangerous.
- Over-declaring buys an unnecessary re-run — and nothing else.

Every ambiguous case therefore errors rather than guessing. A missing or invalid
manifest is an error; a command matching no rule is an error; a rule whose
inputs resolve to zero files is an error. Relax those last two per project with
`strict` when you mean to — never by default.
#link(u("/concepts/"))[Concepts] has the full contract.

= Built for gates

`mmz --is-fresh` inverts the wrapper: instead of running a stale command, it
_refuses_ one. A pre-push hook can require that an expensive check was already
run and memoized, without paying to run it at the least convenient moment — see
#link(u("/gating/"))[Gating with tags]. `mmz` gates its own repository that way.
