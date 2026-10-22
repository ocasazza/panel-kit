//! Grouped, searchable dropdown — the Dioxus rendering of the
//! `panel-kit-core::dropdown` model.
//!
//! A native `<select>` stops scaling past a few dozen entries: no grouping
//! beyond flat `<optgroup>` labels, no filter, no type-ahead. This component
//! renders a button that opens a popup with a search box and grouped items.
//! The state machine (open/close, query, keyboard highlight, filtering,
//! grouping) lives in `panel-kit-core::dropdown`; this module is pixels and
//! events only.
//!
//! # Examples
//!
//! ```no_run
//! use dioxus::prelude::*;
//! use panel_kit::dropdown::{Dropdown, DropdownAction, DropdownItem};
//!
//! # fn ui() -> Element {
//! let items = vec![
//!     DropdownItem { value: "v74-000".into(), label: "real trot".into(), group: "spot-walk-pbt-v74".into() },
//!     DropdownItem { value: "v74-001".into(), label: "velocity tracker".into(), group: "spot-walk-pbt-v74".into() },
//! ];
//! rsx! {
//!     Dropdown {
//!         items,
//!         selected: "v74-000".to_string(),
//!         placeholder: "pick a policy",
//!         on_action: move |a: DropdownAction| {
//!             if let DropdownAction::Select { value } = a {
//!                 // host handles selection…
//!             }
//!         },
//!     }
//! }
//! # }
//! ```

use dioxus::prelude::*;

use panel_kit_core::dropdown::{
    filter_items, group_items, highlight_next, DropdownState,
};
pub use panel_kit_core::dropdown::{DropdownAction, DropdownItem};

/// Grouped, searchable single-select dropdown.
#[component]
pub fn Dropdown(
    /// Items to show, in display order; grouping is derived from each item's
    /// `group` (first-seen order). The host may swap this list live.
    items: Vec<DropdownItem>,
    /// Currently selected value (empty string = nothing selected).
    #[props(default)]
    selected: String,
    /// Button text when nothing is selected.
    #[props(default = "select…".to_string())]
    placeholder: String,
    /// Show the filter search box in the popup. Default true; disable for
    /// short lists where search is noise.
    #[props(default = true)]
    searchable: bool,
    /// Receives every [`DropdownAction`] the widget produces.
    on_action: EventHandler<DropdownAction>,
) -> Element {
    let mut state = use_signal(DropdownState::default);
    let st = state();

    let current_label = items
        .iter()
        .find(|i| i.value == selected)
        .map(|i| i.label.clone())
        .unwrap_or_else(|| if selected.is_empty() { placeholder.clone() } else { selected.clone() });

    let visible = filter_items(&items, &st.query);
    let groups = group_items(&visible);

    let root_class = if st.open { "pk-dropdown pk-dropdown-open" } else { "pk-dropdown" };

    rsx! {
        div { class: "{root_class}",
            button {
                class: "pk-dropdown-btn",
                r#type: "button",
                aria_haspopup: "listbox",
                aria_expanded: "{st.open}",
                onclick: move |_| {
                    let now_open = !state().open;
                    state.set(if now_open { DropdownState::open() } else { DropdownState::default() });
                    on_action.call(DropdownAction::OpenChanged { open: now_open });
                },
                span { class: "pk-dropdown-label", title: "{current_label}", "{current_label}" }
                span { class: "pk-dropdown-caret", "▾" }
            }
            if st.open {
                div {
                    class: "pk-dropdown-popup",
                    role: "listbox",
                    tabindex: "0",
                    onkeydown: move |ev| {
                        let len = filter_items(&items, &state().query).len();
                        match ev.key() {
                            Key::ArrowDown => {
                                ev.prevent_default();
                                let mut s = state();
                                s.highlighted = highlight_next(s.highlighted, len, true);
                                state.set(s);
                            }
                            Key::ArrowUp => {
                                ev.prevent_default();
                                let mut s = state();
                                s.highlighted = highlight_next(s.highlighted, len, false);
                                state.set(s);
                            }
                            Key::Enter => {
                                ev.prevent_default();
                                let s = state();
                                if let Some(i) = s.highlighted {
                                    let vis = filter_items(&items, &s.query);
                                    if let Some(item) = vis.get(i) {
                                        on_action.call(DropdownAction::Select { value: item.value.clone() });
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
                            value: "{st.query}",
                            autofocus: true,
                            oninput: move |ev| {
                                let mut s = state();
                                s.query = ev.value();
                                s.highlighted = None;
                                state.set(s);
                            },
                        }
                    }
                    div { class: "pk-dropdown-list",
                        if groups.is_empty() {
                            div { class: "pk-dropdown-empty", "no matches" }
                        }
                        for (gi, (group, entries)) in groups.iter().enumerate() {
                            div { class: "pk-dropdown-group", key: "g-{gi}",
                                if !group.is_empty() {
                                    div { class: "pk-dropdown-group-label", "{group}" }
                                }
                                for item in entries.iter() {
                                    {
                                        let idx = visible.iter().position(|v| v.value == item.value).unwrap_or(0);
                                        let hl = st.highlighted == Some(idx);
                                        let sel = item.value == selected;
                                        let value = item.value.clone();
                                        let key = item.value.clone();
                                        let label = item.label.clone();
                                        let item_class = if hl {
                                            "pk-dropdown-item highlighted"
                                        } else if sel {
                                            "pk-dropdown-item selected"
                                        } else {
                                            "pk-dropdown-item"
                                        };
                                        rsx! {
                                            div {
                                                key: "{key}",
                                                class: "{item_class}",
                                                role: "option",
                                                aria_selected: "{sel}",
                                                onclick: move |_| {
                                                    on_action.call(DropdownAction::Select { value: value.clone() });
                                                    state.set(DropdownState::default());
                                                    on_action.call(DropdownAction::OpenChanged { open: false });
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
