//! Multi-spec workspace canary — switching between two workspace grammars.
//!
//! This example demonstrates dynamically switching between two workspace
//! specifications loaded from environment variables. The UI provides a topbar
//! with buttons to switch between the active spec, preserving layout state
//! as transitions occur via `switch_workspace_spec`.

use dioxus::events::PointerEvent as DioxusPointerEvent;
use dioxus::prelude::*;
use panel_kit::CSS;

#[allow(dead_code, unused_imports)]
#[path = "support/composable_workspace.rs"]
mod composable_workspace;
#[path = "support/spec_workspace.rs"]
mod spec_workspace;

const SPEC_A: &str = include_str!(env!("PANEL_KIT_WORKSPACE_SPEC"));
const SPEC_B: &str = include_str!(env!("PANEL_KIT_TOPOS_SPEC_B"));

const DEMO_CSS: &str = "
.topbar button { background: var(--bg); color: var(--fg); border: 1px solid var(--line2);
  border-radius: 3px; padding: .15rem .5rem; font-size: .72rem; cursor: pointer; }
.topbar button:hover { border-color: var(--fg); }
.topbar button.active { background: var(--fg); color: var(--bg); }
.topbar .coverage { color: var(--dim); font-size: .68rem; }
.topbar .grammar-switcher { display: flex; gap: .35rem; }
.pk-custom-card { display: grid; gap: .35rem; align-content: start; height: 100%; }
.pk-custom-card h2 { margin: 0; font-size: .85rem; letter-spacing: .08em; text-transform: uppercase; }
.pk-custom-card p { margin: 0; color: var(--dim); }
.pk-editor { width: 100%; min-height: 10rem; height: 75%; background: var(--bg); color: var(--fg);
  border: 1px solid var(--line2); border-radius: 3px; font-family: var(--mono);
  font-size: .78rem; padding: .4rem; resize: none; }
