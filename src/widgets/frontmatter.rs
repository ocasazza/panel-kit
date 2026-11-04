//! Frontmatter leftover-grid painter over the core row selection.
//!
//! Ported from jump-cannon's inspector `frontmatter_grid` + `fm_value_cell`
//! (ports of the egui `frontmatter_grid.rs`): long text in a bounded
//! scroller, nulls as an em-dash, nested JSON pretty-printed. Which rows are
//! leftover is decided by [`panel_kit_core::widgets::frontmatter`].

use dioxus::prelude::*;
use panel_kit_core::widgets::frontmatter::leftover_rows;
use serde_json::Value;

/// The collapsed "Frontmatter" section for everything the chip walker did
/// NOT render — long-form strings, nested objects, mixed arrays, nulls.
/// Keeps the surface honest about the full page metadata, not just the
/// chip-shaped subset. The collapsing header is not drawn when every
/// frontmatter value is already a chip, or the JSON is absent or malformed.
pub fn frontmatter_grid(frontmatter_json: &str) -> Element {
    let Some(rows) = leftover_rows(frontmatter_json) else {
        return rsx! {};
    };
    rsx! {
        details { class: "pk-fm-section",
            summary { "Frontmatter" }
            div { class: "pk-fmgrid",
                for (key, value) in rows {
                    div { class: "pk-fmrow", key: "{key}",
                        span { class: "pk-fmkey", "{key}" }
                        { fm_value_cell(&value) }
                    }
                }
            }
        }
    }
}

/// One leftover-frontmatter value cell: long text in a bounded scroller,
/// nulls as an em-dash, nested JSON pretty-printed. Scalars are chip-walker
/// territory — unreachable under normal flow, but painted as text.
pub fn fm_value_cell(value: &Value) -> Element {
    match value {
        Value::String(s) => rsx! { div { class: "pk-fmtext", "{s}" } },
        Value::Null => rsx! { span { class: "pk-pill-note", "—" } },
        Value::Object(_) | Value::Array(_) => {
            let pretty =
                serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string());
            rsx! { pre { class: "pk-fmjson", "{pretty}" } }
        }
        // Scalars are chip-walker territory — unreachable under normal flow.
        Value::Number(n) => rsx! { span { class: "pk-fmtext", "{n}" } },
        Value::Bool(b) => {
            rsx! { span { class: "pk-fmtext", { if *b { "true" } else { "false" } } } }
        }
    }
}
