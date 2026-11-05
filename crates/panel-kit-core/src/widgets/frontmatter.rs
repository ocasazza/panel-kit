//! Frontmatter leftover-grid selection logic.
//!
//! Ported from jump-cannon's inspector (`panels/inspector.rs`, itself a port
//! of the egui `frontmatter_grid.rs`): the badge chip walker renders the
//! chip-shaped subset of a node's frontmatter; this module decides which rows
//! are LEFT OVER, so a renderer can keep the surface honest about the full
//! page metadata instead of only the chip-shaped subset.
//!
//! Serialized frontmatter is a host wire format, so this module is only built
//! when the crate has `serde_json` (the `spec-json` feature, which the web
//! runtime enables).

use serde_json::Value;

/// Keys already promoted to typed fields — mirrors the egui
/// `frontmatter_grid.rs` SKIP_KEYS (NOT the chip walker's list, which also
/// skips "aliases"; the two surfaces keep distinct lists by design).
pub const GRID_SKIP_KEYS: &[&str] = &["tags", "tag", "doctype", "folder", "title", "id", "path"];

/// Whether `key` is skipped case-insensitively by the leftover grid.
pub fn grid_skipped(key: &str) -> bool {
    GRID_SKIP_KEYS.iter().any(|k| k.eq_ignore_ascii_case(key))
}

/// True when the chip walker would emit at least one chip for this value, so
/// the grid must NOT also render the row — port of
/// `frontmatter_grid.rs::chip_walker_handles`.
pub fn chip_walker_handles(value: &Value) -> bool {
    match value {
        Value::String(s) => {
            let t = s.trim();
            !t.is_empty() && t.chars().count() <= 120
        }
        Value::Array(arr) => arr
            .iter()
            .any(|v| matches!(v, Value::String(_) | Value::Number(_) | Value::Bool(_))),
        Value::Number(_) | Value::Bool(_) => true,
        Value::Null | Value::Object(_) => false,
    }
}

/// The rows the chip walker did NOT render, in JSON order: `None` when the
/// JSON is absent, malformed, `{}`, `null`, or when every value is already a
/// chip. Port of `show_frontmatter_section` + `show_frontmatter_grid` row
/// selection — the renderer draws a collapsing grid over exactly these rows.
pub fn leftover_rows(frontmatter_json: &str) -> Option<Vec<(String, Value)>> {
    if frontmatter_json.is_empty() || frontmatter_json == "{}" || frontmatter_json == "null" {
        return None;
    }
    let map =
        serde_json::from_str::<serde_json::Map<String, Value>>(frontmatter_json).ok()?;
    let rows: Vec<(String, Value)> = map
        .into_iter()
        .filter(|(k, v)| !grid_skipped(k) && !chip_walker_handles(v))
        .collect();
    (!rows.is_empty()).then_some(rows)
}

#[cfg(test)]
mod tests {
    use super::{chip_walker_handles, grid_skipped, leftover_rows};
    use serde_json::json;

    #[test]
    fn skip_keys_match_case_insensitively() {
        assert!(grid_skipped("tags"));
        assert!(grid_skipped("Doctype"));
        assert!(!grid_skipped("aliases"));
        assert!(!grid_skipped("custom-field"));
    }

    #[test]
    fn chip_walker_covers_only_chip_shapes() {
        assert!(chip_walker_handles(&json!("plain")));
        assert!(chip_walker_handles(&json!(3)));
        assert!(chip_walker_handles(&json!(true)));
        assert!(chip_walker_handles(&json!(["a", 1])));
        assert!(!chip_walker_handles(&json!("")));
        assert!(!chip_walker_handles(&json!("a string longer than the one hundred and twenty character chip budget would ever allow for a single inline pill badge row")));
        assert!(!chip_walker_handles(&json!({ "nested": true })));
        assert!(!chip_walker_handles(&json!([[[1]]])));
        assert!(!chip_walker_handles(&json!(null)));
    }

    #[test]
    fn leftover_rows_select_the_unhandled_and_reject_the_rest() {
        // A string <= 120 chars is chip-handled, so "note" needs a value
        // longer than the chip budget to stay in the leftover grid.
        let fm = format!(
            r#"{{"title":"kept out","note":"{pad}","cfg":{{"a":1}},"mix":[[1],[2]],"n":null}}"#,
            pad = "x".repeat(121)
        );
        let rows = leftover_rows(&fm).unwrap();
        assert_eq!(
            rows.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>(),
            ["note", "cfg", "mix", "n"]
        );
        // Absent / malformed / already-all-chips inputs render no grid.
        assert_eq!(leftover_rows(""), None);
        assert_eq!(leftover_rows("{}"), None);
        assert_eq!(leftover_rows("null"), None);
        assert_eq!(leftover_rows("{bad json"), None);
        assert_eq!(leftover_rows(r#"{"title":"x","tags":["a"]}"#), None);
    }
}
