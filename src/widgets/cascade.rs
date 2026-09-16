//! Dioxus Miller-columns painter over the renderer-neutral cascade model.
//!
//! The host owns the items, selection, and [`CascadeState`]. This adapter
//! translates DOM input into core state transitions and [`CascadeAction`]
//! values without retaining controller state.

use dioxus::prelude::*;
use panel_kit_core::widgets::{cascade, dropdown};

pub use panel_kit_core::widgets::cascade::{CascadeAction, CascadeItem, CascadeState};

/// Paint a cascading multi-column single-select.
///
/// `state` remains host-owned so rerenders cannot reset an open cascade and
/// the host can inspect or drive navigation programmatically.
#[component]
pub fn CascadingDropdown(
    /// Leaf items carrying their ancestor category paths.
    items: Vec<CascadeItem>,
    /// Popup and navigation state owned by the host.
    state: Signal<CascadeState>,
    /// Ancestor path of the current selection.
    #[props(default)]
    selected_path: Vec<String>,
    /// Human label of the current leaf selection.
    #[props(default)]
    selected_label: String,
    /// Trigger text shown when there is no selection.
    #[props(default = "select…".to_string())]
    placeholder: String,
    /// Show the full `path / leaf` breadcrumb in the closed trigger.
    ///
    /// When false, only the leaf is visible and the full breadcrumb remains in
    /// the trigger tooltip.
    #[props(default = true)]
    show_breadcrumb: bool,
    /// Receives committed selections and popup visibility changes.
    on_action: EventHandler<CascadeAction>,
) -> Element {
    let current_state = state();
    let full_selection_label = if selected_label.is_empty() {
        placeholder.clone()
    } else {
        let mut crumbs = selected_path.clone();
        crumbs.push(selected_label.clone());
        crumbs.join(" / ")
    };
    let button_label = if selected_label.is_empty() || show_breadcrumb {
        full_selection_label.clone()
    } else {
        selected_label.clone()
    };

    // Hoist values shared by rsx move closures. Signal-backed state itself is
    // not retained by this painter.
    let open = current_state.open;
    let path = current_state.path.clone();
    let highlighted = current_state.highlighted;
    let depth = path.len();
    let open_columns = cascade::columns(&items, &path);
    let items_for_keyboard = items.clone();
    let root_class = if open {
        "pk-widget pk-dropdown pk-cascade pk-dropdown-open"
    } else {
        "pk-widget pk-dropdown pk-cascade"
    };

    rsx! {
        div { class: "{root_class}",
            button {
                class: "pk-dropdown-btn",
                r#type: "button",
                aria_haspopup: "listbox",
                aria_expanded: "{open}",
                onclick: move |_| {
                    let now_open = !state().open;
                    state.set(if now_open {
                        CascadeState::open()
                    } else {
                        CascadeState::default()
                    });
                    on_action.call(CascadeAction::OpenChanged { open: now_open });
                },
                span {
                    class: "pk-dropdown-label",
                    title: "{full_selection_label}",
                    "{button_label}"
                }
                span { class: "pk-dropdown-caret", aria_hidden: "true", "▾" }
            }
            if open {
                div {
                    class: "pk-cascade-popup",
                    role: "listbox",
                    tabindex: "0",
                    onkeydown: move |event| {
                        let entries = cascade::column(&items_for_keyboard, &state().path);
                        let entry_count = entries.len();
                        match event.key() {
                            Key::ArrowDown | Key::ArrowUp => {
                                event.prevent_default();
                                let mut next = state();
                                next.highlighted = dropdown::highlight_next(
                                    next.highlighted,
                                    entry_count,
                                    event.key() == Key::ArrowDown,
                                );
                                state.set(next);
                            }
                            Key::ArrowRight | Key::Enter => {
                                event.prevent_default();
                                let current = state();
                                if let Some(index) = current.highlighted {
                                    if let Some(entry) = entries.get(index) {
                                        if entry.has_children {
                                            let mut next = current;
                                            next.descend(entry.key.clone());
                                            state.set(next);
                                        } else if let Some(value) = &entry.value {
                                            on_action.call(CascadeAction::Select {
                                                path: current.path.clone(),
                                                value: value.clone(),
                                            });
                                            state.set(CascadeState::default());
                                            on_action.call(CascadeAction::OpenChanged {
                                                open: false,
                                            });
                                        }
                                    }
                                }
                            }
                            Key::ArrowLeft => {
                                event.prevent_default();
                                let mut next = state();
                                next.ascend();
                                state.set(next);
                            }
                            Key::Escape => {
                                state.set(CascadeState::default());
                                on_action.call(CascadeAction::OpenChanged { open: false });
                            }
                            _ => {}
                        }
                    },
                    for (column_index, entries) in open_columns.iter().enumerate() {
                        div {
                            class: "pk-cascade-col",
                            key: "c-{column_index}-{depth}",
                            if entries.is_empty() {
                                div { class: "pk-cascade-empty", "empty" }
                            }
                            for (entry_index, entry) in entries.iter().enumerate() {
                                {
                                    let active_column = column_index == depth;
                                    let entry_highlighted =
                                        active_column && highlighted == Some(entry_index);
                                    let on_path = path
                                        .get(column_index)
                                        .map(|segment| *segment == entry.key)
                                        .unwrap_or(false);
                                    let item_class = if entry_highlighted {
                                        "pk-cascade-item highlighted"
                                    } else if on_path {
                                        "pk-cascade-item selected"
                                    } else {
                                        "pk-cascade-item"
                                    };
                                    let entry = entry.clone();
                                    // The click closure owns `entry`; use independent key and
                                    // label values so rendering never moves its selection value.
                                    let key_attr = entry.key.clone();
                                    let label = entry.label.clone();
                                    let has_children = entry.has_children;
                                    rsx! {
                                        div {
                                            key: "{key_attr}",
                                            class: "{item_class}",
                                            role: "option",
                                            aria_selected: "{entry_highlighted}",
                                            title: "{label}",
                                            onclick: move |_| {
                                                if has_children {
                                                    // Re-navigate from the clicked column, including
                                                    // clicks in an earlier already-open column.
                                                    {
                                                        let mut next = state.write();
                                                        next.path.truncate(column_index);
                                                        next.path.push(entry.key.clone());
                                                        next.highlighted = None;
                                                    }
                                                } else if let Some(value) = entry.value.clone() {
                                                    let mut selected_path = state().path.clone();
                                                    selected_path.truncate(column_index);
                                                    on_action.call(CascadeAction::Select {
                                                        path: selected_path,
                                                        value,
                                                    });
                                                    state.set(CascadeState::default());
                                                    on_action.call(CascadeAction::OpenChanged {
                                                        open: false,
                                                    });
                                                }
                                            },
                                            span { "{label}" }
                                            if has_children {
                                                span {
                                                    class: "pk-cascade-chevron",
                                                    aria_hidden: "true",
                                                    "▸"
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
