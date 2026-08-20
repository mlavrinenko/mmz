//! Every rule tagged `gate` must name an input set whose closure includes
//! `flake.lock`.
//!
//! `docs/contributing/gates.md` argues, under "Getting the scopes right",
//! that a gate's inputs should be broad enough that a dev-shell bump busts
//! it — the toolchain a gate ran under is as much a fact about the pass as
//! the sources are, and a rule that does not depend on `flake.lock` can read
//! fresh after `nix flake update` swapped the binary it runs. That claim was
//! never enforced: `just machete` shipped with `inputs: [manifests,
//! recipe-machete]`, and neither `manifests` nor `recipe-machete` reaches
//! `flake.lock`, so a toolchain bump left its recorded pass looking current
//! even though the `cargo-machete` it last ran under no longer exists on
//! PATH. See `tasks/mmz-just-machete-is-not-busted-by-a-dev-shell-bump.typ`.
//!
//! Rather than re-parsing `.mmz/conf.d/*.yaml` and reimplementing the
//! fragment merge (imports, cross-file scope references, duplicate-key
//! rejection — see `src/compose.rs`), this test shells out to the compiled
//! binary's own `--dump-config=json` against this repo's real manifest. That
//! is the already-resolved model: every scope's globs and every command's
//! inputs, merged exactly the way `mmz` merges them for a real run. Trusting
//! it here means a bug in the merge itself is someone else's test's job
//! (`compose_tests.rs`, `compose_merge_tests.rs`); this one only asks whether
//! the *declared* closure, once resolved, reaches `flake.lock`.
//!
//! This is the only file under `tests/` that runs `mmz` against the repo's
//! own manifest instead of a synthetic one in a tempdir, so it does not
//! `mod support;` — every other integration test that does also calls
//! `support::write_manifest`, and this one has no fixture to write, which
//! would leave that helper dead code in this binary alone.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use assert_cmd::Command;

/// An `mmz` invocation rooted at `dir`. A trimmed copy of
/// `tests/support::mmz` — see the module doc for why this file does not pull
/// in `mod support` instead.
fn mmz(dir: &Path) -> Command {
    let mut cmd = Command::cargo_bin("mmz").expect("binary should build");
    cmd.current_dir(dir);
    cmd
}

/// How many `gate`-tagged rules `.mmz/conf.d/` is expected to declare. A
/// floor, not an exact count, so a new gate does not need to bump this — but
/// a scan that suddenly sees none of them means the scan broke, not that the
/// repo shed every gate.
const MIN_GATE_RULES: usize = 10;

/// name -> the scope's own globs, read out of a `--dump-config=json` report.
/// `--dump-config` already merged every fragment, so a scope declared in one
/// file and referenced from another (e.g. `toolchain` in 10-rust.yaml, named
/// from `just machete`) needs no special handling here.
fn scope_globs(dump: &serde_json::Value) -> BTreeMap<String, BTreeSet<String>> {
    dump.get("scopes")
        .and_then(serde_json::Value::as_array)
        .expect("dump-config should carry a scopes array")
        .iter()
        .map(|scope| {
            let name = scope
                .get("name")
                .and_then(serde_json::Value::as_str)
                .expect("scope name")
                .to_owned();
            let globs = scope
                .get("globs")
                .and_then(serde_json::Value::as_array)
                .expect("scope globs")
                .iter()
                .filter_map(serde_json::Value::as_str)
                .map(str::to_owned)
                .collect();
            (name, globs)
        })
        .collect()
}

/// Whether `command`'s `inputs` resolve, through `scopes`, to a closure that
/// contains `flake.lock`. An input is either a scope (resolve its globs) or a
/// probe (no files, so it cannot contribute `flake.lock` and is skipped) —
/// only a scope's glob list can close over a literal path.
fn closes_over_flake_lock(
    command: &serde_json::Value,
    scopes: &BTreeMap<String, BTreeSet<String>>,
) -> bool {
    let inputs = command
        .get("inputs")
        .and_then(serde_json::Value::as_array)
        .expect("command inputs");
    inputs.iter().any(|input| {
        input
            .as_str()
            .and_then(|input| scopes.get(input))
            .is_some_and(|globs| globs.contains("flake.lock"))
    })
}

/// Whether `command` carries the `gate` tag.
fn is_gate(command: &serde_json::Value) -> bool {
    command
        .get("tags")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|tags| tags.iter().any(|tag| tag.as_str() == Some("gate")))
}

#[test]
fn every_gate_rule_s_input_closure_includes_flake_lock() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let out = mmz(&root)
        .arg("--dump-config=json")
        .output()
        .expect("mmz --dump-config=json should run");
    assert!(
        out.status.success(),
        "dump-config should succeed against this repo's own manifest: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let dump: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("dump-config=json should print valid JSON");
    let scopes = scope_globs(&dump);
    let commands = dump
        .get("commands")
        .and_then(serde_json::Value::as_array)
        .expect("dump-config should carry a commands array");

    let mut checked = 0_usize;
    let mut missing = Vec::new();

    for command in commands.iter().filter(|command| is_gate(command)) {
        checked += 1;
        if !closes_over_flake_lock(command, &scopes) {
            let name = command
                .get("name")
                .and_then(serde_json::Value::as_str)
                .expect("command name");
            missing.push(name.to_owned());
        }
    }

    assert!(
        checked >= MIN_GATE_RULES,
        "expected at least {MIN_GATE_RULES} rules tagged `gate`, saw \
         {checked} — the scan above has stopped matching the merged manifest"
    );

    assert!(
        missing.is_empty(),
        "these gate rules' input closures do not include `flake.lock`, so a \
         `nix flake update` that changes the dev shell leaves their recorded \
         pass looking fresh even though the toolchain that ran them changed: \
         {missing:?}"
    );
}
