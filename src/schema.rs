//! The JSON Schemas for `.mmz/config.yaml` and for a fragment it `imports:`,
//! emitted by `mmz --schema` and `mmz --schema=fragment`.
//!
//! Both schemas are maintained by hand alongside [`crate::manifest`] and
//! embedded at build time, so each `mmz --schema…` form and the file editors
//! load by URL are the same bytes. Point `# yaml-language-server: $schema=…`
//! at either for completion and validation in your editor.
//!
//! The two are not maintained independently: [`crate::compose`] rejects
//! `cache_dir`, `gitignore`, `strict`, `on_hit` and `probe_shell` in an
//! imported file (see
//! `check_no_policy_keys` there), so [`FRAGMENT_SCHEMA`] is exactly
//! [`SCHEMA`] with those five properties removed — same `imports`, `scopes`,
//! `probes` and `commands` shapes, same `additionalProperties: false`. The
//! `tests` module below asserts that relationship on every `cargo test`
//! rather than through a generate-then-diff gate, so drift in either
//! direction fails a plain unit test instead of needing new gate wiring.

/// The mmz manifest JSON Schema (draft 2020-12), as published in the repo.
pub const SCHEMA: &str = include_str!("../schema/mmz.schema.json");

/// The JSON Schema (draft 2020-12) for a file named in a manifest's
/// `imports:` list. See the module doc for how this is kept from drifting
/// away from [`SCHEMA`].
pub const FRAGMENT_SCHEMA: &str = include_str!("../schema/mmz-fragment.schema.json");

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use crate::compose::POLICY_KEYS;

    use super::{FRAGMENT_SCHEMA, SCHEMA};

    #[test]
    fn schema_documents_every_manifest_field() {
        assert!(SCHEMA.contains("\"$schema\""), "declares its meta-schema");
        for key in [
            "imports",
            "scopes",
            "globs",
            "commands",
            "gitignore",
            "cache_dir",
            "probe_shell",
            "match",
            "exact",
            "tags",
            "strict",
            "no_match",
            "no_inputs",
            "on_hit",
        ] {
            assert!(SCHEMA.contains(key), "schema mentions `{key}`");
        }
    }

    #[test]
    fn fragment_schema_is_valid_json_describing_a_fragment() {
        let fragment: serde_json::Value =
            serde_json::from_str(FRAGMENT_SCHEMA).expect("fragment schema is valid json");
        assert_eq!(
            fragment.get("$schema").and_then(serde_json::Value::as_str),
            Some("https://json-schema.org/draft/2020-12/schema"),
            "declares the same meta-schema as the config schema"
        );
        for key in ["imports", "scopes", "probes", "commands"] {
            assert!(
                FRAGMENT_SCHEMA.contains(key),
                "fragment schema mentions `{key}`"
            );
        }
    }

    /// The derivation the module doc promises: the fragment schema's
    /// property set is exactly the config schema's minus the five policy
    /// keys, every property the two share is byte-identical (compared as
    /// parsed JSON, so key order in the source files cannot fail this by
    /// accident), and both keep `additionalProperties: false`. Fails on
    /// drift in either direction — a key added to one schema and not the
    /// other, or a shared property's description or shape edited in only
    /// one place.
    #[test]
    fn fragment_property_set_is_config_minus_policy_keys() {
        let config: serde_json::Value =
            serde_json::from_str(SCHEMA).expect("config schema is valid json");
        let fragment: serde_json::Value =
            serde_json::from_str(FRAGMENT_SCHEMA).expect("fragment schema is valid json");

        let config_props = config
            .get("properties")
            .and_then(serde_json::Value::as_object)
            .expect("config schema has a properties object");
        let fragment_props = fragment
            .get("properties")
            .and_then(serde_json::Value::as_object)
            .expect("fragment schema has a properties object");

        let mut expected: BTreeSet<&str> = config_props.keys().map(String::as_str).collect();
        for key in POLICY_KEYS {
            assert!(
                expected.remove(key),
                "config schema no longer declares policy key `{key}`; update POLICY_KEYS \
                 and schema/mmz-fragment.schema.json together"
            );
        }
        let actual: BTreeSet<&str> = fragment_props.keys().map(String::as_str).collect();
        assert_eq!(
            actual, expected,
            "fragment schema's properties must be exactly the config schema's minus \
             cache_dir, gitignore, strict and on_hit"
        );

        for (key, fragment_value) in fragment_props {
            let config_value = config_props
                .get(key)
                .unwrap_or_else(|| panic!("config schema also declares `{key}`; just checked"));
            assert_eq!(
                config_value, fragment_value,
                "`{key}` must describe the same shape and prose in both schemas"
            );
        }

        // The sentence this whole test exists to prove, written literally:
        // the fragment schema forbids exactly what compose::check_no_policy_keys
        // rejects at load time — no more (a key it doesn't reject stays legal
        // in the fragment) and no less (a key it does reject is undeclared, so
        // additionalProperties: false makes setting it a validation failure).
        // POLICY_KEYS is imported from crate::compose, not redeclared here, so
        // this binds both schemas to the loader's own list rather than to a
        // second copy of it.
        for key in POLICY_KEYS {
            assert!(
                config_props.contains_key(key),
                "config schema must declare policy key `{key}`, which the loader treats \
                 as root-only"
            );
            assert!(
                !fragment_props.contains_key(key),
                "fragment schema must not declare policy key `{key}`: \
                 compose::check_no_policy_keys rejects it in a fragment, so the schema \
                 must not advertise it as legal"
            );
        }

        assert_eq!(
            config.get("additionalProperties"),
            Some(&serde_json::json!(false)),
            "config schema rejects unknown keys"
        );
        assert_eq!(
            fragment.get("additionalProperties"),
            Some(&serde_json::json!(false)),
            "fragment schema rejects unknown keys"
        );
    }

    /// The behaviour the derivation exists for: validating a fragment against
    /// [`FRAGMENT_SCHEMA`] must reject each root-only policy key and accept
    /// every rule-declaring key. With `additionalProperties: false`, a key
    /// absent from `properties` is exactly a key no document may set, so
    /// asserting absence/presence here is asserting accept/reject.
    #[test]
    fn fragment_schema_rejects_policy_keys_and_accepts_rule_keys() {
        let fragment: serde_json::Value =
            serde_json::from_str(FRAGMENT_SCHEMA).expect("fragment schema is valid json");
        let props = fragment
            .get("properties")
            .and_then(serde_json::Value::as_object)
            .expect("fragment schema has a properties object");

        for key in POLICY_KEYS {
            assert!(
                !props.contains_key(key),
                "fragment schema must not declare policy key `{key}`; \
                 additionalProperties: false makes an undeclared key a rejection"
            );
        }
        for key in ["imports", "scopes", "probes", "commands"] {
            assert!(
                props.contains_key(key),
                "fragment schema must declare `{key}`, which a fragment may legally set"
            );
        }
    }
}
