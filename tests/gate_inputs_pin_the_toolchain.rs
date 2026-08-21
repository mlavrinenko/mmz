//! Every rule tagged `gate` must name at least one input that moves when the
//! dev shell does.
//!
//! `docs/contributing/gates.md` argues, under "Getting the scopes right", that
//! a gate's inputs have to reach the toolchain pins: the binaries a gate ran
//! under are as much a fact about the pass as the sources are, and a rule that
//! does not depend on them can read fresh after a `nix flake update` swapped
//! the binary it runs. That claim went unenforced long enough to be violated —
//! `just machete` shipped with `inputs: [manifests, recipe-machete]`, and
//! neither of those reaches `flake.lock`, so a toolchain bump left its recorded
//! pass looking current even though the `cargo-machete` that earned it no
//! longer existed on PATH. See
//! `tasks/mmz-just-machete-is-not-busted-by-a-dev-shell-bump.typ`.
//!
//! # Why this asserts on `flake.lock` and not on "a toolchain pin"
//!
//! `rust-toolchain.toml` is in the `rust` scope, and nine of the ten gates name
//! `rust`, so accepting it here would pass almost every rule for free and prove
//! nothing about the one that matters. It is also the wrong file: nothing in
//! this repo installs a toolchain from it — the dev shell does, out of
//! `flake.nix` — so it is a declaration that does not move when the binaries
//! move. `flake.lock` is the file that does.
//!
//! # Two mechanisms reach it, and both count
//!
//! A rule can depend on `flake.lock` two ways, and this test accepts either:
//!
//! - a **scope** whose globs name it, hashing the whole file; or
//! - a **probe** whose `file:` reads it, hashing only the node its `json:`
//!   selects.
//!
//! The second is what `.mmz/conf.d/` uses today — `nixpkgs-tools`,
//! `qahq-tools`, `tola-tools`, one per flake input the dev shell draws binaries
//! from — because a whole-file hash conflates a hundred nodes and busts clippy
//! when `nixpkgs-lib` moves. The property being asserted survived that change;
//! only the mechanism carrying it did not, which is why this test resolves
//! probes as well as scopes. Its predecessor
//! (`gate_inputs_close_over_flake_lock.rs`) resolved scopes only and was
//! correct about the old spelling and wrong about the new one.
//!
//! What this test cannot judge is whether a probe selects the *right* node. A
//! gate running `linecop` could name a probe reading `.nodes["flake-utils"]`
//! and pass here. That is the same boundary `src/probe.rs` draws around probe
//! content generally: mmz refuses a probe that measures nothing, and leaves
//! measuring the wrong thing to the manifest author. This test proves a gate
//! measures the lockfile at all — the failure that actually happened.
//!
//! Rather than re-parsing `.mmz/conf.d/*.yaml` and reimplementing the fragment
//! merge (imports, cross-file scope and probe references, duplicate-key
//! rejection — see `src/compose.rs`), this test shells out to the compiled
//! binary's own `--dump-config=json` against this repo's real manifest. That is
//! the already-resolved model: every scope's globs, every probe's source, and
//! every command's inputs, merged exactly the way `mmz` merges them for a real
//! run. Trusting it here means a bug in the merge itself is someone else's
//! test's job (`compose_tests.rs`, `compose_merge_tests.rs`); this one only
//! asks whether the *declared* inputs, once resolved, reach the lockfile.
//!
//! This is the only file under `tests/` that runs `mmz` against the repo's own
//! manifest instead of a synthetic one in a tempdir, so it does not
//! `mod support;` — every other integration test that does also calls
//! `support::write_manifest`, and this one has no fixture to write, which would
//! leave that helper dead code in this binary alone.

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

/// The one file whose content moves when the dev shell moves. See the module
/// doc for why `rust-toolchain.toml` does not qualify.
const TOOLCHAIN_PIN: &str = "flake.lock";

