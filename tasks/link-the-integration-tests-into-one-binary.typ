#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "Link the integration tests into one binary",
  priority: framework("ice", confidence: 0.8, ease: 7.0, impact: 5.0),
  tags: ("tests", "build", "efficiency"),
  status: proposed(2026, 10, 11),
)

== Summary

Cargo builds one executable per file in `tests/`, and each one links every dependency again. mmz has 19 test files, so `cargo test --no-run` builds 19 integration executables, and 14 of them also compile their own copy of `tests/support/mod.rs` through `mod support;`. Link them into one `tests/it` binary, with `support` declared once.

Measured 2026-10-11 in mindtape: 74 test files gave `target/debug/deps` 263 executables averaging 298 MB, 80 GB in all. `just cover` builds the whole set a second time, instrumented: 51 GB in one worktree, and the disk filled. canibump's 7 test files built 28 binaries of about 37 MB each with full debug info. Debug info size is handled host-wide in `~/.nix` (task `shrink-rust-dev-builds-host-wide-with-line-table-debug-info-and-weekly-unit-cleanup`); this task is the per-repo half. Expect the integration binary count to go from 19 to 1, with link time down by about the same factor.

== Changes

+ Record the baseline: `just count-tests` if the repo has it, else the `test result:` totals of `cargo test --workspace`, and after `cargo test --no-run` the output of `find target/debug/deps -maxdepth 1 -type f -executable | wc -l`. Do it for the default features and for `--features lang-all` (`just test-lang-all`), because `cli_ast_probes.rs` is gated on `lang-rust`.
+ `git mv tests/*.rs tests/it/` and `git mv tests/support tests/it/support`, then add `tests/it/main.rs` with `mod support;` once and one `mod <stem>;` per moved file. Cargo autodiscovers `tests/it/main.rs` as the test target `it`.
+ In each of the 14 files that declare `mod support;` (`cli.rs`, `cli_ast_probes.rs`, `cli_clock.rs`, `cli_declaration_changes.rs`, `cli_dump.rs`, `cli_empty_selection.rs`, `cli_json_probes.rs`, `cli_outputs.rs`, `cli_parametric.rs`, `cli_probes.rs`, `cli_scope_gitignore.rs`, `cli_unreadable_inputs.rs`, `cli_unreadable_state.rs`, `gate_probe_normalisation.rs`), delete that line and make every `support::...` path read `crate::support::...`.
+ Inner attributes survive as module attributes: `#![cfg(unix)]` in `cli_unreadable_state.rs:12` and `cli_unreadable_inputs.rs:14`, and `#![cfg(feature = "lang-rust")]` in `cli_ast_probes.rs:14`. Check that each still sits before the file's first item.
+ Relative paths: no test file uses `include_str!`, `include_bytes!` or `#[path]`. The `env!("CARGO_MANIFEST_DIR")` paths in `changelog_history_gate.rs:77`, `docs_capture_paths.rs:24`, `gate_inputs_pin_the_toolchain.rs:160` and `gate_probe_normalisation.rs:99` are unaffected.
+ `Cargo.toml` has no `[[test]]` entries; add none unless autodiscovery fails.
+ Rewrite the prose that names the old layout: the module doc of `tests/support/mod.rs` (it says each test file can `mod support;` it, and that being a subdirectory is what keeps it from becoming a binary), `tests/gate_inputs_pin_the_toolchain.rs:70` and its module doc (it keeps a trimmed copy of `mmz()` and explains why it does not pull in `mod support`; as a module of `it` it can use `crate::support::mmz` and drop the copy), `docs/contributing/gates.md:84` and `docs/src/contributing/gates.typ:172` (both cite `tests/gate_inputs_pin_the_toolchain.rs`; edit the Typst source and regenerate the Markdown), `AGENTS.md:53` and `CONTRIBUTING.md:89` (they say `tests/`, so add that the suite is one binary and a new file goes in as `tests/it/<name>.rs` plus a `mod` line).
+ Already correct: `.mmz/conf.d/10-rust.yaml:25` (`tests/**/*.rs` matches `tests/it/`). The Justfile runs `cargo test --workspace` (lines 100, 115) with no `--test` flag, and `tarpaulin.toml` names no test file. Re-run `grep -rnE "tests/" --exclude-dir=target --exclude-dir=.git --exclude-dir=tasks .` to confirm.
+ Check: the test totals equal the baseline for both feature sets, `cargo test --no-run` shows one integration executable, and `just check` is green.

== Watch for

- This repo gates on plain `cargo test`, not nextest, so the tests of the merged binary share one process as parallel threads. A test that sets an environment variable, changes the working directory or binds a fixed port could now disturb another. Most drive the real binary through `assert_cmd` in their own temp dir, which is isolated, but run the suite a few times before trusting one green pass. A test that only fails together with others is a latent bug to fix in that test, not a reason to keep the old layout.
- Out of scope: inline `#[cfg(test)]` unit tests, which already share one binary per crate, and debug info settings, which are host-wide.
