//! Shared helpers for the integration suite. The suite is one binary
//! (`tests/it/main.rs`), and this module is declared there once, so each test
//! module reaches it as `crate::support`.

use std::fs;
use std::path::Path;

use assert_cmd::Command;

/// An `mmz` invocation rooted at `dir`.
///
/// `MMZ_DISABLE` is cleared, so a suite run under a forced `just check` still
/// sees mmz skip, and `MMZ_REPORT`, so a suite run under a reporting caller
/// leaves its file alone; a test that wants either sets it after this returns.
pub fn mmz(dir: &Path) -> Command {
    let mut cmd = Command::cargo_bin("mmz").expect("binary should build");
    cmd.current_dir(dir)
        .env_remove("MMZ_DISABLE")
        .env_remove("MMZ_REPORT");
    cmd
}

/// Writes the manifest to `.mmz/config.yaml` under `dir`.
pub fn write_manifest(dir: &Path, body: &str) {
    let cfg = dir.join(".mmz");
    fs::create_dir_all(&cfg).expect("create .mmz");
    fs::write(cfg.join("config.yaml"), body).expect("write manifest");
}
