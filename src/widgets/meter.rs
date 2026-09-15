//! Web meter painter over the shared core meter model.

use dioxus::prelude::*;
use panel_kit_core::widgets::meter::MeterModel;

fn percent(ratio: f64) -> u32 {
    (ratio.clamp(0.0, 1.0) * 100.0).round() as u32
}

fn color_style(model: &MeterModel) -> String {
    model
        .color
        .map(|(r, g, b)| format!("--meter-c:rgb({r},{g},{b});"))
        .unwrap_or_default()
}

/// Paint a core [`MeterModel`] as an accessible native web progressbar.
pub fn meter(model: &MeterModel) -> Element {
    let pct = percent(model.ratio);
    let style = color_style(model);
    rsx! {
        div { class: "pk-widget pk-meter", role: "progressbar", aria_label: "meter {model.label} {pct}%", aria_valuemin: "0", aria_valuemax: "100", aria_valuenow: "{pct}", style: "{style}",
            span { class: "pk-meter-label", "{model.label}" }
            span { class: "pk-meter-track",
                span { class: "pk-meter-fill", style: "width:{pct}%;" }
            }
            span { class: "pk-meter-value", "{model.text}" }
        }
    }
}
