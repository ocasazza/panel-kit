//! OKF graph-topoi host — one knowledge-catalog repository read as two regimes.
//!
//! Run with: `dx serve --example okf --platform web`, with the `okfEnv`
//! variables in the environment (`nix develop` provides them).
//!
//! - The `okf` grammar parses the pinned Open Knowledge Format snapshot into
//!   one site; `okf-catalog` and `okf-lineage` are two regimes over it, and
//!   `t` (or the top bar) crosses `f` between them. The switch exchanges the
//!   `WorkspaceSpec` and its persisted layout, recolours every node by the
//!   codomain's sorts, and re-solves the codomain's layout *seeded from the
//!   current positions*, which the renderer tweens over ~800 ms.
//! - The graph panel is the workspace's declared `graph` external component:
//!   jump-cannon's WebGPU view when `navigator.gpu` yields an adapter, panel-kit's
//!   own SVG painter over the same positions otherwise. The panel declares
//!   `keep_mounted`, so the canvas is not torn down when the panel leaves the
//!   frame.
//! - Every other panel body is painted from `OkfHost::resolve(binding)` through
//!   the shared section painter, so no panel can show another panel's content.
//!
//! Keys: `t` switch regime, `n`/`p` next/previous node, `l` re-solve the layout,
//! `r` reset the workspace layout.

use std::sync::LazyLock;

use dioxus::events::{Key, KeyboardEvent, PointerEvent as DioxusPointerEvent};
use dioxus::prelude::*;
use panel_kit::CSS;
use panel_kit_core::reducer::{Viewport, WorkspaceEvent};
use panel_kit_core::spec::ComponentEvent;
use panel_kit_core::widgets::ContentSpec;
use panel_kit_core::Units;

#[allow(dead_code, unused_imports)]
#[path = "support/composable_workspace.rs"]
mod composable_workspace;
#[allow(dead_code)]
#[path = "support/spec_workspace.rs"]
mod spec_workspace;
#[allow(dead_code)]
#[path = "support/section_view.rs"]
mod section_view;
#[allow(dead_code)]
#[path = "support/okf_host.rs"]
mod okf_host;
#[allow(dead_code)]
#[path = "support/graph_component.rs"]
mod graph_component;

use graph_component::graph_layout::solve;
use graph_component::GraphComponent;
use okf_host::{OkfHost, OkfSpecs, CATALOG, GRAPH, LINEAGE};

const GRAMMAR: &str = include_str!(env!("PANEL_KIT_OKF_GRAMMAR"));
const DATA: &str = include_str!(env!("PANEL_KIT_OKF_DATA"));
const CATALOG_SPEC: &str = include_str!(env!("PANEL_KIT_OKF_CATALOG"));
const LINEAGE_SPEC: &str = include_str!(env!("PANEL_KIT_OKF_LINEAGE"));
const BASE: &str = include_str!(env!("PANEL_KIT_OKF_BASE"));
const MORPHISM: &str = include_str!(env!("PANEL_KIT_OKF_MORPHISM"));
const CATALOG_PHYSICS: &str = include_str!(env!("PANEL_KIT_OKF_CATALOG_PHYSICS"));
const LINEAGE_PHYSICS: &str = include_str!(env!("PANEL_KIT_OKF_LINEAGE_PHYSICS"));

const HOST_CSS: &str = "
.topbar { flex-wrap: wrap; height: auto; min-height: 36px; gap: .45rem; }
.topbar h1 { font-size: .74rem; text-transform: uppercase; }
.topbar button { background: var(--bg); color: var(--fg); border: 1px solid var(--line2);
  border-radius: 3px; padding: .15rem .5rem; font-size: .72rem; cursor: pointer; }
.topbar button:hover { border-color: var(--fg); }
.topbar button.active { background: var(--fg); color: var(--bg); }
.topbar .axis { color: var(--dim); font-size: .62rem; text-transform: uppercase;
  letter-spacing: .12em; }
.topbar .axis-buttons { display: flex; gap: .35rem; }
.topbar .status { color: var(--accent); font-size: .7rem; }
.topbar .hint { color: var(--dim); font-size: .62rem; }
.pk-content-unbound { color: var(--yellow); font-size: .78rem; }
";

/// The eight Nix-authored specs `okfEnv` names.
const SPECS: OkfSpecs<'static> = OkfSpecs {
    grammar: GRAMMAR,
    data: DATA,
    catalog: CATALOG_SPEC,
    lineage: LINEAGE_SPEC,
    base: BASE,
    morphism: MORPHISM,
    catalog_physics: CATALOG_PHYSICS,
    lineage_physics: LINEAGE_PHYSICS,
};

/// The loaded host, or the reason it could not start.
type Host = Signal<Result<OkfHost, String>>;

/// The catalog regime's `WorkspaceSpec` JSON, resolved once for the spec
/// resolver hook, which requires `&'static str`.
static INITIAL_WORKSPACE: LazyLock<String> = LazyLock::new(|| {
    OkfHost::load(&SPECS)
        .expect("the OKF specs are embedded from their store paths")
        .workspace_json()
});

fn main() {
    dioxus::launch(App);
}

