//! Shared helpers for the integration suite under `tests/`. Living in a
//! `support/mod.rs` subdirectory keeps Cargo's test autodiscovery from
//! treating this as its own integration test binary — only files directly
//! under `tests/` become one — so each test file can `mod support;` it
//! without duplicating the boilerplate.

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
