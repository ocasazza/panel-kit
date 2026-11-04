//! Shared form-row painters: label + control on one row.
//!
//! Ported from jump-cannon's layout-panel form rows (`panels/layout.rs`):
//! [`Slider`], [`IntRow`], [`CheckRow`], and [`TextRow`]. Value parsing and
//! clamping live in [`panel_kit_core::widgets::forms`].

use dioxus::prelude::*;
use panel_kit_core::widgets::forms as forms_core;

/// Slider row: range input (optionally log-mapped, like egui's
/// `.logarithmic(true)`) plus a number box for direct entry. The range input
/// reports continuously; the number box commits on change, clamped into
/// `[min, max]`.
#[component]
pub fn Slider(
    /// Label in the row's key column.
    label: String,
    /// Inclusive minimum. Must be positive when `log` is set.
    min: f64,
    /// Inclusive maximum.
    max: f64,
    /// Current value.
    value: f64,
    /// Receives the committed value.
    on: EventHandler<f64>,
    /// Log-map the slider across orders of magnitude.
    #[props(default)]
    log: Option<bool>,
    /// Native tooltip.
    #[props(default)]
    title: Option<String>,
) -> Element {
    let log = log.unwrap_or(false);
    let pos = if log {
        forms_core::log_position(value, min, max)
    } else {
        value
    };
    let (rmin, rmax, rstep) = forms_core::slider_range(min, max, log);
    rsx! {
        div { class: "pk-form-row", title: title.unwrap_or_default(),
            span { class: "pk-form-key", "{label}" }
            input {
                r#type: "range",
                class: "pk-form-range",
                min: "{rmin}",
                max: "{rmax}",
                step: "{rstep}",
                value: "{pos}",
                oninput: move |e| {
                    if let Ok(p) = e.value().parse::<f64>() {
                        let v = if log { forms_core::log_value(p, min, max) } else { p };
                        on.call(v);
                    }
                },
            }
            input {
                r#type: "number",
                class: "pk-form-num",
                value: "{forms_core::fmt_f(value)}",
                onchange: move |e| {
                    if let Some(v) = forms_core::parse_clamped_f64(&e.value(), min, max) {
                        on.call(v);
                    }
                },
            }
        }
    }
}

/// Integer drag-value stand-in (egui `DragValue`): a clamped number box.
#[component]
pub fn IntRow(
    /// Label in the row's key column.
    label: String,
    /// Inclusive minimum.
    min: i64,
    /// Inclusive maximum.
    max: i64,
    /// Current value.
    value: i64,
    /// Receives the committed value.
    on: EventHandler<i64>,
    /// Native tooltip.
    #[props(default)]
    title: Option<String>,
    /// Muted note after the input, e.g. an unit or a consequence.
    #[props(default)]
    note: Option<String>,
) -> Element {
    rsx! {
        div { class: "pk-form-row", title: title.unwrap_or_default(),
            span { class: "pk-form-key", "{label}" }
            input {
                r#type: "number",
                class: "pk-form-num",
                min: "{min}",
                max: "{max}",
                step: "1",
                value: "{value}",
                onchange: move |e| {
                    if let Some(v) = forms_core::parse_clamped_i64(&e.value(), min, max) {
                        on.call(v);
                    }
                },
            }
            if let Some(n) = note {
                span { class: "pk-form-note", "{n}" }
            }
        }
    }
}

/// Checkbox row with an optional trailing text span.
#[component]
pub fn CheckRow(
    /// Label in the row's key column.
    label: String,
    /// Whether the box is checked.
    value: bool,
    /// Receives the new checked state.
    on: EventHandler<bool>,
    /// Text after the box.
    #[props(default)]
    text: Option<String>,
    /// Native tooltip.
    #[props(default)]
    title: Option<String>,
) -> Element {
    rsx! {
        div { class: "pk-form-row", title: title.unwrap_or_default(),
            span { class: "pk-form-key", "{label}" }
            label { class: "pk-form-check",
                input {
                    r#type: "checkbox",
                    checked: value,
                    onchange: move |e| on.call(e.checked()),
                }
                if let Some(t) = text {
                    span { "{t}" }
                }
            }
        }
    }
}

/// Single-line text row; commits on change.
#[component]
pub fn TextRow(
    /// Label in the row's key column.
    label: String,
    /// Current value.
    value: String,
    /// Receives the committed value.
    on: EventHandler<String>,
    /// Native tooltip.
    #[props(default)]
    title: Option<String>,
) -> Element {
    rsx! {
        div { class: "pk-form-row", title: title.unwrap_or_default(),
            span { class: "pk-form-key", "{label}" }
            input {
                r#type: "text",
                class: "pk-form-text",
                value: "{value}",
                onchange: move |e| on.call(e.value()),
            }
        }
    }
}
