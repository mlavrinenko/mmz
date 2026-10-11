//! Shared helpers for the integration suite. The suite is one binary
//! (`tests/it/main.rs`), and this module is declared there once, so each test
//! module reaches it as `crate::support`.

use std::fs;
use std::path::Path;

use assert_cmd::Command;

/// An `mmz` invocation rooted at `dir`.
pub fn mmz(dir: &Path) -> Command {
    let mut cmd = Command::cargo_bin("mmz").expect("binary should build");
    cmd.current_dir(dir);
    cmd
}

/// Writes the manifest to `.mmz/config.yaml` under `dir`.
pub fn write_manifest(dir: &Path, body: &str) {
    let cfg = dir.join(".mmz");
    fs::create_dir_all(&cfg).expect("create .mmz");
    fs::write(cfg.join("config.yaml"), body).expect("write manifest");
}
