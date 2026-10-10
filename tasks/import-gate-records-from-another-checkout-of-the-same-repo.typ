#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "Import gate records from another checkout of the same repo",
  priority: framework("ice", confidence: 0.8, ease: 6.0, impact: 6.0),
  tags: ("cli", "cache"),
  status: proposed(2026, 10, 11),
)

== Summary

Delegated work runs in a sibling git worktree, and its gate passes there: `.mmz/cache/*.yaml` in that worktree records every rule as fresh. A fast-forward makes the main checkout the same tree. But each checkout keeps its own gitignored `.mmz/cache/`, so the main checkout still reads every rule as stale. Its next commit then refuses until a full `just check` re-runs a gate that already passed.

Today each repo copies the records with a shell helper. It started as the carry block at the end of ~/projects/home/mindtape/.just/scripts/integrate.sh, and became `wt-carry-gate` in ~/.nix/apps/worktrunk.nix (2026-10-11). The per-repo `.config/wt.toml` design (mindtape task `configure-worktrunk-in-the-repo-so-a-fresh-clone-needs-no-host-setup`) would copy that helper into every repo. Copying cache files belongs in mmz, which owns their format.

== Changes

+ New `mmz --import <dir>`, in the help's flag style next to `--prune`. `<dir>` is another checkout of the same repo, usually the worktree being landed.
+ For each record in `<dir>/.mmz/cache/` whose rule exists in this checkout's merged config, copy it unless the record here has a later `ran_at`. mmz stays the judge: a record whose input digest doesn't match this tree reads stale as usual, so importing can never make a stale rule look fresh. The digest is built from repo-relative paths and content hashes, which makes it portable.
+ Refuse a `<dir>` that has no `.mmz/` or that resolves to this checkout. Print one line per imported rule, plus `mmz: imported N records from <dir>`. Exit 0 when nothing needed importing.
+ Tests: a record that is fresh in the other checkout reads fresh here after import, provided the trees match. A record for different inputs stays stale. A newer local record is kept. An unknown rule is skipped.
+ Docs: README and `--help`. The worktrunk hook becomes `pre-remove = "mmz --import {{ worktree_path }}"`, run from the primary checkout, or the same from the worktree pointing at the primary. Pick the direction that is easier to reason about, and document it.

== Watch for

- The helper also copies `target/coverage/lcov.info` and rewrites its absolute `SF:` paths. That isn't mmz data, so it is out of scope. The repo's script, or a later coverage tool, keeps that part.
- The helper only carries after a landed merge (HEAD equal on both sides). `--import` doesn't need that guard, because digests already refuse a foreign tree.
