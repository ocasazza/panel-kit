//! Spec-driven workspace canary — executable documentation for composable web parts.
//!
//! The workspace is authored in Nix and embedded as a build-time store path:
//! `include_str!(env!("PANEL_KIT_WORKSPACE_SPEC"))`. The example decodes,
//! validates, resolves, restores, reduces, projects, and paints without using
//! the controller API. It is intentionally broad: the Nix specs compiled by the
//! `checks.workspace-spec-web-wasm` lane exercise full chrome, standalone
//! one-panel surface mode, and every web widget painter.

use dioxus::events::PointerEvent as DioxusPointerEvent;
use dioxus::prelude::*;
use panel_kit::CSS;

#[path = "support/spec_workspace.rs"]
mod spec_workspace;
#[allow(dead_code, unused_imports)]
#[path = "support/composable_workspace.rs"]
mod composable_workspace;

const SPEC_JSON: &str = include_str!(env!("PANEL_KIT_WORKSPACE_SPEC"));

const DEMO_CSS: &str = "
.topbar button { background: var(--bg); color: var(--fg); border: 1px solid var(--line2);
  border-radius: 3px; padding: .15rem .5rem; font-size: .72rem; cursor: pointer; }
.topbar button:hover { border-color: var(--fg); }
.topbar .coverage { color: var(--dim); font-size: .68rem; }
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
    let workspace = spec_workspace::use_spec_workspace(SPEC_JSON);
    spec_workspace::mount_viewport_observer(&workspace);
    let emit = spec_workspace::workspace_event_handler(&workspace);
    let header_clicks = use_signal(|| 0_u32);

    let snapshot = workspace.snapshot.read();
    let mut scratch = workspace.scratch.borrow_mut();
    let frame = spec_workspace::project_workspace(&workspace, &snapshot, &mut scratch);
    let root_class = panel_kit::widgets::root::root_class(&frame);
    let workspace_class = spec_workspace::workspace_area_class(&frame);
    let workspace_style = frame
        .tile_grid
        .map(panel_kit::widgets::root::tile_grid_style)
        .unwrap_or_default();
    let mode = composable_workspace::web_canary::mode_label(frame.mode);
    let surface = composable_workspace::web_canary::surface_label(frame.surface.class);
    let kinds = composable_workspace::web_canary::content_kinds(&workspace.resolved);

    let pointer_move_workspace = workspace.clone();
    let pointer_up_workspace = workspace.clone();
    let pointer_cancel_workspace = workspace.clone();
    let key_workspace = workspace.clone();
    let wheel_workspace = workspace.clone();
    let reset_workspace = workspace.clone();

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
                h1 { "panel-kit WorkspaceSpec web canary" }
                span { class: "hint",
                    "spec: {workspace.resolved.id} · mode: {mode} · surface: {surface} · panels: {workspace.resolved.panels.len()}"
                }
                span { class: "coverage", "content: {kinds}" }
                button {
                    onclick: move |_| spec_workspace::reset_workspace(&reset_workspace),
                    "reset authored layout"
                }
            }
            div {
                class: "{workspace_class}",
                style: "{workspace_style}",
                onwheel: move |event| spec_workspace::handle_wheel(&wheel_workspace, &event),
                {composable_workspace::web_canary::workspace_contents(&frame, &workspace.resolved, emit, header_clicks)}
            }
            if workspace.resolved.chrome.dock {
                {panel_kit::widgets::dock::dock(frame.dock, &workspace.resolved.catalog, emit)}
            }
        }
    }
}
