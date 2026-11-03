//! Clickable neighbour-pill painter over the core pill model.
//!
//! Ported from jump-cannon's inspector `pill_list` (itself a port of the egui
//! `clickable_list`): body-click selects the id the pill represents — no
//! filter affordance, because a pill represents an entity, not a
//! `(field, value)` attribute.

use dioxus::prelude::*;
use panel_kit_core::badge::{BadgeAction, BadgeClickKind, BadgeKind, BadgeSpec};
use panel_kit_core::widgets::selection::Pill;

/// Render a wrapped row of clickable pills. `field` is the badge's filter
/// field name; body clicks route the pill's id through `on_select`. The
/// `… N more not shown` note renders when `truncated > 0`.
pub fn pill_list(
    field: &str,
    pills: Vec<Pill>,
    truncated: usize,
    on_select: EventHandler<String>,
) -> Element {
    rsx! {
        div { class: "pk-pills",
            for p in pills {
                {
                    let id = p.id.clone();
                    let spec = BadgeSpec {
                        small: true,
                        click_kind: BadgeClickKind::Clicked,
                        override_color: p.tint,
                        ..BadgeSpec::new(field, p.label.clone(), BadgeKind::Generic)
                    };
                    rsx! {
                        // The badge's own `title` carries the pill label; wrap it
                        // so the full id + direction reads on hover (the pill
                        // label is the truncated basename).
                        span { class: "pk-pill", key: "{p.id}", title: "{p.tip}",
                            crate::badge::Badge {
                                spec,
                                on_action: move |a| {
                                    if let BadgeAction::Clicked { .. } = a {
                                        on_select.call(id.clone());
                                    }
                                },
                            }
                        }
                    }
                }
            }
        }
        if truncated > 0 {
            div { class: "pk-pill-note", "… {truncated} more not shown" }
        }
    }
}
