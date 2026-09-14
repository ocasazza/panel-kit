//! Loading-workspace handoff demo — the pre-WASM shell's Dioxus twin hands
//! off to a real V2-persisted workspace.
//!
//! Run with: `dx serve --example loading_workspace --platform web`.
//!
//! Toggle between an honest indeterminate phase and a measured 42% phase,
//! then finish loading. The mounted workspace exposes its live surface tier
//! and `coarse_pointer` capability and delegates keyboard and pointer input
//! through `Workspace`. The note also makes the reduced-motion contract
//! visible: motion can stop without inventing progress.

use dioxus::events::PointerEvent as DioxusPointerEvent;
use dioxus::prelude::*;
use panel_kit::{LayoutBuilder, PanelKind, PanelWin, SurfaceClass};
use serde::{Deserialize, Serialize};

const DEMO_CSS: &str = "
.boot-demo-controls { position: fixed; z-index: 2147483001; right: 14px; bottom: 14px;
  display: flex; flex-wrap: wrap; align-items: center; gap: .4rem; max-width: 42rem;
  border: 1px solid var(--line2); border-radius: 3px; padding: .5rem .65rem;
  background: var(--bg); color: var(--fg); }
.boot-demo-controls p { flex-basis: 100%; margin: 0; color: var(--dim); font-size: .78rem; }
.boot-demo-controls button, .topbar button { border: 1px solid var(--line2);
  border-radius: 3px; padding: .2rem .55rem; background: var(--bg); color: var(--fg);
  font-family: var(--mono); font-size: .72rem; cursor: pointer; }
.boot-demo-controls button:hover, .topbar button:hover { border-color: var(--fg); }
.boot-demo-controls button.on { color: var(--fg); border-color: var(--accent); }
.ready-state { color: var(--dim); }
";

#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
enum Panel {
    Handoff,
}

impl PanelKind for Panel {
    fn title(self) -> &'static str {
        "Handoff"
    }
}

fn default_layout() -> Vec<PanelWin<Panel>> {
    let mut builder = LayoutBuilder::new();
    vec![builder.at(Panel::Handoff, 16.0, 16.0, 620.0, 320.0)]
}

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    let ws = panel_kit::use_workspace("panel_kit_loading_workspace_demo", default_layout);
    let mut ready = use_signal(|| false);
    let mut measured = use_signal(|| false);
    let profile = ws.surface_profile();
    let surface = match profile.class {
        SurfaceClass::Compact => "compact",
        SurfaceClass::Tablet => "tablet",
        SurfaceClass::Regular => "regular",
    };

    rsx! {
        style { {panel_kit::CSS} }
        style { {DEMO_CSS} }
        if !*ready.read() {
            panel_kit::LoadingWorkspace {
                title: "PANEL KIT",
                status: if *measured.read() {
                    "loading measured assets…"
                } else {
                    "discovering workspace resources…"
                },
                progress: if *measured.read() { Some(0.42) } else { None },
            }
            aside { class: "boot-demo-controls",
                p {
                    "Indeterminate means the total is unknown and shows no percentage. "
                    "Reduced motion stops its sweep but preserves that honest state."
                }
                p {
                    "pending surface: {surface} · caps.coarse_pointer: "
                    if profile.caps.coarse_pointer { "true" } else { "false" }
                }
                button {
                    class: if !*measured.read() { "on" } else { "" },
                    r#type: "button",
                    onclick: move |_| measured.set(false),
                    "unknown total"
                }
                button {
                    class: if *measured.read() { "on" } else { "" },
                    r#type: "button",
                    onclick: move |_| measured.set(true),
                    "measured: 42%"
                }
                button {
                    r#type: "button",
                    onclick: move |_| ready.set(true),
                    "finish loading"
                }
            }
        } else {
            div {
                class: ws.root_class(),
                tabindex: "0",
                onpointermove: move |event: DioxusPointerEvent| ws.handle_pointer_move(&event),
                onpointerup: move |event: DioxusPointerEvent| ws.handle_pointer_up(&event),
                onpointercancel: move |event: DioxusPointerEvent| ws.handle_pointer_up(&event),
                onkeydown: move |event: KeyboardEvent| ws.handle_key(&event),
                header { class: "topbar",
                    h1 { "loading workspace handoff" }
                    span { class: "hint",
                        "surface: {surface} · caps.coarse_pointer: "
                        if profile.caps.coarse_pointer { "true" } else { "false" }
                    }
                    button {
                        onclick: move |_| ready.set(false),
                        "show loading shell"
                    }
                }
                {ws.render(|Panel::Handoff, _maximized| rsx! {
                    p { "The post-WASM loading surface has handed off to the same "
                        "workspace contract the application will keep." }
                    p { class: "ready-state",
                        "This panel uses V2 layout persistence. Resize the viewport to "
                        "watch the surface tier and pointer capability update above."
                    }
                })}
                {ws.dock()}
            }
        }
    }
}