/// The browser viewport, in the units the web backend projects in.
fn browser_viewport() -> Viewport {
    let (width, height) = panel_kit::surface::viewport_size();
    Viewport { width, height, units: Units::CssPx }
}

/// The one element every panel body comes from: the external graph component
/// for `okf.graph`, the resolved section for every other binding.
///
/// An id the host does not resolve names itself rather than leaving a blank
/// body, and a host that failed to start says so in every panel.
fn host_content(host: Host, positions: Signal<Vec<[f32; 3]>>, id: &str, content: &ContentSpec) -> Element {
    if id == GRAPH {
        return rsx! { OkfGraph { host, positions } };
    }
    let state = host.read();
    let Ok(state) = state.as_ref() else {
        return rsx! { div { class: "pk-content pk-content-unbound", "host failed to start" } };
    };
    let Some(section) = state.resolve(id) else {
        return rsx! {
            div {
                class: "pk-content pk-content-unbound",
                role: "group",
                aria_label: "unbound binding {id}",
                "unbound: {id}"
            }
        };
    };
    section_view::paint(section, content, None, None)
}

/// The `graph` external component over the active regime's model.
///
/// `positions` is the arrangement the host wants drawn; a topos switch solves
/// the codomain's layout seeded from it and the component tweens there.
#[component]
fn OkfGraph(host: Host, positions: Signal<Vec<[f32; 3]>>) -> Element {
    let model = use_memo(move || host.read().as_ref().map(OkfHost::graph).unwrap_or_default());
    let layout = use_memo(move || host.read().as_ref().map(OkfHost::graph_layout).unwrap_or_default());
    let selected = use_memo(move || {
        host.read().as_ref().ok().and_then(|state| state.selected().map(str::to_owned))
    });
    let component = use_memo(move || {
        host.read().as_ref().ok().and_then(|state| state.components().first().cloned())
    });
    let on_event = EventHandler::new(move |event: ComponentEvent| {
        if let ComponentEvent::Select { node } = event {
            host.with_mut(|state| {
                if let Ok(state) = state {
                    state.select(Some(&node));
                }
            });
        }
    });

    // Reading `positions` for the target prop subscribes this component, so
    // every re-solve re-renders it and its tween effect walks to the new one.
    let target = positions.read().clone();
    let Some(component) = component() else {
        return rsx! { div { class: "pk-content pk-content-unbound", "no graph component declared" } };
    };

    rsx! {
        GraphComponent {
            model: model(),
            layout: layout(),
            target,
            selected: selected(),
            on_event,
            component,
        }
    }
}

/// Solve the active regime's layout and make it the graph's next target.
///
/// `seeded` walks from the arrangement currently on screen — what a topos
/// switch needs, so the codomain's solver starts where the domain left off.
/// Unseeded, the solver starts from its own deterministic arrangement.
fn retarget(host: &mut Host, positions: &mut Signal<Vec<[f32; 3]>>, seeded: bool) {
    let seed = positions.peek().clone();
    let next = host.with(|state| match state {
        Ok(state) => {
            let model = state.graph();
            let layout = state.graph_layout();
            Some(solve(&model, &layout, if seeded { Some(&seed) } else { None }))
        }
        Err(_) => None,
    });
    if let Some(next) = next {
        positions.set(next);
    }
}

