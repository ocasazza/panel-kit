//! Loading demo — the full hydration arc and every store outcome.
//!
//! Run with: `dx serve --example loading --platform web`.
//!
//! The measured pre-workspace phase renders a determinate
//! `LoadingWorkspace`. Once its V2-persisted workspace mounts, Graph reports
//! determinate chunk progress while Branches remains honestly indeterminate
//! until the operator completes or fails it. The visible controls exercise
//! `LoadingStore::succeed`, `LoadingStore::fail`, and retry after failure;
//! the root translates keyboard and pointer input through the core reducer.

use dioxus::events::PointerEvent as DioxusPointerEvent;
use dioxus::prelude::*;
use panel_kit::loading::{loading_store, GlobalLoadingBar, LoadingGate};
use panel_kit::{LayoutBuilder, LoadingWorkspace, PanelKind, PanelWin};
use panel_kit_core::persist::SavePolicy;
use serde::{Deserialize, Serialize};

#[allow(dead_code)]
#[path = "support/composable_workspace.rs"]
mod composable_workspace;

const DEMO_CSS: &str = "
.load-actions { display: flex; flex-wrap: wrap; gap: .4rem; margin-left: auto; }
.load-actions button { background: var(--bg); color: var(--fg);
  border: 1px solid var(--line2); border-radius: 3px; padding: .15rem .5rem;
  font-family: var(--mono); font-size: .72rem; cursor: pointer; }
.load-actions button:hover { border-color: var(--fg); }
.load-note { color: var(--dim); font-size: .78rem; }
";
/// localStorage key retained for saved-layout compatibility.
///
/// Stable `Panel` serde IDs are unchanged: `Graph`, `Branches`.
const STORAGE_KEY: &str = "panel_kit_loading_demo";

#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
enum Panel {
    Graph,
    Branches,
}

impl PanelKind for Panel {
    fn title(self) -> &'static str {
        match self {
            Panel::Graph => "Graph",
            Panel::Branches => "Branches",
        }
    }
}

fn main() {
    dioxus::launch(App);
}

/// Pretend network: report `steps` increasing fractions over time.
async fn fake_fetch(store: panel_kit::loading::LoadingStore, steps: usize) {
    for i in 1..=steps {
        gloo_timers::future::TimeoutFuture::new(400).await;
        store.update(
            Some(i as f64 / steps as f64),
            Some(format!("chunk {i}/{steps}")),
        );
    }
    store.succeed();
}

fn default_layout() -> Vec<PanelWin<Panel>> {
    let mut b = LayoutBuilder::new();
    vec![
        b.at(Panel::Graph, 16.0, 16.0, 520.0, 360.0),
        b.at(Panel::Branches, 560.0, 16.0, 320.0, 360.0),
    ]
}

#[component]
fn App() -> Element {
    let graph = loading_store("demo-graph", "loading graph…");
    let branches = loading_store("demo-branches", "loading branches…");
    let workspace =
        composable_workspace::use_demo_workspace(STORAGE_KEY, default_layout, SavePolicy::OnSettle);
    composable_workspace::mount_viewport_observer(&workspace);
    let emit = composable_workspace::workspace_event_handler(&workspace);
    let mut booted = use_signal(|| false);
    let mut phase = use_signal(|| 0.0f64);

    // The hook set is unconditional: this one task advances the measured boot
    // phase, mounts the workspace, then starts both store-shaped loads.
    use_future(move || async move {
        for i in 1..=4 {
            gloo_timers::future::TimeoutFuture::new(300).await;
            phase.set(i as f64 / 4.0);
        }
        booted.set(true);
        graph.begin_with("connecting");
        branches.begin_with("waiting for a response with no measurable total");
        fake_fetch(graph, 6).await;
    });

    if !*booted.read() {
        return rsx! {
            LoadingWorkspace {
                title: "PANEL KIT",
                status: "initializing measured stages…",
                progress: *phase.read(),
            }
        };
    }

    let snapshot = workspace.snapshot.read();
    let mut scratch = workspace.scratch.borrow_mut();
    let frame = composable_workspace::project_workspace(&snapshot, &mut scratch);
    let root_class = panel_kit::widgets::root::root_class(&frame);
    let workspace_class = composable_workspace::workspace_area_class(&frame);
    let workspace_style = frame
        .tile_grid
        .map(panel_kit::widgets::root::tile_grid_style)
        .unwrap_or_default();
    let pointer_move_workspace = workspace.clone();
    let pointer_up_workspace = workspace.clone();
    let pointer_cancel_workspace = workspace.clone();
    let key_workspace = workspace.clone();
    let wheel_workspace = workspace.clone();

    rsx! {
        style { {panel_kit::CSS} }
        style { {DEMO_CSS} }
        div {
            class: "{root_class}",
            tabindex: "0",
            onpointermove: move |event: DioxusPointerEvent| {
                composable_workspace::handle_pointer_move(&pointer_move_workspace, &event)
            },
            onpointerup: move |event: DioxusPointerEvent| {
                composable_workspace::handle_pointer_up(&pointer_up_workspace, &event)
            },
            onpointercancel: move |event: DioxusPointerEvent| {
                composable_workspace::handle_pointer_up(&pointer_cancel_workspace, &event)
            },
            onkeydown: move |event: KeyboardEvent| {
                composable_workspace::handle_key(&key_workspace, &event)
            },
            header { class: "topbar",
                h1 { "loading demo" }
                GlobalLoadingBar {}
                div { class: "load-actions",
                    button {
                        onclick: move |_| branches.begin_with(
                            "waiting for a response with no measurable total"
                        ),
                        "retry branches"
                    }
                    button {
                        onclick: move |_| branches.succeed(),
                        "complete branches"
                    }
                    button {
                        onclick: move |_| branches.fail(
                            "simulated branch service failure; choose retry branches"
                        ),
                        "fail branches"
                    }
                }
            }
            div {
                class: "{workspace_class}",
                style: "{workspace_style}",
                onwheel: move |event| composable_workspace::handle_wheel(&wheel_workspace, &event),
                for panel in frame.panels.iter().copied() {
                    if let Some(meta) = workspace.catalog.get(panel.key) {
                        {
                            let panel_class = format!("panel-{}", meta.slug);
                            panel_kit::widgets::panel::panel_shell(panel, Some(&panel_class), rsx! {
                                {panel_kit::widgets::panel::panel_chrome_with_events(
                                    panel,
                                    meta,
                                    emit,
                                    Some(panel_kit::widgets::panel::traffic_lights(panel, emit)),
                                    None,
                                )}
                                {panel_kit::widgets::panel::panel_body(match panel.key {
                                    Panel::Graph => rsx! {
                                        p { class: "load-note",
                                            "Determinate: measured chunks always include a percentage."
                                        }
                                        LoadingGate { store: graph,
                                            div { "graph data rendered here" }
                                        }
                                    },
                                    Panel::Branches => rsx! {
                                        p { class: "load-note",
                                            "Indeterminate: no percentage is invented. With reduced motion, "
                                            "the sweep stops while this label remains."
                                        }
                                        LoadingGate { store: branches,
                                            ul {
                                                li { "main" }
                                                li { "feat/loading-stores" }
                                            }
                                        }
                                    },
                                })}
                                {panel_kit::widgets::panel::resize_grip(panel, emit)}
                            })
                        }
                    }
                }
            }
            {panel_kit::widgets::dock::dock(frame.dock, &workspace.catalog, emit, None)}
        }
    }
}
