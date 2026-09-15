//! Web spinner painter over the shared core spinner model.

use dioxus::prelude::*;
use panel_kit_core::widgets::spinner::{spinner_frame, SpinnerModel};

/// Paint an authored [`SpinnerModel`] with a deterministic animation frame.
pub fn from_model(model: &SpinnerModel, tick: u64) -> Element {
    spinner(model.label.as_deref().unwrap_or_default(), tick)
}

/// Paint a borrowed spinner label with a deterministic core-selected frame.
pub fn spinner(label: &str, tick: u64) -> Element {
    let frame = spinner_frame(tick);
    let aria = if label.is_empty() { "loading" } else { label };
    rsx! {
        span { class: "pk-widget spinner", role: "status", aria_live: "polite", aria_label: "{aria}",
            span { class: "spin-ring", aria_hidden: "true", "{frame}" }
            if !label.is_empty() {
                span { class: "spin-label", "{label}" }
            }
        }
    }
}