#[component]
fn App() -> Element {
    let mut host = use_signal(|| OkfHost::load(&SPECS));
    // The first arrangement is the active regime's layout solved cold.
    let mut positions = use_signal(move || {
        host.peek()
            .as_ref()
            .map(|state| solve(&state.graph(), &state.graph_layout(), None))
            .unwrap_or_default()
    });

    let initial = spec_workspace::use_spec_workspace(INITIAL_WORKSPACE.as_str());
    let mut workspace = use_signal(move || initial.clone());
    let header_clicks = use_signal(|| 0_u32);

    // The viewport observer captures the workspace it mounted with; a topos
    // switch installs a new one, so the signal is read per event instead.
    let observed = workspace;
    let _viewport = panel_kit::surface::observe_viewport(EventHandler::new(move |size: Viewport| {
        let current = observed.read();
        spec_workspace::workspace_event_handler(&current).call(WorkspaceEvent::ViewportChanged {
            size,
            policy: current.resolved.surface.resize_policy,
        });
    }));

    // One transaction per action: crossing `f` exchanges the host's regime,
    // the WorkspaceSpec and its persisted layout, and the graph's arrangement.
    let mut switch_topos = move |id: &str| {
        let crossed = host.with_mut(|state| match state {
            Ok(state) if state.topos_id() != id => {
                state.switch_topos();
                true
            }
            _ => false,
        });
        if !crossed {
            return;
        }
        let Some(json) = host.with(|state| state.as_ref().ok().map(OkfHost::workspace_json)) else {
            return;
        };
        // Every read guard ends with its statement: the switch writes both
        // signals, and a live read of either would panic the borrow.
        let switched = {
            let current = workspace.read();
            spec_workspace::switch_workspace_spec(&current, &json)
        };
        workspace.set(switched);
        retarget(&mut host, &mut positions, true);
        // The restored layout carries the spec's authored viewport, and no new
        // resize arrives on a switch: re-measure so the grid fills the window.
        let current = workspace.read();
        spec_workspace::workspace_event_handler(&current).call(WorkspaceEvent::ViewportChanged {
            size: browser_viewport(),
            policy: current.resolved.surface.resize_policy,
        });
    };

    let key_workspace = workspace.read().clone();
    let mut on_key = move |event: KeyboardEvent| {
        let mut handled = false;
        if !panel_kit::input::is_editing() {
            if let Key::Character(character) = event.key() {
                match character.as_str() {
                    "t" => {
                        let active = host.with(|state| state.as_ref().ok().map(|state| state.topos_id().to_owned()));
                        switch_topos(match active.as_deref() {
                            Some(CATALOG) => LINEAGE,
                            _ => CATALOG,
                        });
                        handled = true;
                    }
                    "n" => {
                        host.with_mut(|state| {
                            if let Ok(state) = state {
                                state.cycle_selection(1);
                            }
                        });
                        handled = true;
                    }
                    "p" => {
                        host.with_mut(|state| {
                            if let Ok(state) = state {
                                state.cycle_selection(-1);
                            }
                        });
                        handled = true;
                    }
                    "l" => {
                        retarget(&mut host, &mut positions, false);
                        handled = true;
                    }
                    "r" => {
                        spec_workspace::reset_workspace(&key_workspace);
                        handled = true;
                    }
                    _ => {}
                }
            }
        }
        if handled {
            event.prevent_default();
        } else {
            spec_workspace::handle_key(&key_workspace, &event);
        }
    };

    let current = workspace.read();
    let emit = spec_workspace::workspace_event_handler(&current);
    let pointer_move_workspace = current.clone();
    let pointer_up_workspace = current.clone();
    let pointer_cancel_workspace = current.clone();
    let wheel_workspace = current.clone();

    let snapshot = current.snapshot.read();
    let mut scratch = current.scratch.borrow_mut();
    let frame = spec_workspace::project_workspace(&current, &snapshot, &mut scratch);
    let root_class = panel_kit::widgets::root::root_class(&frame);
    let workspace_class = spec_workspace::workspace_area_class(&frame);
    let workspace_style = frame
        .tile_grid
        .map(panel_kit::widgets::root::tile_grid_style)
        .unwrap_or_default();

    let (topos_ids, active_topos, status) = host.with(|state| match state {
        Ok(state) => {
            let graph = state.graph();
            let selected = state.selected().unwrap_or("none");
            (
                state.topos_ids(),
                state.topos_id().to_owned(),
                format!(
                    "{} · {} nodes · {} morphisms · selected {selected}",
                    state.topos_id(),
                    graph.nodes.len(),
                    graph.edges.len(),
                ),
            )
        }
        Err(error) => ([CATALOG, LINEAGE], String::new(), error.clone()),
    });

    // rsx cannot nest iterator combinators producing rsx inside a for-loop.
    let topos_buttons: Vec<Element> = topos_ids
        .iter()
        .map(|id| {
            let id = *id;
            let mut switch = switch_topos;
            rsx! {
                button {
                    key: "topos-{id}",
                    title: "Cross f to this regime: swap the WorkspaceSpec, its layout, and the graph",
                    class: if id == active_topos { "active" },
                    aria_pressed: id == active_topos,
                    onclick: move |_| switch(id),
                    "{id}"
                }
            }
        })
        .collect();

    let binding_content = move |id: &str, content: &ContentSpec| host_content(host, positions, id, content);

    rsx! {
        style { {CSS} }
        style { {HOST_CSS} }
        style { {graph_view::CSS} }
        div {
            class: "{root_class}",
            tabindex: "0",
            onpointermove: move |event: DioxusPointerEvent| spec_workspace::handle_pointer_move(&pointer_move_workspace, &event),
            onpointerup: move |event: DioxusPointerEvent| spec_workspace::handle_pointer_up(&pointer_up_workspace, &event),
            onpointercancel: move |event: DioxusPointerEvent| spec_workspace::handle_pointer_up(&pointer_cancel_workspace, &event),
            onkeydown: move |event: KeyboardEvent| on_key(event),
            header { class: "topbar",
                h1 { "OKF graph topoi" }
                span { class: "axis", "regime" }
                div { class: "axis-buttons",
                    {topos_buttons.into_iter()}
                }
                span { class: "status", "{status}" }
                span { class: "hint", "t switch regime · n/p next/prev node · l re-solve · r reset layout · click a node to select it" }
            }
            div {
                class: "{workspace_class}",
                style: "{workspace_style}",
                onwheel: move |event| spec_workspace::handle_wheel(&wheel_workspace, &event),
                {composable_workspace::web_canary::workspace_contents_kept(
                    &frame,
                    &current.resolved,
                    &snapshot,
                    emit,
                    header_clicks,
                    &binding_content,
                )}
            }
            if current.resolved.chrome.dock {
                {panel_kit::widgets::dock::dock(
                    frame.dock,
                    &current.resolved.catalog,
                    emit,
                    None,
                )}
            }
        }
    }
}