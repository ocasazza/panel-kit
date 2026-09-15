//! Loading-workspace handoff demo — the pre-WASM shell's Dioxus twin hands
//! off to a real V2-persisted workspace.
//!
//! Run with: `dx serve --example loading_workspace --platform web`.
//!
//! Toggle between an honest indeterminate phase and a measured 42% phase,
//! then finish loading. The mounted workspace exposes its live surface tier
//! and `coarse_pointer` capability and translates keyboard/pointer input
//! through the host-owned reducer. The note also makes the reduced-motion contract
//! visible: motion can stop without inventing progress.

use dioxus::events::PointerEvent as DioxusPointerEvent;
use dioxus::prelude::*;
use panel_kit::{LayoutBuilder, PanelKind, PanelWin};
use panel_kit_core::persist::SavePolicy;
use serde::{Deserialize, Serialize};

#[path = "support/composable_workspace.rs"]
mod composable_workspace;

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
/// localStorage key retained for saved-layout compatibility.
///
/// Stable `Panel` serde ID is unchanged: `Handoff`.
const STORAGE_KEY: &str = "panel_kit_loading_workspace_demo";


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

fn surface_label(class: panel_kit_core::SurfaceClass) -> &'static str {
    match class {
        panel_kit_core::SurfaceClass::Compact => "compact",
        panel_kit_core::SurfaceClass::Tablet => "tablet",
        panel_kit_core::SurfaceClass::Regular => "regular",
    }
}

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    let workspace =
        composable_workspace::use_demo_workspace(STORAGE_KEY, default_layout, SavePolicy::OnSettle);
    composable_workspace::mount_viewport_observer(&workspace);
    let emit = composable_workspace::workspace_event_handler(&workspace);
    let mut ready = use_signal(|| false);
    let mut measured = use_signal(|| false);
    let profile = panel_kit::surface::surface_profile(workspace.snapshot.read().viewport.width);
    let surface = surface_label(profile.class);
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
                                        emit.clone(),
                                        Some(panel_kit::widgets::panel::traffic_lights(panel, emit.clone())),
                                        None,
                                    )}
                                    {panel_kit::widgets::panel::panel_body(match panel.key {
                                        Panel::Handoff => rsx! {
                                            p { "The post-WASM loading surface has handed off to the same "
                                                "workspace contract the application will keep." }
                                            p { class: "ready-state",
                                                "This panel uses V2 layout persistence. Resize the viewport to "
                                                "watch the surface tier and pointer capability update above."
                                            }
                                        },
                                    })}
                                    {panel_kit::widgets::panel::resize_grip(panel, emit.clone())}
                                })
                            }
                        }
                    }
                }
                {panel_kit::widgets::dock::dock(frame.dock, &workspace.catalog, emit.clone())}
            }
        }
    }
}
