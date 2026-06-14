//! The JSON Schema for `mmz.yaml`, emitted by `mmz --schema`.
//!
//! The schema is maintained by hand alongside [`crate::manifest`] and embedded
//! at build time, so `mmz --schema` and the file editors load by URL are the
//! same bytes. Point `# yaml-language-server: $schema=…` at it for completion
//! and validation in your editor.

/// The mmz manifest JSON Schema (draft 2020-12), as published in the repo.
pub const SCHEMA: &str = include_str!("../schema/mmz.schema.json");

#[cfg(test)]
mod tests {
    use super::SCHEMA;

    #[test]
    fn schema_documents_every_manifest_field() {
        assert!(SCHEMA.contains("\"$schema\""), "declares its meta-schema");
        for key in [
            "scopes",
            "commands",
            "gitignore",
            "cache_dir",
            "match",
            "exact",
            "strict",
            "no_match",
            "no_inputs",
        ] {
            assert!(SCHEMA.contains(key), "schema mentions `{key}`");
        }
    }
}
