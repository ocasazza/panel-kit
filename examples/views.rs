//! Views demo — named layouts over one responsive, V2-persisted workspace.
//!
//! Run with: `dx serve --example views --platform web`
//! (dioxus-cli 0.6.x; provided by `nix develop`).
//!
//! What it demonstrates:
//! - `use_views(base_key, defaults, initial_views)` seeds "Main" and "Focus"
//!   over one workspace, with keyboard and pointer input delegated to it.
//! - The live surface tier, `caps.coarse_pointer`, active view storage key,
//!   and stored schema are visible rather than implicit.
//! - Each view persists at `panel_kit_example_views:view:<name>` as
//!   `SavedLayoutV2`; the registry lives at `panel_kit_example_views:views`.
//! - "migrate a V1 record" writes a real pre-1.0 `{ panels, tiling }` record
//!   to a new view, switches through the production reader, then reports the
//!   schema actually observed after the settled layout is written as V2.
//! - Creating, switching, renaming, deleting, and resetting views all operate
//!   on the same registry. A pre-views layout at the bare base key is copied
//!   into the first view on first run and deliberately left untouched.

use dioxus::events::PointerEvent as DioxusPointerEvent;
use dioxus::prelude::*;
use gloo_storage::{LocalStorage, Storage};
use panel_kit::{
    use_views, LayoutBuilder, Mode, PanelKind, PanelWin, SavedLayout, StoredLayout, SurfaceClass,
    CSS,
};
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