.spec-list { margin: .25rem 0; padding-left: 1.1rem; }
.spec-list li { color: var(--dim); }
.too-small { display: grid; place-items: center; height: 100%; color: var(--yellow); }
";

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    let workspace_a_init = spec_workspace::use_spec_workspace(SPEC_A);
    let workspace_b_init = spec_workspace::use_spec_workspace(SPEC_B);
    
    let mut workspace_a = use_signal(|| workspace_a_init.clone());
    let mut workspace_b = use_signal(|| workspace_b_init.clone());
    let mut active_index = use_signal(|| 0usize);

    spec_workspace::mount_viewport_observer(&workspace_a());
    spec_workspace::mount_viewport_observer(&workspace_b());

    let current_workspace = if active_index() == 0 {
        workspace_a()
    } else {
        workspace_b()
    };

    let emit = spec_workspace::workspace_event_handler(&current_workspace);
    let header_clicks = use_signal(|| 0_u32);
    let snap_policy = (current_workspace.snap)();
    let mut move_snap = current_workspace.snap;
    let mut resize_snap = current_workspace.snap;

    let snapshot = current_workspace.snapshot.read();
    let mut scratch = current_workspace.scratch.borrow_mut();
    let frame = spec_workspace::project_workspace(&current_workspace, &snapshot, &mut scratch);
    let root_class = panel_kit::widgets::root::root_class(&frame);
    let workspace_class = spec_workspace::workspace_area_class(&frame);
    let workspace_style = frame
        .tile_grid
        .map(panel_kit::widgets::root::tile_grid_style)
        .unwrap_or_default();
    let mode = composable_workspace::web_canary::mode_label(frame.mode);
    let surface = composable_workspace::web_canary::surface_label(frame.surface.class);
    let kinds = composable_workspace::web_canary::content_kinds(&current_workspace.resolved);

    let pointer_move_workspace = current_workspace.clone();
    let pointer_up_workspace = current_workspace.clone();
    let pointer_cancel_workspace = current_workspace.clone();
    let key_workspace = current_workspace.clone();
    let wheel_workspace = current_workspace.clone();
    let header_reset_workspace = current_workspace.clone();
    let dock_reset_workspace = current_workspace.clone();

    rsx! {
        style { {CSS} }
        style { {DEMO_CSS} }
        div {
            class: "{root_class}",
            tabindex: "0",
            onpointermove: move |event: DioxusPointerEvent| {
                spec_workspace::handle_pointer_move(&pointer_move_workspace, &event)
            },
            onpointerup: move |event: DioxusPointerEvent| {
                spec_workspace::handle_pointer_up(&pointer_up_workspace, &event)
            },
            onpointercancel: move |event: DioxusPointerEvent| {
                spec_workspace::handle_pointer_up(&pointer_cancel_workspace, &event)
            },
            onkeydown: move |event| spec_workspace::handle_key(&key_workspace, &event),
            header { class: "topbar",
                h1 { "panel-kit Multi-Spec WorkspaceSpec Web Canary" }
                div { class: "grammar-switcher",
                    button {
                        class: if active_index() == 0 { "active" } else { "" },
                        onclick: move |_| {
                            let ws = workspace_a();
                            let switched = spec_workspace::switch_workspace_spec(&ws, SPEC_A);
                            workspace_a.set(switched);
                            active_index.set(0);
                        },
                        "Canary"
                    }
                    button {
                        class: if active_index() == 1 { "active" } else { "" },
                        onclick: move |_| {
                            let ws = workspace_b();
                            let switched = spec_workspace::switch_workspace_spec(&ws, SPEC_B);
                            workspace_b.set(switched);
                            active_index.set(1);
                        },
                        "Web"
                    }
                }
                span { class: "hint",
                    "spec: {current_workspace.resolved.id} · mode: {mode} · surface: {surface} · panels: {current_workspace.resolved.panels.len()}"
                }
                span { class: "coverage", "content: {kinds}" }
                button {
                    onclick: move |_| spec_workspace::reset_workspace(&header_reset_workspace),
                    "reset authored layout"
                }
            }
            div {
                class: "{workspace_class}",
                style: "{workspace_style}",
                onwheel: move |event| spec_workspace::handle_wheel(&wheel_workspace, &event),
                {composable_workspace::web_canary::workspace_contents(&frame, &current_workspace.resolved, emit, header_clicks)}
            }
            if current_workspace.resolved.chrome.dock {
                {panel_kit::widgets::dock::dock(
                    frame.dock,
                    &current_workspace.resolved.catalog,
                    emit,
                    Some(rsx! {
                        div {
                            class: "snap-toggle",
                            role: "group",
                            aria_label: "Pointer snap policy",
                            button {
                                r#type: "button",
                                class: if snap_policy.move_ { "on" } else { "off" },
                                aria_pressed: snap_policy.move_,
                                title: "Toggle move snapping",
                                onclick: move |_| {
                                    move_snap.with_mut(|policy| policy.move_ = !policy.move_);
                                },
                                span { class: "snap-toggle-glyph", aria_hidden: "true",
                                    if snap_policy.move_ { "●" } else { "○" }
                                }
                                " move"
                            }
                            button {
                                r#type: "button",
                                class: if snap_policy.resize { "on" } else { "off" },
                                aria_pressed: snap_policy.resize,
                                title: "Toggle resize snapping",
                                onclick: move |_| {
                                    resize_snap.with_mut(|policy| policy.resize = !policy.resize);
                                },
                                span { class: "snap-toggle-glyph", aria_hidden: "true",
                                    if snap_policy.resize { "●" } else { "○" }
                                }
                                " resize"
                            }
                            button {
                                r#type: "button",
                                class: "reset",
                                title: "Clear saved layout and restore authored geometry",
                                onclick: move |_| {
                                    spec_workspace::reset_workspace(&dock_reset_workspace);
                                },
                                "reset layout"
                            }
                        }
                    }),
                )}
            }
        }
    }
}
