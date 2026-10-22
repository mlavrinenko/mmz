#import "@local/mindtape:0.2.0": *
#import "/.mindtape/lib.typ": *

#show: task.with(
  title: "Run the aarch64-linux release binary before it ships",
  kind: "dev",
  tags: ("build",),
  priority: framework("ice", confidence: 0.8, ease: 6.0, impact: 5.0),
  difficulty: 1,
  estimate: (src-files: 2),
  assignee: "agent",
  x-goals: (
    product: (
      no-false-green: neutral,
      exact-inputs: neutral,
      cheap-gates: neutral,
      agent-drivable: neutral,
      composable-manifest: neutral,
      ready-binaries: helps[both aarch64-linux archives are started on their own target before upload, like every other one],
      docs-teach-judgement: neutral,
    ),
    process: (
      mmz-gates-mmz: neutral,
      docs-read-facts: neutral,
      rules-not-memory: neutral,
      cheap-delegation: neutral,
    ),
  ),
  status: accepted(2026, 10, 23)[waits on the next release run],
  links: related(
    "mmz-verify-the-lang-all-release-build-on-every-target.typ",
  )[found by this verification],
)

== Problem

+ Open `.github/workflows/release.yml` at `v0.13.0`.
+ Read the "Verify the flavour is what it claims" step.
+ 🔴 It carries `if: matrix.target != 'aarch64-unknown-linux-gnu'`. Both aarch64-linux binaries were cross-built on x86_64 and shipped without ever being started.

== TO-BE

- 🟢 Both aarch64-linux legs build on `ubuntu-24.04-arm` and run the verify step, like every other target.
- 🟢 The next tagged release run shows both legs green, and the `full` one prints `28 ast langs`.
- 🟢 The binary's minimum glibc stays at 2.39, the one the cross sysroot had.

== Changes

- `.github/workflows/release.yml`: build aarch64-linux natively on `ubuntu-24.04-arm` and drop the verify step's skip. Landed in `da4b06e`.
- `.github/workflows/ci.yml`: stop calling the aarch64 target "the cross". Landed in `da4b06e`.
- What remains is proof: read the next tagged release run leg by leg, then close.

== Rollback

- Revert `da4b06e`. The legs go back to the cross build on `ubuntu-latest`, and the verify skip returns.

== Out of scope

- qemu-user after the cross build. The native runner made it unnecessary.
- The macOS and Windows legs, which already run on their own targets.

== Dogfood

This repo's own release is the change. The tag cut after the current backlog lands runs it, and `mt done` waits on that run.
