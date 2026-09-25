//! Dioxus painters for renderer-neutral widget content views.
//!
//! These modules are web adapters only: semantic widget data stays in
//! `panel-kit-core`, while this crate maps borrowed runtime views to DOM.

pub mod badge;
pub mod cascade;
pub mod charts;
pub mod data_table;
pub mod dock;
pub mod dropdown;
pub mod meter;
pub mod panel;
mod panel_layout;
pub mod root;
pub mod scroll;
pub mod spinner;
pub mod status;
pub mod table;

pub use cascade::{CascadeAction, CascadeItem, CascadeState, CascadingDropdown};
pub use data_table::{DataColumnSpec, DataRow, DataTable, SortKey};
pub use dropdown::{Dropdown, DropdownAction, DropdownItem, DropdownState};

use dioxus::prelude::*;
use panel_kit_core::badge::BadgeAction;
use panel_kit_core::widgets::ContentView;

/// Paint one borrowed core content view as semantic Dioxus DOM.
pub fn content_view(
    view: ContentView<'_>,
    spinner_tick: u64,
    on_badge_action: EventHandler<BadgeAction>,
) -> Element {
    match view {
        ContentView::Custom { binding } => rsx! {
            div { class: "pk-content pk-content-custom", role: "group", aria_label: "custom content {binding}", "data-binding": "{binding}" }
        },
        ContentView::Text { text, scroll } => scroll::text(text, scroll),
        ContentView::Editor { binding } => rsx! {
            div { class: "pk-content pk-content-editor", role: "group", aria_label: "editor content {binding}", "data-binding": "{binding}" }
        },
        ContentView::Badges(specs) => badge::strip(specs, on_badge_action),
        ContentView::Table(view) => table::table(view, None, None, false),
        ContentView::TimeSeries { series, unit } => charts::time_series(series, unit),
        ContentView::Gauges(items) => charts::gauges(items),
        ContentView::Flamegraph(spans) => charts::flamegraph(spans),
        ContentView::Boxplot(items) => charts::boxplot(items),
        ContentView::Meter(model) => meter::meter(model),
        ContentView::Status(model) => status::status(model),
        ContentView::Spinner { label } => spinner::spinner(label, spinner_tick),
    }
}

#[cfg(test)]
mod snapshot_expectations;
#[cfg(test)]
mod snapshot_semantics;
#[cfg(test)]
mod tests;
