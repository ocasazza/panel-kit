//! Dioxus painter for the grouped, searchable dropdown model in
//! [`panel_kit_core::widgets::dropdown`].
//!
//! The host owns both the items and popup state. This adapter only translates
//! DOM input into core state transitions and [`DropdownAction`] values.

use dioxus::prelude::*;
use panel_kit_core::widgets::dropdown;

pub use panel_kit_core::widgets::dropdown::{DropdownAction, DropdownItem, DropdownState};

/// Paint a grouped, searchable single-select dropdown.
///
/// `state` is host-owned so parent rerenders cannot reset an open popup and
/// the host can inspect or drive filter and keyboard-navigation state.
#[component]
pub fn Dropdown(
    /// Items in display order. Group ordering follows first appearance.
    items: Vec<DropdownItem>,
    /// Popup state owned by the host.
    state: Signal<DropdownState>,
    /// Currently selected stable value; empty means no selection.
    #[props(default)]
    selected: String,
    /// Trigger text shown when there is no selection.
    #[props(default = "select…".to_string())]
    placeholder: String,
    /// Whether to show the popup filter input.
    #[props(default = true)]
    searchable: bool,
    /// Receives committed selections and popup visibility changes.
    on_action: EventHandler<DropdownAction>,
) -> Element {
    let current_state = state();
    let current_label = items
        .iter()
        .find(|item| item.value == selected)
        .map(|item| item.label.clone())
        .unwrap_or_else(|| {
            if selected.is_empty() {
                placeholder.clone()
            } else {
                selected.clone()
            }
        });
    let visible = dropdown::filter_items(&items, &current_state.query);
    let groups = dropdown::group_items(&visible);
    let root_class = if current_state.open {
        "pk-widget pk-dropdown pk-dropdown-open"
    } else {
        "pk-widget pk-dropdown"
    };

    rsx! {
        div { class: "{root_class}",
            button {
                class: "pk-dropdown-btn",
                r#type: "button",
                aria_haspopup: "listbox",
                aria_expanded: "{current_state.open}",
                onclick: move |_| {
                    let now_open = !state().open;
                    state.set(if now_open {
                        DropdownState::open()
                    } else {
                        DropdownState::default()
                    });
                    on_action.call(DropdownAction::OpenChanged { open: now_open });
                },
                span {
                    class: "pk-dropdown-label",
                    title: "{current_label}",
                    "{current_label}"
                }
                span { class: "pk-dropdown-caret", aria_hidden: "true", "▾" }
            }
            if current_state.open {
                div {
                    class: "pk-dropdown-popup",
                    role: "listbox",
                    tabindex: "0",
                    onkeydown: move |event| {
                        let visible_len = dropdown::filter_items(&items, &state().query).len();
                        match event.key() {
                            Key::ArrowDown => {
                                event.prevent_default();
                                let mut next = state();
                                next.highlighted = dropdown::highlight_next(
                                    next.highlighted,
                                    visible_len,
                                    true,
                                );
                                state.set(next);
                            }
                            Key::ArrowUp => {
                                event.prevent_default();
                                let mut next = state();
                                next.highlighted = dropdown::highlight_next(
                                    next.highlighted,
                                    visible_len,
                                    false,
                                );
                                state.set(next);
                            }
                            Key::Enter => {
                                event.prevent_default();
                                let current = state();
                                if let Some(index) = current.highlighted {
                                    let filtered = dropdown::filter_items(&items, &current.query);
                                    if let Some(item) = filtered.get(index) {
                                        on_action.call(DropdownAction::Select {
                                            value: item.value.clone(),
                                        });
                                        state.set(DropdownState::default());
                                        on_action.call(DropdownAction::OpenChanged { open: false });
                                    }
                                }
                            }
                            Key::Escape => {
                                state.set(DropdownState::default());
                                on_action.call(DropdownAction::OpenChanged { open: false });
                            }
                            _ => {}
                        }
                    },
                    if searchable {
                        input {
                            class: "pk-dropdown-search",
                            r#type: "text",
                            placeholder: "filter…",
                            value: "{current_state.query}",
                            autofocus: true,
                            oninput: move |event| {
                                let mut next = state();
                                next.query = event.value();
                                next.highlighted = None;
                                state.set(next);
                            },
                        }
                    }
                    div { class: "pk-dropdown-list",
                        if groups.is_empty() {
                            div { class: "pk-dropdown-empty", "no matches" }
                        }
                        for (group_index, (group, entries)) in groups.iter().enumerate() {
                            div {
                                class: "pk-dropdown-group",
                                key: "g-{group_index}",
                                if !group.is_empty() {
                                    div { class: "pk-dropdown-group-label", "{group}" }
                                }
                                for item in entries.iter() {
                                    {
                                        let index = visible
                                            .iter()
                                            .position(|visible_item| visible_item.value == item.value)
                                            .unwrap_or(0);
                                        let highlighted = current_state.highlighted == Some(index);
                                        let selected_item = item.value == selected;
                                        let value = item.value.clone();
                                        // Keep the key independent of `value`: the click closure owns
                                        // `value`, while Dioxus also needs a stable key during diffing.
                                        let key_attr = item.value.clone();
                                        let label = item.label.clone();
                                        let item_class = if highlighted {
                                            "pk-dropdown-item highlighted"
                                        } else if selected_item {
                                            "pk-dropdown-item selected"
                                        } else {
                                            "pk-dropdown-item"
                                        };
                                        rsx! {
                                            div {
                                                key: "{key_attr}",
                                                class: "{item_class}",
                                                role: "option",
                                                aria_selected: "{selected_item}",
                                                title: "{label}",
                                                onclick: move |_| {
                                                    on_action.call(DropdownAction::Select {
                                                        value: value.clone(),
                                                    });
                                                    state.set(DropdownState::default());
                                                    on_action.call(DropdownAction::OpenChanged {
                                                        open: false,
                                                    });
                                                },
                                                "{label}"
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