/// Names of every input that reaches [`TOOLCHAIN_PIN`], whichever mechanism it
/// uses: a scope whose globs list it, or a probe that reads it with `file:`.
///
/// Both kinds land in one set because `inputs:` has one namespace — a rule
/// names `rust` and `nixpkgs-tools` identically, and so does the lookup below.
fn inputs_reaching_the_pin(dump: &serde_json::Value) -> BTreeSet<String> {
    let scopes = entries(dump, "scopes")
        .filter(|scope| globs(scope).is_some_and(|globs| globs.contains(&TOOLCHAIN_PIN)));
    let probes = entries(dump, "probes").filter(|probe| {
        probe.get("file").and_then(serde_json::Value::as_str) == Some(TOOLCHAIN_PIN)
    });
    scopes.chain(probes).map(name_of).collect()
}

/// The entries of a top-level array section of the dump (`scopes`, `probes`,
/// `commands`), which `--dump-config=json` always emits even when empty.
fn entries<'a>(
    dump: &'a serde_json::Value,
    section: &str,
) -> impl Iterator<Item = &'a serde_json::Value> {
    dump.get(section)
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| panic!("dump-config should carry a {section} array"))
        .iter()
}

/// A dump entry's `name`, which every scope, probe and command carries.
fn name_of(entry: &serde_json::Value) -> String {
    entry
        .get("name")
        .and_then(serde_json::Value::as_str)
        .expect("dump entry should carry a name")
        .to_owned()
}

/// A scope entry's declared globs, as strings.
fn globs(scope: &serde_json::Value) -> Option<Vec<&str>> {
    Some(
        scope
            .get("globs")?
            .as_array()?
            .iter()
            .filter_map(serde_json::Value::as_str)
            .collect(),
    )
}

/// A rule's `inputs`, which the dump omits entirely when the list is empty.
fn inputs_of(command: &serde_json::Value) -> Vec<String> {
    command
        .get("inputs")
        .and_then(serde_json::Value::as_array)
        .map(|inputs| {
            inputs
                .iter()
                .filter_map(serde_json::Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

/// Whether `command` carries the `gate` tag.
fn is_gate(command: &serde_json::Value) -> bool {
    command
        .get("tags")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|tags| tags.iter().any(|tag| tag.as_str() == Some("gate")))
}

/// The merged manifest of the repo this test is compiled from.
fn dump_this_repo() -> serde_json::Value {
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
    serde_json::from_slice(&out.stdout).expect("dump-config=json should print valid JSON")
}

#[test]
fn every_gate_rule_depends_on_the_dev_shell_s_lockfile() {
    let dump = dump_this_repo();
    let reaching = inputs_reaching_the_pin(&dump);

    assert!(
        !reaching.is_empty(),
        "no scope or probe in the merged manifest reads `{TOOLCHAIN_PIN}` at \
         all, so the scan below would fail every gate for the wrong reason — \
         the mechanism this test resolves has changed, not the property"
    );

    let mut checked = 0_usize;
    let mut missing: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for command in dump
        .get("commands")
        .and_then(serde_json::Value::as_array)
        .expect("dump-config should carry a commands array")
        .iter()
        .filter(|command| is_gate(command))
    {
        checked += 1;
        let inputs = inputs_of(command);
        if !inputs.iter().any(|input| reaching.contains(input)) {
            missing.insert(name_of(command), inputs);
        }
    }

    assert!(
        checked >= MIN_GATE_RULES,
        "expected at least {MIN_GATE_RULES} rules tagged `gate`, saw \
         {checked} — the scan above has stopped matching the merged manifest"
    );

    assert!(
        missing.is_empty(),
        "these gate rules name no input that reads `{TOOLCHAIN_PIN}`, so a \
         `nix flake update` that swaps the binaries they run leaves their \
         recorded pass looking fresh — give each one the `*-tools` probe for \
         the flake input its tools come out of: {missing:?}\n\nthe inputs that \
         would have counted: {reaching:?}"
    );
}
