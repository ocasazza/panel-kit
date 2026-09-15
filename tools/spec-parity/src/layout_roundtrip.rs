//! Retained `mkLayout` compatibility checks.

use std::collections::BTreeSet;
use std::error::Error;

use panel_kit_core::SavedLayoutV2;
use serde_json::Value;

use crate::diagnostics::DiagnosticReport;

/// Decode retained `mkLayout` JSON, encode it, decode it again, and prove no fields changed.
pub(crate) fn round_trip_saved_layout(text: &str) -> Result<(), Box<dyn Error>> {
    let original = serde_json::from_str::<Value>(text)?;
    let decoded = serde_json::from_value::<SavedLayoutV2<String>>(original.clone())?;
    let encoded = serde_json::to_value(&decoded)?;
    let decoded_again = serde_json::from_value::<SavedLayoutV2<String>>(encoded)?;
    let round_tripped = serde_json::to_value(&decoded_again)?;

    compare_complete_layout_json(&original, &round_tripped)?;
    Ok(())
}

/// Compare two complete layout JSON objects with deterministic JSON-pointer diagnostics.
fn compare_complete_layout_json(
    original: &Value,
    round_tripped: &Value,
) -> Result<(), DiagnosticReport> {
    let mut lines = vec![
        "spec-parity: mkLayout".to_owned(),
        "layout round-trip changed complete SavedLayoutV2 JSON".to_owned(),
    ];

    append_json_diffs(&mut lines, "", original, round_tripped);
    if lines.len() == 2 {
        Ok(())
    } else {
        Err(DiagnosticReport::new(lines))
    }
}

/// Append recursive differences between two JSON values.
fn append_json_diffs(
    lines: &mut Vec<String>,
    pointer: &str,
    original: &Value,
    round_tripped: &Value,
) {
    match (original, round_tripped) {
        (Value::Object(left), Value::Object(right)) => {
            let keys: BTreeSet<_> = left.keys().chain(right.keys()).collect();
            for key in keys {
                append_optional_json_diff(
                    lines,
                    &join_object_pointer(pointer, key),
                    left.get(key),
                    right.get(key),
                );
            }
        }
        (Value::Array(left), Value::Array(right)) => {
            for index in 0..left.len().max(right.len()) {
                append_optional_json_diff(
                    lines,
                    &join_array_pointer(pointer, index),
                    left.get(index),
                    right.get(index),
                );
            }
        }
        _ if original != round_tripped => lines.push(format!(
            "{}: original={}, round_trip={}",
            pointer,
            format_json_value(Some(original)),
            format_json_value(Some(round_tripped))
        )),
        _ => {}
    }
}

/// Append a value, missing-value, or nested difference at one JSON pointer.
fn append_optional_json_diff(
    lines: &mut Vec<String>,
    pointer: &str,
    original: Option<&Value>,
    round_tripped: Option<&Value>,
) {
    match (original, round_tripped) {
        (Some(left), Some(right)) => append_json_diffs(lines, pointer, left, right),
        _ if original != round_tripped => lines.push(format!(
            "{pointer}: original={}, round_trip={}",
            format_json_value(original),
            format_json_value(round_tripped)
        )),
        _ => {}
    }
}

/// Join an object-key segment to a JSON pointer.
fn join_object_pointer(pointer: &str, segment: &str) -> String {
    format!("{pointer}/{}", escape_json_pointer(segment))
}

/// Join an array index to a JSON pointer.
fn join_array_pointer(pointer: &str, index: usize) -> String {
    format!("{pointer}/{index}")
}

/// Escape one JSON pointer path segment per RFC 6901.
fn escape_json_pointer(segment: &str) -> String {
    segment.replace('~', "~0").replace('/', "~1")
}

/// Format a JSON value for deterministic diagnostics.
fn format_json_value(value: Option<&Value>) -> String {
    value
        .map(|value| serde_json::to_string(value).expect("JSON values serialize"))
        .unwrap_or_else(|| "<missing>".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mk_layout_json_round_trips_as_complete_saved_layout_v2() {
        round_trip_saved_layout(include_str!("../fixtures/live-seven-layout.json"))
            .expect("mkLayout output must remain SavedLayoutV2-compatible");
    }

    #[test]
    fn complete_layout_comparison_rejects_lost_panel_geometry() {
        let original = layout_fixture_value();
        let mut changed = original.clone();
        changed["panels"][0]
            .as_object_mut()
            .expect("panel is an object")
            .remove("w");

        let report = compare_complete_layout_json(&original, &changed)
            .expect_err("lost geometry must fail the complete round-trip proof");

        assert!(report.to_string().contains("/panels/0/w"));
    }

    #[test]
    fn complete_layout_comparison_rejects_changed_viewport() {
        let original = layout_fixture_value();
        let mut changed = original.clone();
        changed["viewport"][0] = serde_json::json!(999.0);

        let report = compare_complete_layout_json(&original, &changed)
            .expect_err("changed viewport must fail the complete round-trip proof");

        assert!(report.to_string().contains("/viewport/0"));
    }

    fn layout_fixture_value() -> serde_json::Value {
        serde_json::from_str(include_str!("../fixtures/live-seven-layout.json"))
            .expect("fixture is valid JSON")
    }
}
