//! Views demo — several named panel layouts inside one workspace.
//!
//! Run with: `dx serve --example views --platform web`
//! (dioxus-cli 0.6.x; provided by `nix develop`)
//!
//! What it demonstrates:
//! - `use_views(base_key, defaults, initial_views)` seeding two views
//!   ("Main" and "Focus") over a single workspace — one `ws.render(body)` /
//!   `ws.dock()` pair serves every view.
//! - Per-view layout persistence: drag/resize/minimize panels, switch views
//!   with the topbar buttons, switch back — each view restores its own
//!   arrangement. Layouts persist under `panel_kit_example_views:view:<name>`,
//!   the registry (view list + active view) under `panel_kit_example_views:views`
//!   — reload the page and you land back on the view you left.
//! - View management: create a view from the input, rename or delete the
//!   active view. Renaming moves the stored layout to the new key; deleting
//!   drops it (the last view refuses to die).
//! - Legacy migration: if localStorage holds a pre-views single layout at
//!   the bare base key `panel_kit_example_views` (e.g. saved by the
//!   `workspace` example's `use_workspace` shape), it is copied into the
//!   first view on first run and the legacy key is left untouched.

use dioxus::prelude::*;
use gloo_storage::Storage;
use panel_kit::{use_views, LayoutBuilder, Mode, PanelKind, PanelWin, CSS};
use serde::{Deserialize, Serialize};

/// Base localStorage key: the registry persists at `{BASE_KEY}:views`, each
/// view's layout at `{BASE_KEY}:view:<name>`.
const BASE_KEY: &str = "panel_kit_example_views";

/// Demo styles layered after `panel_kit::CSS`. The view switcher chrome is
/// app territory — the hook ships no UI of its own.
const DEMO_CSS: &str = "
.topbar button { background: var(--bg); color: var(--fg); border: 1px solid var(--line2);
  border-radius: 3px; padding: .15rem .5rem; font-size: .72rem; cursor: pointer; }
.topbar button:hover { border-color: var(--fg); }
.topbar button.active-view { background: var(--inv-bg); color: var(--inv-fg); }
.topbar input { background: var(--bg); color: var(--fg); border: 1px solid var(--line2);
  border-radius: 3px; padding: .15rem .4rem; font-size: .72rem; font-family: var(--mono);
  width: 7rem; }
.topbar .err { color: var(--red); font-size: .72rem; }
.status-list { margin: .25rem 0; padding-left: 1.1rem; }
.status-list li { color: var(--dim); }
textarea.notes { width: 100%; height: 70%; background: var(--bg); color: var(--fg);
  border: 1px solid var(--line2); border-radius: 3px; font-family: var(--mono);
  font-size: .78rem; padding: .4rem; resize: none; }
";

#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
enum Panel {
    Notes,
    Preview,
    Status,
}

impl PanelKind for Panel {
    fn title(self) -> &'static str {
        match self {
            Panel::Notes => "Notes",
            Panel::Preview => "Preview",
            Panel::Status => "Status",
        }
    }
}

/// Default layout for a view's first visit (and the seed for panels added
/// after a view's layout was saved).
fn default_layout() -> Vec<PanelWin<Panel>> {
    let mut b = LayoutBuilder::new();
    vec![
        b.at(Panel::Notes, 16.0, 16.0, 420.0, 300.0),
        b.at(Panel::Preview, 452.0, 16.0, 420.0, 300.0),
        b.at(Panel::Status, 16.0, 332.0, 560.0, 260.0),
    ]
}

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    let views = use_views(BASE_KEY, default_layout, &["Main", "Focus"]);
    let ws = views.workspace;
    // Scratch state for the create/rename input and the last rejected op.
    let mut draft = use_signal(String::new);
    let mut err = use_signal(String::new);

    let active = views.active.read().clone();
    let names = views.names.read().clone();

    let body = move |kind: Panel, _maximized: bool| -> Element {
        match kind {
            Panel::Notes => rsx! {
                p { "Arrange this view, then switch away and back — every view "
                    "keeps its own layout." }
                textarea { class: "notes", placeholder: "per-view notes…" }
            },
            Panel::Preview => rsx! {
                p { "The workspace is created once by " code { "use_views" }
                    "; switching views swaps its stored layout, not its hooks." }
            },
            Panel::Status => {
                let active_now = views.active.read().clone();
                let rows: Vec<String> = views
                    .names
                    .read()
                    .iter()
                    .map(|n| {
                        let marker = if *n == active_now { " (active)" } else { "" };
                        format!("{n} → {}{marker}", views.layout_key(n))
                    })
                    .collect();
                rsx! {
                    p { "registry: " code { "{BASE_KEY}:views" } }
                    p { "per-view layout keys:" }
                    ul { class: "status-list",
                        for r in rows {
                            li { "{r}" }
                        }
                    }
                    p { "A pre-views layout at the bare " code { "{BASE_KEY}" }
                        " key migrates into the first view on first run and is "
                        "never deleted." }
                }
            }
        }
    };

    rsx! {
        style { {CSS} }
        style { {DEMO_CSS} }
        div {
            class: ws.root_class(),
            onmousemove: move |e| ws.handle_mouse_move(&e),
            onmouseup: move |_| ws.handle_mouse_up(),
            header { class: "topbar",
                h1 { "panel-kit views demo" }
                for name in names.iter().cloned() {
                    {
                        let is_active = name == active;
                        rsx! {
                            button {
                                key: "{name}",
                                class: if is_active { "active-view" } else { "" },
                                onclick: move |_| {
                                    err.set(String::new());
                                    if let Err(e) = views.switch(&name) {
                                        err.set(e.to_string());
                                    }
                                },
                                "{name}"
                            }
                        }
                    }
                }
                input {
                    placeholder: "view name",
                    value: "{draft}",
                    oninput: move |e| draft.set(e.value()),
                }
                button {
                    onclick: move |_| {
                        let name = draft.read().clone();
                        match views.create(&name).and_then(|_| views.switch(&name)) {
                            Ok(()) => { draft.set(String::new()); err.set(String::new()); }
                            Err(e) => err.set(e.to_string()),
                        }
                    },
                    "+ view"
                }
                button {
                    onclick: move |_| {
                        let name = draft.read().clone();
                        match views.rename(&views.active.read().clone(), &name) {
                            Ok(()) => { draft.set(String::new()); err.set(String::new()); }
                            Err(e) => err.set(e.to_string()),
                        }
                    },
                    "rename active"
                }
                button {
                    onclick: move |_| {
                        let name = views.active.read().clone();
                        if let Err(e) = views.delete(&name) {
                            err.set(e.to_string());
                        } else {
                            err.set(String::new());
                        }
                    },
                    "delete active"
                }
                button {
                    onclick: move |_| {
                        // Reset the active view: drop its stored layout and
                        // re-seed from the defaults.
                        let name = views.active.read().clone();
                        gloo_storage::LocalStorage::delete(views.layout_key(&name));
                        let mut panels = ws.panels;
                        panels.set(default_layout());
                        let mut mode = ws.mode;
                        mode.set(Mode::Floating);
                    },
                    "reset view"
                }
                if !err.read().is_empty() {
                    span { class: "err", "{err}" }
                }
            }
            {ws.render(body)}
            {ws.dock()}
        }
    }
}
