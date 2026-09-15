//! Badge-list painter over borrowed core badge specs.

use dioxus::prelude::*;
use panel_kit_core::badge::{BadgeAction, BadgeSpec};

/// Paint a borrowed badge slice as an accessible list of shared badge chips.
pub fn strip(specs: &[BadgeSpec], on_action: EventHandler<BadgeAction>) -> Element {
    rsx! {
        div { class: "pk-widget pk-badge-strip", role: "list", aria_label: "badges",
            for spec in specs.iter() {
                span { class: "pk-badge-item", role: "listitem",
                    {crate::badge::paint_badge(spec, None, on_action)}
                }
            }
        }
    }
}
