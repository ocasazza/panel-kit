//! Cascading (Miller-columns) dropdown — the Dioxus rendering of the
//! `panel-kit-core::cascade` model.
//!
//! One column per hierarchy level: picking a category opens the next column,
//! picking a leaf commits. Use it when the group choice is itself the first
//! selection (robot -> campaign -> experiment), where a flat grouped list
//! forces the user to scroll past whole categories they don't want.
//!
//! # Examples
//!
//! ```no_run
//! use dioxus::prelude::*;
//! use panel_kit::cascade::{CascadeAction, CascadeItem, CascadeState, CascadingDropdown};
//!
//! # fn ui() -> Element {
//! let items = vec![
//!     CascadeItem { path: vec!["spot".into(), "v74".into()], value: "spot-walk-pbt-v74-000".into(), label: "000 — real trot".into() },
//!     CascadeItem { path: vec!["spider".into()], value: "spider".into(), label: "spider (sandbox)".into() },
//! ];
//! let state = use_signal(CascadeState::default);
//! rsx! {
//!     CascadingDropdown {
//!         items,
//!         state,
//!         selected_path: vec!["spot".to_string(), "v74".to_string()],
//!         selected_label: "000 — real trot".to_string(),
//!         placeholder: "pick a robot / policy",
//!         on_action: move |a: CascadeAction| {
//!             if let CascadeAction::Select { path, value } = a {
//!                 // host handles selection…
//!             }
//!         },
//!     }
//! }
//! # }
//! ```
use dioxus::prelude::*;
use panel_kit_core::cascade::column;
pub use panel_kit_core::cascade::{CascadeAction, CascadeItem, CascadeState};

