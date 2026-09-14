//! Loading demo — the full hydration arc and every store outcome.
//!
//! Run with: `dx serve --example loading --platform web`.
//!
//! The measured pre-workspace phase renders a determinate
//! `LoadingWorkspace`. Once its V2-persisted workspace mounts, Graph reports
//! determinate chunk progress while Branches remains honestly indeterminate
//! until the operator completes or fails it. The visible controls exercise
//! `LoadingStore::succeed`, `LoadingStore::fail`, and retry after failure;
//! the root delegates keyboard and pointer input to `Workspace`.

use dioxus::events::PointerEvent as DioxusPointerEvent;
use dioxus::prelude::*;
use panel_kit::loading::{loading_store, GlobalLoadingBar, LoadingGate};
use panel_kit::{LayoutBuilder, LoadingWorkspace, PanelKind, PanelWin};
use serde::{Deserialize, Serialize};

const DEMO_CSS: &str = "
.load-actions { display: flex; flex-wrap: wrap; gap: .4rem; margin-left: auto; }
.load-actions button { background: var(--bg); color: var(--fg);
  border: 1px solid var(--line2); border-radius: 3px; padding: .15rem .5rem;
  font-family: var(--mono); font-size: .72rem; cursor: pointer; }
.load-actions button:hover { border-color: var(--fg); }
.load-note { color: var(--dim); font-size: .78rem; }
";

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
    let ws = panel_kit::use_workspace("panel_kit_loading_demo", default_layout);
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

    rsx! {
        style { {panel_kit::CSS} }
        style { {DEMO_CSS} }
        div {
            class: ws.root_class(),
            tabindex: "0",
            onpointermove: move |event: DioxusPointerEvent| ws.handle_pointer_move(&event),
            onpointerup: move |event: DioxusPointerEvent| ws.handle_pointer_up(&event),
            onpointercancel: move |event: DioxusPointerEvent| ws.handle_pointer_up(&event),
            onkeydown: move |event: KeyboardEvent| ws.handle_key(&event),
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
            {ws.render(|kind, _maximized| match kind {
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
            {ws.dock()}
        }
    }
}
