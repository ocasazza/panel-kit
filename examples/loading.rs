//! The hydration arc, end to end: static boot fragment (pre-WASM, see
//! `assets/panel-kit-boot.html`) → `LoadingWorkspace` with a measured
//! percentage → workspace chrome visible immediately while each panel's data
//! lazy-loads behind a store, surfaced per-panel (`LoadingGate`) and globally
//! (`GlobalLoadingBar`).
//!
//! Run with `dx serve --example loading --platform web`.

use dioxus::prelude::*;
use panel_kit::loading::{loading_store, GlobalLoadingBar, LoadingGate};
use panel_kit::{LayoutBuilder, LoadingWorkspace, PanelKind, PanelWin};
use serde::{Deserialize, Serialize};

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

#[component]
fn App() -> Element {
    // Phase 1: app-owned init with measurable stages drives the page-level
    // loading bar (0% → 100%), then the workspace chrome mounts.
    let mut booted = use_signal(|| false);
    let mut phase = use_signal(|| 0.0f64);
    use_future(move || async move {
        for i in 1..=4 {
            gloo_timers::future::TimeoutFuture::new(300).await;
            phase.set(i as f64 / 4.0);
        }
        booted.set(true);
    });

    if !*booted.read() {
        return rsx! {
            LoadingWorkspace {
                title: "PANEL KIT",
                status: "initializing…",
                progress: *phase.read(),
            }
        };
    }

    // Phase 2: chrome first, data lazily. Each panel's store feeds both its
    // gate and the global bar — the page is interactive while data streams.
    let graph = loading_store("demo-graph", "loading graph…");
    let branches = loading_store("demo-branches", "loading branches…");
    use_future(move || async move {
        graph.begin_with("connecting");
        fake_fetch(graph, 6).await;
    });
    use_future(move || async move {
        branches.begin();
        fake_fetch(branches, 3).await;
    });

    fn defaults() -> Vec<PanelWin<Panel>> {
        let mut b = LayoutBuilder::new();
        vec![
            b.at(Panel::Graph, 16.0, 16.0, 520.0, 360.0),
            b.at(Panel::Branches, 560.0, 16.0, 320.0, 360.0),
        ]
    }
    let ws = panel_kit::use_workspace("panel_kit_loading_demo", defaults);

    rsx! {
        style { {panel_kit::CSS} }
        div { class: ws.root_class(),
            header { class: "topbar",
                h1 { "loading demo" }
                GlobalLoadingBar {}
            }
            {ws.render(|kind, _maximized| match kind {
                Panel::Graph => rsx! {
                    LoadingGate { store: graph,
                        div { style: "padding: 1rem", "graph data rendered here" }
                    }
                },
                Panel::Branches => rsx! {
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
