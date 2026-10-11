#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "Fork and land worktrees with in-repo worktrunk hooks",
  tags: ("tooling", "gating"),
  priority: framework("ice", confidence: 0.85, ease: 7.0, impact: 6.0),
  status: wip(2026, 10, 11),
)

== Summary

~/.nix no longer ships worktrunk user hooks, and mmz carries none of its own. A new worktree (\`wt switch --create x\`) needs a manual \`direnv allow\` before its dev shell loads, so \`just\`, \`mt\` and the other repo tools do not resolve there until someone remembers. \`wt merge\` runs no gate at all: a branch fast-forwards onto main whether or not \`just check\` passed, or \`mt check --commits main..HEAD\` accepts its footers. And after \`wt remove\` the gate records of the worktree are lost, so the primary checkout re-runs every arm that already passed on the branch, although \`mmz --import\` exists for exactly that.

The maintainer wants the delegation tooling in the repo, nothing only one machine has. canibump already does it: \`.config/wt.toml\`, \`worktrunk\` in the dev shell, \`just fork \<slug>\` and \`just land\`. Copy that shape here.

Changes. \`flake.nix\` gains \`worktrunk\` in the dev shell. \`.config/wt.toml\` holds \`pre-start\` (\`direnv allow\`), \`pre-merge\` (\`just check\`, then \`mt check --commits \<target>..HEAD\`) and \`pre-remove\` (step into the primary checkout and \`mmz --import\` the worktree's records; the hook runs inside the worktree being removed). \`just fork \<slug>\` wraps \`wt switch --create --yes\`, \`just land\` wraps \`wt merge --no-squash --no-commit --yes\`. CONTRIBUTING and AGENTS say to delegate with \`just fork\` and land with \`just land\`.

Done when \`just fork scratch\` yields a worktree whose dev shell resolves \`just\`, \`wt hook pre-merge\` there runs the gate and the commit check, and \`wt remove scratch\` prints \`mmz: imported N records\`.

