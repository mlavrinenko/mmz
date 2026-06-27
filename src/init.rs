//! `mmz --init`: scaffold a starter `mmz.yaml` in the working directory.

use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

/// Name written by [`init`].
const MANIFEST_NAME: &str = "mmz.yaml";

/// Commented starter manifest. Doubles as inline documentation of the format,
/// and carries the `$schema` line so editors validate the file immediately.
///
/// The `$schema` URL is pinned to the version that scaffolded the file (the
/// `v{version}` git tag), not `main`, so each project validates against the
/// schema its mmz was built for even when projects pin different versions.
pub const TEMPLATE: &str = concat!(
    "# yaml-language-server: $schema=https://raw.githubusercontent.com/mlavrinenko/mmz/v",
    env!("CARGO_PKG_VERSION"),
    "/schema/mmz.schema.json
# mmz.yaml — memoized command runner config.
# Prefix a command with `mmz`; it is skipped when the matched rule's inputs are
# byte-for-byte unchanged since the command last succeeded.

# Named glob sets, declared once and referenced by commands. `*` stays within a
# directory; `**` crosses directories.
scopes:
  rust: [\"**/*.rs\", \"Cargo.toml\", \"Cargo.lock\"]

# Ordered rules. The first whose name is a token-prefix of the command wins.
# Set `match: exact` on a rule to match only the bare command (no extra args).
commands:
  - name: cargo test
    inputs: [rust]

# Printed to stderr when a command is skipped. `{cache:<field>}` pulls a field
# straight from the cache record (command, ran_at, input_digest, ...). Set it per
# command to override, or to \"\" to silence that one.
on_hit: \"mmz: skipped {cache:command} (inputs unchanged)\"

# Directory for throwaway cache records, relative to this file. Git-ignore it.
# cache_dir: .mmz

# Skip git-ignored paths when expanding globs (default true).
# gitignore: true

# Runtime cases mmz errors on instead of falling back. Omit for all (the safe
# default); list a subset to relax the rest; use [] to fall back everywhere.
# strict: [no_match, no_inputs]
"
);

/// Writes [`TEMPLATE`] to `mmz.yaml` in `cwd`, returning the path written.
///
/// # Errors
///
/// Returns [`Error::ManifestExists`] if a manifest is already present (so an
/// existing config is never clobbered), or [`Error::Io`] on a write failure.
pub fn init(cwd: &Path) -> Result<PathBuf> {
    let path = cwd.join(MANIFEST_NAME);
    if path.exists() {
        return Err(Error::ManifestExists { path });
    }
    std::fs::write(&path, TEMPLATE)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::{MANIFEST_NAME, TEMPLATE, init};
    use crate::manifest::Manifest;

    #[test]
    fn template_is_a_valid_manifest() {
        let manifest: Manifest = serde_yaml_ng::from_str(TEMPLATE).expect("template parses");
        manifest.validate().expect("template validates");
    }

    #[test]
    fn template_pins_schema_to_this_version() {
        let pinned = format!("mmz/v{}/schema/mmz.schema.json", env!("CARGO_PKG_VERSION"));
        assert!(
            TEMPLATE.contains(&pinned),
            "schema URL pins the build version"
        );
        assert!(
            !TEMPLATE.contains("/main/"),
            "no floating main ref in the scaffolded schema URL"
        );
    }

    #[test]
    fn template_scaffolds_a_configured_on_hit() {
        let manifest: Manifest = serde_yaml_ng::from_str(TEMPLATE).expect("template parses");
        assert_eq!(
            manifest.on_hit.as_deref(),
            Some("mmz: skipped {cache:command} (inputs unchanged)"),
            "init ships an on_hit notice using a cache macro"
        );
    }

    #[test]
    fn writes_then_refuses_to_clobber() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = init(dir.path()).expect("first init writes");
        assert_eq!(path, dir.path().join(MANIFEST_NAME));
        assert!(path.is_file(), "manifest written");
        assert!(init(dir.path()).is_err(), "second init refuses to clobber");
    }
}
