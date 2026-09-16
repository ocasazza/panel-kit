//! Independently callable web dock composition part.

use dioxus::prelude::*;
use panel_kit_core::frame::DockProjection;
use panel_kit_core::panel::PanelCatalog;
use panel_kit_core::reducer::WorkspaceEvent;
use panel_kit_core::{PanelCommand, PanelKey};

/// Paint a dock from projected minimized-panel entries.
///
/// `extras` is a host-owned trailing slot for workspace policy controls such
/// as snap toggles and layout reset. Pass `None` when no trailing controls are
/// needed; the dock never owns or persists their state.
///
/// The dock is replace-only composition: it does not mount panels, chrome, or a
/// controller. Clicks translate to core restore commands for the host reducer.
pub fn dock<K: PanelKey>(
    items: &[DockProjection<K>],
    catalog: &PanelCatalog<K>,
    emit: EventHandler<WorkspaceEvent<K>>,
    extras: Option<Element>,
) -> Element {
    rsx! {
        footer { class: "dock",
            span { class: "dock-label", "dock:" }
            if items.is_empty() {
                span { class: "dock-empty", "— nothing minimized —" }
            }
            for item in items.iter().copied() {
                {dock_chip(item, catalog, emit)}
            }
            if let Some(extra) = extras {
                div { class: "dock-extras", {extra} }
            }
        }
    }
}

fn dock_chip<K: PanelKey>(
    item: DockProjection<K>,
    catalog: &PanelCatalog<K>,
    emit: EventHandler<WorkspaceEvent<K>>,
) -> Element {
    let Some(meta) = catalog.get(item.key) else {
        return rsx! {};
    };
    let chip_id = format!("panel-kit-dock-{}", meta.slug);
    let label = format!("Restore {}", meta.title);
    let title = meta.title.clone();

    rsx! {
        button {
            key: "{chip_id}",
            id: "{chip_id}",
            r#type: "button",
            class: "dock-chip",
            aria_label: "{label}",
            onclick: move |_| emit.call(WorkspaceEvent::Command { target: Some(item.key), command: PanelCommand::Restore }),
            "{title}"
        }
    }
}