/// Cascading multi-column single-select.
///
/// The popup state is HOST-OWNED (`state` prop): the component is a pure
/// function of props, so parent re-renders can never reset an open cascade
/// mid-navigation, and the host can open/close/drive it programmatically.
#[component]
pub fn CascadingDropdown(
    /// Leaf items, each carrying its ancestor category path. The host may
    /// swap this list live.
    items: Vec<CascadeItem>,
    /// Popup state (open/descend path/highlight), owned by the host.
    state: Signal<CascadeState>,
    /// Ancestor path of the current selection (breadcrumb under the button).
    #[props(default)]
    selected_path: Vec<String>,
    /// Leaf label of the current selection (last breadcrumb segment).
    #[props(default)]
    selected_label: String,
    /// Button text when nothing is selected.
    #[props(default = "select…".to_string())]
    placeholder: String,
    /// Show `path / label` in the closed button (default) or just the leaf.
    /// Hosts in narrow panels set false — the full path stays in the tooltip.
    #[props(default = true)]
    show_breadcrumb: bool,
    /// Receives every [`CascadeAction`] the widget produces.
    on_action: EventHandler<CascadeAction>,
) -> Element {
    let st = state();

    let button_label = if selected_label.is_empty() {
        placeholder.clone()
    } else if show_breadcrumb {
        let mut crumbs = selected_path.clone();
        crumbs.push(selected_label.clone());
        crumbs.join(" / ")
    } else {
        selected_label.clone()
    };
    // Hoist everything rsx closures need into plain locals (the signal state
    // is not Copy, and move closures can't share captured Vecs).
    let open = st.open;
    let path = st.path.clone();
    let highlighted = st.highlighted;
    let depth = path.len();
    let cols = panel_kit_core::cascade::columns(&items, &path);
    let items_for_keys = items.clone();

    let root_class = if open { "pk-dropdown pk-cascade pk-dropdown-open" } else { "pk-dropdown pk-cascade" };

    rsx! {
        div { class: "{root_class}",
            button {
                class: "pk-dropdown-btn",
                r#type: "button",
                aria_haspopup: "listbox",
                aria_expanded: "{open}",
                onclick: move |_| {
                    let now_open = !state().open;
                    state.set(if now_open { CascadeState::open() } else { CascadeState::default() });
                    on_action.call(CascadeAction::OpenChanged { open: now_open });
                },
                span { class: "pk-dropdown-label", title: "{button_label}", "{button_label}" }
                span { class: "pk-dropdown-caret", "▾" }
            }
            if open {
                div {
                    class: "pk-cascade-popup",
                    role: "listbox",
                    tabindex: "0",
                    onkeydown: move |ev| {
                        let entries = column(&items_for_keys, &state().path);
                        let len = entries.len();
                        match ev.key() {
                            Key::ArrowDown | Key::ArrowUp => {
                                ev.prevent_default();
                                let mut s = state();
                                s.highlighted = panel_kit_core::dropdown::highlight_next(
                                    s.highlighted,
                                    len,
                                    ev.key() == Key::ArrowDown,
                                );
                                state.set(s);
                            }
                            Key::ArrowRight | Key::Enter => {
                                ev.prevent_default();
                                let s = state();
                                if let Some(i) = s.highlighted {
                                    if let Some(entry) = entries.get(i) {
                                        if entry.has_children {
                                            let mut s = s;
                                            s.descend(entry.key.clone());
                                            state.set(s);
                                        } else if let Some(value) = &entry.value {
                                            let path = s.path.clone();
                                            on_action.call(CascadeAction::Select {
                                                path,
                                                value: value.clone(),
                                            });
                                            state.set(CascadeState::default());
                                            on_action.call(CascadeAction::OpenChanged { open: false });
                                        }
                                    }
                                }
                            }
                            Key::ArrowLeft => {
                                ev.prevent_default();
                                let mut s = state();
                                s.ascend();
                                state.set(s);
                            }
                            Key::Escape => {
                                state.set(CascadeState::default());
                                on_action.call(CascadeAction::OpenChanged { open: false });
                            }
                            _ => {}
                        }
                    },
                    for (ci, entries) in cols.iter().enumerate() {
                        div { class: "pk-cascade-col", key: "c-{ci}-{depth}",
                            if entries.is_empty() {
                                div { class: "pk-cascade-empty", "empty" }
                            }
                            for (idx, entry) in entries.iter().enumerate() {
                                {
                                    let is_active_col = ci == depth;
                                    let hl = is_active_col && highlighted == Some(idx);
                                    let on_path = path.get(ci).map(|s| *s == entry.key).unwrap_or(false);
                                    let class = if hl {
                                        "pk-cascade-item highlighted"
                                    } else if on_path {
                                        "pk-cascade-item selected"
                                    } else {
                                        "pk-cascade-item"
                                    };
                                    let entry = entry.clone();
                                    // Hoisted for rsx: the onclick move closure
                                    // owns `entry`, so key/label need copies.
                                    let key_attr = entry.key.clone();
                                    let label = entry.label.clone();
                                    rsx! {
                                        div {
                                            key: "{key_attr}",
                                            class: "{class}",
                                            role: "option",
                                            aria_selected: "{hl}",
                                            title: "{label}",
                                            onclick: move |_| {
                                                if entry.has_children {
                                                    // Descend from THIS column's depth so clicking
                                                    // an earlier column re-navigates correctly.
                                                    {
                                                        let mut s = state.write();
                                                        s.path.truncate(ci);
                                                        s.path.push(entry.key.clone());
                                                        s.highlighted = None;
                                                    }
                                                } else if let Some(value) = entry.value.clone() {
                                                    let mut p = state().path.clone();
                                                    p.truncate(ci);
                                                    on_action.call(CascadeAction::Select { path: p, value });
                                                    state.set(CascadeState::default());
                                                    on_action.call(CascadeAction::OpenChanged { open: false });
                                                }
                                            },
                                            span { "{label}" }
                                            if entry.has_children {
                                                span { class: "pk-cascade-chevron", "▸" }
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
