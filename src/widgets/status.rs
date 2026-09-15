//! Web status painter over the shared core status model.

use dioxus::prelude::*;
use panel_kit_core::widgets::status::{StatusModel, StatusState};

fn state_name(state: StatusState) -> &'static str {
    match state {
        StatusState::Ok => "ok",
        StatusState::Warning => "warning",
        StatusState::Error => "error",
        StatusState::Info => "info",
        StatusState::Unknown => "unknown",
    }
}

fn style(model: &StatusModel) -> String {
    let (r, g, b) = model.color;
    format!("--status-c:rgb({r},{g},{b});")
}

/// Paint a core [`StatusModel`] as a live semantic status readout.
pub fn status(model: &StatusModel) -> Element {
    let state = state_name(model.state);
    let style = style(model);
    rsx! {
        span { class: "pk-widget pk-status pk-status-{state}", role: "status", aria_label: "{model.label} {state}", style: "{style}",
            span { class: "pk-status-dot", aria_hidden: "true", "●" }
            span { class: "pk-status-label", "{model.label}" }
        }
    }
}
