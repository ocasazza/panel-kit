//! Schema export, canonicalization, and strict decode stages.

use std::error::Error;
use std::fs;
use std::path::Path;

use panel_kit_core::spec::WorkspaceSpec;
use serde_json::{Map, Value};

const REQUIRED_NULLABLE_SCHEMA_FIELDS: &[(&str, &[&str])] =
    &[("BadgeSpec", &["override_color", "accent_color"])];

/// Export the Rust-authoritative `WorkspaceSpec` JSON Schema in canonical form.
pub(crate) fn workspace_spec_schema_json() -> Result<String, Box<dyn Error>> {
    let schema = schemars::schema_for!(WorkspaceSpec);
    let mut value = serde_json::to_value(schema)?;
    require_nullable_schema_fields(&mut value)?;

    Ok(format!("{}\n", canonical_json(&value)?))
}

/// Compare the generated schema with a committed schema file after canonicalization.
pub(crate) fn check_committed_schema(path: &Path) -> Result<(), Box<dyn Error>> {
    let generated = workspace_spec_schema_json()?;
    let committed = canonical_json_text(&fs::read_to_string(path)?)?;

    if generated == committed {
        Ok(())
    } else {
        Err(format!(
            "workspace spec schema drift: generated schema differs from {}",
            path.display()
        )
        .into())
    }
}

/// Strictly decode a normalized WorkspaceSpec document and return the typed value.
pub(crate) fn decode_workspace_spec_text(text: &str) -> Result<WorkspaceSpec, Box<dyn Error>> {
    WorkspaceSpec::from_json_str(text).map_err(Into::into)
}

/// Canonicalize arbitrary JSON text to a single-line string plus trailing newline.
pub(crate) fn canonical_json_text(text: &str) -> Result<String, Box<dyn Error>> {
    let value: Value = serde_json::from_str(text)?;

    Ok(format!("{}\n", canonical_json(&value)?))
}

/// Canonicalize a JSON value by sorting object keys recursively.
pub(crate) fn canonical_json(value: &Value) -> Result<String, serde_json::Error> {
    serde_json::to_string(&canonical_value(value))
}

/// Apply schemars post-processing for required-nullable fields.
fn require_nullable_schema_fields(schema: &mut Value) -> Result<(), Box<dyn Error>> {
    let definitions = schema
        .get_mut("$defs")
        .and_then(Value::as_object_mut)
        .ok_or("workspace spec schema is missing $defs")?;

    for &(definition, fields) in REQUIRED_NULLABLE_SCHEMA_FIELDS {
        let object = definitions
            .get_mut(definition)
            .and_then(Value::as_object_mut)
            .ok_or_else(|| format!("workspace spec schema is missing $defs/{definition}"))?;
        let required = object
            .get_mut("required")
            .and_then(Value::as_array_mut)
            .ok_or_else(|| format!("workspace spec schema is missing $defs/{definition}/required"))?;

        for &field in fields {
            let required_field = Value::String(field.to_owned());
            if !required.contains(&required_field) {
                required.push(required_field);
            }
        }
    }

    Ok(())
}

/// Recursively sort object keys while preserving arrays and scalar values.
fn canonical_value(value: &Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(items.iter().map(canonical_value).collect()),
        Value::Object(fields) => {
            let mut sorted = Map::new();
            let mut keys: Vec<_> = fields.keys().collect();
            keys.sort_unstable();

            for key in keys {
                sorted.insert(key.clone(), canonical_value(&fields[key]));
            }

            Value::Object(sorted)
        }
        scalar => scalar.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_spec_schema_matches_committed_schema() {
        let generated = workspace_spec_schema_json().expect("schema exports");
        let committed = canonical_json_text(include_str!("../../../nix/schema/workspace-spec.schema.json"))
            .expect("committed schema is canonicalizable JSON");

        assert_eq!(generated, committed);
    }

    #[test]
    fn badge_schema_requires_nullable_color_fields() {
        let schema_text = workspace_spec_schema_json().expect("schema exports");
        let schema: Value = serde_json::from_str(&schema_text).expect("schema is JSON");
        let required = schema
            .pointer("/$defs/BadgeSpec/required")
            .and_then(Value::as_array)
            .expect("BadgeSpec declares required fields");

        for field in ["override_color", "accent_color"] {
            assert!(
                required.contains(&Value::String(field.into())),
                "BadgeSpec.required should include required-nullable field {field}"
            );
            let types = schema
                .pointer(&format!("/$defs/BadgeSpec/properties/{field}/type"))
                .and_then(Value::as_array)
                .expect("required color field remains nullable");
            assert!(
                types.contains(&Value::String("null".into())),
                "BadgeSpec.{field} should still accept explicit null"
            );
        }
    }

    #[test]
    fn nix_only_field_is_rejected_by_strict_decode() {
        let mut doc: Value = serde_json::from_str(include_str!("../fixtures/workspace-spec-reference.json"))
            .expect("reference spec fixture is JSON");
        doc["chrome"]["new_field"] = Value::Bool(true);

        let errors = WorkspaceSpec::from_json_value(doc).expect_err("unknown Nix-only field is rejected");
        let first = errors.iter().next().expect("diagnostic exists");

        assert_eq!(first.pointer(), "/chrome/new_field");
        assert!(first.message().contains("unknown field; allowed fields:"));
    }

    #[test]
    fn canonical_json_sorts_nested_object_keys() {
        let value = serde_json::json!({
            "z": 1,
            "a": { "b": 2, "a": 1 },
        });

        assert_eq!(canonical_json(&value).unwrap(), r#"{"a":{"a":1,"b":2},"z":1}"#);
    }
}