fn stored_schema(key: &str) -> &'static str {
    match LocalStorage::get::<StoredLayout<Panel>>(key) {
        Ok(StoredLayout::V2(_)) => "V2",
        Ok(StoredLayout::V1(_)) => "V1",
        Err(_) => "not stored yet",
    }
}

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    let views = use_views(BASE_KEY, default_layout, &["Main", "Focus"]);
    let ws = views.workspace;
    let mut draft = use_signal(String::new);
    let mut err = use_signal(String::new);
    let mut migration_observation = use_signal(String::new);

    let active = views.active.read().clone();
    let names = views.names.read().clone();
    let active_key = views.layout_key(&active);
    let active_schema = stored_schema(&active_key);
    let profile = ws.surface_profile();
    let surface = match profile.class {
        SurfaceClass::Compact => "compact",
        SurfaceClass::Tablet => "tablet",
        SurfaceClass::Regular => "regular",
    };

    let body = move |kind: Panel, _maximized: bool| -> Element {
        match kind {
            Panel::Notes => rsx! {
                p { "Arrange this view, then switch away and back — every view "
                    "keeps its own V2 layout." }
                textarea { class: "notes", placeholder: "per-view notes…" }
            },
            Panel::Preview => rsx! {
                p { "The workspace is created once by " code { "use_views" }
                    "; switching views swaps its stored layout, not its hooks." }
                p { "Keyboard and pointer events are handled by the workspace "
                    "contract at the root." }
            },
            Panel::Status => {
                let active_now = views.active.read().clone();
                let active_key_now = views.layout_key(&active_now);
                let rows: Vec<String> = views
                    .names
                    .read()
                    .iter()
                    .map(|name| {
                        let marker = if *name == active_now { " (active)" } else { "" };
                        let key = views.layout_key(name);
                        format!("{name} → {key} [{}]{marker}", stored_schema(&key))
                    })
                    .collect();
                rsx! {
                    p {
                        "surface: " b { "{surface}" }
                        " · caps.coarse_pointer: "
                        b { if profile.caps.coarse_pointer { "true" } else { "false" } }
                    }
                    p { "active storage key: " code { "{active_key_now}" } }
                    p { "registry: " code { "{BASE_KEY}:views" } }
                    p { "per-view layout records:" }
                    ul { class: "status-list",
                        for row in rows {
                            li { "{row}" }
                        }
                    }
                    p { "A pre-1.0 V1 record is migrated in memory by the shared "
                        "reader; the settled persistence effect writes it back as V2." }
                    if !migration_observation.read().is_empty() {
                        p { "{migration_observation}" }
                    }
                    p { "Separately, a pre-views record at the bare "
                        code { "{BASE_KEY}" }
                        " key is copied into the first view and never deleted." }
                }
            }
        }
    };

    rsx! {
        style { {CSS} }
        style { {DEMO_CSS} }
        div {
            class: ws.root_class(),
            tabindex: "0",
            onpointermove: move |event: DioxusPointerEvent| ws.handle_pointer_move(&event),
            onpointerup: move |event: DioxusPointerEvent| ws.handle_pointer_up(&event),
            onpointercancel: move |event: DioxusPointerEvent| ws.handle_pointer_up(&event),
            onkeydown: move |event: KeyboardEvent| ws.handle_key(&event),
            header { class: "topbar",
                h1 { "panel-kit views demo" }
                span { class: "hint",
                    "surface: {surface} · caps.coarse_pointer: "
                    if profile.caps.coarse_pointer { "true" } else { "false" }
                    " · active key: {active_key} [{active_schema}]"
                }
                for name in names.iter().cloned() {
                    {
                        let is_active = name == active;
                        let click_name = name.clone();
                        rsx! {
                            button {
                                key: "{name}",
                                class: if is_active { "active-view" } else { "" },
                                onclick: move |_| {
                                    err.set(String::new());
                                    if let Err(error) = views.switch(&click_name) {
                                        err.set(error.to_string());
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
                    oninput: move |event| draft.set(event.value()),
                }
                button {
                    onclick: move |_| {
                        let name = draft.read().clone();
                        match views.create(&name).and_then(|_| views.switch(&name)) {
                            Ok(()) => {
                                draft.set(String::new());
                                err.set(String::new());
                            }
                            Err(error) => err.set(error.to_string()),
                        }
                    },
                    "+ view"
                }
                button {
                    onclick: move |_| {
                        let name = draft.read().clone();
                        match views.rename(&views.active.read().clone(), &name) {
                            Ok(()) => {
                                draft.set(String::new());
                                err.set(String::new());
                            }
                            Err(error) => err.set(error.to_string()),
                        }
                    },
                    "rename active"
                }
                button {
                    onclick: move |_| {
                        let name = views.active.read().clone();
                        if let Err(error) = views.delete(&name) {
                            err.set(error.to_string());
                        } else {
                            err.set(String::new());
                        }
                    },
                    "delete active"
                }
                button {
                    onclick: move |_| {
                        let name = views.active.read().clone();
                        LocalStorage::delete(views.layout_key(&name));
                        let mut panels = ws.panels;
                        panels.set(default_layout());
                        let mut mode = ws.mode;
                        mode.set(Mode::Floating);
                    },
                    "reset view"
                }
                button {
                    onclick: move |_| {
                        let existing = views.names.read().clone();
                        let mut suffix = 1_u32;
                        let name = loop {
                            let candidate = format!("V1 migration {suffix}");
                            if !existing.contains(&candidate) {
                                break candidate;
                            }
                            suffix += 1;
                        };
                        if let Err(error) = views.create(&name) {
                            err.set(error.to_string());
                            return;
                        }
                        let key = views.layout_key(&name);
                        let legacy = SavedLayout {
                            panels: default_layout(),
                            tiling: true,
                        };
                        if let Err(error) = LocalStorage::set(&key, legacy) {
                            err.set(error.to_string());
                            return;
                        }
                        migration_observation.set(format!(
                            "Seeded V1 at {key}; switching through the production reader…"
                        ));
                        if let Err(error) = views.switch(&name) {
                            err.set(error.to_string());
                            return;
                        }
                        spawn(async move {
                            gloo_timers::future::TimeoutFuture::new(75).await;
                            let observed = stored_schema(&key);
                            migration_observation.set(format!(
                                "After read and settle: {key} is observed as {observed}."
                            ));
                        });
                    },
                    "migrate a V1 record"
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
