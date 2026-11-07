//! Trio host — one site, three regimes, one physics base.
//!
//! Run with: `dx serve --example trio --platform web`, with the `trioEnv`
//! variables in the environment (`nix develop` provides them).
//!
//! - The `trace-lines` (or equivalent `trace-json`) grammar reads the one
//!   site: agent sessions and the knowledge objects they produce and verify.
//!   `agentic`, `membrane`, and `okf-catalog` are three regimes over it,
//!   related by `f: agentic → membrane`, `ψ: okf-catalog → membrane`,
//!   `χ: okf-catalog → agentic`, and `ω: agentic → okf-catalog`; every regime
//!   reads every kind, natively or along the diagram.
//! - Every regime pulls its tuning back from the one physics base, so a regime
//!   switch keeps the arrangement and only recolours it: the same nodes in the
//!   same places wearing the codomain's sorts, the live simulation re-tuned in
//!   place. Switching grammar re-solves, because the objects were re-read.
//! - The graph panel is the active regime's declared `graph` external
//!   component — jump-cannon's WebGPU view when `navigator.gpu` yields an
//!   adapter, panel-kit's own SVG painter over the same positions otherwise.
//!   The panel declares `keep_mounted`, so the canvas survives a spec swap.
//! - Every other panel body is painted from `TrioHost::resolve(binding)`, so no
//!   panel can show another panel's content.
//!
//! Keys: `t` next regime, `g` grammar, `n`/`p` next/previous selection, `l`
//! re-solve the layout, `r` reset the workspace layout.

use std::sync::LazyLock;

use dioxus::events::{Key, KeyboardEvent, PointerEvent as DioxusPointerEvent};
use dioxus::prelude::*;
use panel_kit::editor::MonacoEditor;
use panel_kit::CSS;
use panel_kit_core::badge::BadgeAction;
use panel_kit_core::reducer::{Viewport, WorkspaceEvent};
use panel_kit_core::spec::ComponentEvent;
use panel_kit_core::widgets::ContentSpec;
use panel_kit_core::Units;
use panel_kit_grammar::Section;

use panel_kit::{section_view, spec_workspace};

#[allow(dead_code)]
#[path = "support/trio_host.rs"]
mod trio_host;
#[allow(dead_code)]
#[path = "support/graph_component.rs"]
mod graph_component;

use graph_component::graph_layout::solve;
use graph_component::GraphComponent;
use spec_workspace::SpecWorkspaceState;
use trio_host::{RegimeId, TrioHost, TrioSpecs};

const AGENTIC: &str = include_str!(env!("PANEL_KIT_TRIO_AGENTIC"));
const MEMBRANE: &str = include_str!(env!("PANEL_KIT_TRIO_MEMBRANE"));
const OKF_CATALOG: &str = include_str!(env!("PANEL_KIT_TRIO_OKF_CATALOG"));
const BASE: &str = include_str!(env!("PANEL_KIT_TRIO_BASE"));
const F: &str = include_str!(env!("PANEL_KIT_TRIO_F"));
const PSI: &str = include_str!(env!("PANEL_KIT_TRIO_PSI"));
const CHI: &str = include_str!(env!("PANEL_KIT_TRIO_CHI"));
const OMEGA: &str = include_str!(env!("PANEL_KIT_TRIO_OMEGA"));
const AGENTIC_PHYSICS: &str = include_str!(env!("PANEL_KIT_TRIO_AGENTIC_PHYSICS"));
const MEMBRANE_PHYSICS: &str = include_str!(env!("PANEL_KIT_TRIO_MEMBRANE_PHYSICS"));
const CATALOG_PHYSICS: &str = include_str!(env!("PANEL_KIT_TRIO_OKF_CATALOG_PHYSICS"));
const GRAMMAR_LINES: &str = include_str!(env!("PANEL_KIT_TRIO_GRAMMAR_LINES"));
const GRAMMAR_JSON: &str = include_str!(env!("PANEL_KIT_TRIO_GRAMMAR_JSON"));
const DATA_LINES: &str = include_str!(env!("PANEL_KIT_TRIO_DATA_LINES"));
const DATA_JSON: &str = include_str!(env!("PANEL_KIT_TRIO_DATA_JSON"));

const HOST_CSS: &str = "
.topbar { gap: .45rem; white-space: nowrap; overflow: hidden; }
.dock .keys { color: var(--dim); font-size: .68rem; white-space: nowrap; }
.topbar h1 { font-size: .74rem; text-transform: uppercase; }
.topbar button { background: var(--bg); color: var(--fg); border: 1px solid var(--line2);
  border-radius: 3px; padding: .15rem .5rem; font-size: .72rem; cursor: pointer; }
.topbar button:hover:not(:disabled) { border-color: var(--fg); }
.topbar button.active { background: var(--fg); color: var(--bg); }
.topbar button:disabled { color: var(--dim); border-style: dashed; cursor: not-allowed; }
.topbar .axis { color: var(--dim); font-size: .62rem; text-transform: uppercase;
  letter-spacing: .12em; }
.topbar .axis-buttons { display: flex; gap: .35rem; align-items: center; }
.topbar .status { color: var(--accent); font-size: .7rem; }
.pk-content-unbound { color: var(--yellow); font-size: .78rem; }
";

/// The fifteen Nix-authored documents `trioEnv` names.
const SPECS: TrioSpecs<'static> = TrioSpecs {
    agentic: AGENTIC,
    membrane: MEMBRANE,
    okf_catalog: OKF_CATALOG,
    base: BASE,
    f: F,
    psi: PSI,
    chi: CHI,
    omega: OMEGA,
    agentic_physics: AGENTIC_PHYSICS,
    membrane_physics: MEMBRANE_PHYSICS,
    catalog_physics: CATALOG_PHYSICS,
    trace_lines_grammar: GRAMMAR_LINES,
    trace_lines_data: DATA_LINES,
    trace_json_grammar: GRAMMAR_JSON,
    trace_json_data: DATA_JSON,
};

/// The loaded host, or the reason it could not start.
type Host = Signal<Result<TrioHost, String>>;

fn main() {
    dioxus::launch(App);
}

/// The browser viewport, in the units the web backend projects in.
fn browser_viewport() -> Viewport {
    let (width, height) = panel_kit::surface::viewport_size();
    Viewport { width, height, units: Units::CssPx }
}

/// The one element every panel body comes from: the external graph component
/// for the active regime's graph binding, a Monaco editor for an editor
/// binding, the resolved section for every other.
///
/// A binding the host does not resolve names itself rather than leaving a blank
/// body, and a host that failed to start says so in every panel.
fn host_content(host: Host, positions: Signal<Vec<[f32; 3]>>, id: &str, content: &ContentSpec) -> Element {
    // The graph binding is read in its own guard, so the guard ends before the
    // component below re-reads the same signal.
    let graph = host.with(|state| {
        state.as_ref().ok().is_some_and(|state| id == state.regime().graph_binding())
    });
    if graph {
        return rsx! { TrioGraph { host, positions } };
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
    if let ContentSpec::Editor { multiline, placeholder, .. } = content {
        return editor_body(state, id, *multiline, placeholder, section);
    }
    section_view::paint(id, section, content, None, Some(on_badge(&host)))
}

/// The editor body for one binding: the active grammar's source under the
/// grammar's own language, a morphism document as JSON, and the authored
/// prompt otherwise. Monaco keeps its own model, so the element is keyed by
/// regime and grammar to re-mount with the document that is now bound.
fn editor_body(
    state: &TrioHost,
    id: &str,
    multiline: bool,
    placeholder: &str,
    section: &Section,
) -> Element {
    let Section::Text(text) = section else {
        return panel_kit::canary::editor_placeholder(id, multiline, placeholder);
    };
    let language = if id == trio_host::GRAMMAR { state.grammar_language() } else { "json" };
    let key = format!("{}-{}-{id}", state.regime().label(), state.grammar_id());
    rsx! {
        MonacoEditor { key: "{key}", initial: "{text.text}", language: "{language}", read_only: true }
    }
}

/// Badge clicks select: the badge's own field and value go to the host, which
/// moves the selection to the object that badge names.
fn on_badge(host: &Host) -> EventHandler<BadgeAction> {
    let mut host = *host;
    EventHandler::new(move |action: BadgeAction| {
        let (field, value) = match action {
            BadgeAction::Toggle { field, value } | BadgeAction::Clicked { field, value } => {
                (field, value)
            }
            _ => return,
        };
        host.with_mut(|state| {
            if let Ok(state) = state {
                state.select_matching(&field, &value);
            }
        });
    })
}

/// The `graph` external component over the active regime's model.
///
/// `positions` is the arrangement the host wants drawn; a site or grammar
/// switch re-solves it and the component tweens there, while a regime switch
/// leaves it alone and only recolours.
#[component]
fn TrioGraph(host: Host, positions: Signal<Vec<[f32; 3]>>) -> Element {
    let model = use_memo(move || host.read().as_ref().map(TrioHost::graph).unwrap_or_default());
    let layout = use_memo(move || host.read().as_ref().map(TrioHost::graph_layout).unwrap_or_default());
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
/// `seeded` walks from the arrangement currently on screen — what a site
/// switch needs, so the new site's solver starts where the old one left off.
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

/// Bring the mounted workspace and the graph's arrangement in line with the
/// host after an axis moved.
///
/// A regime change installs that regime's `WorkspaceSpec` and its persisted
/// layout; a site or grammar change additionally re-solves the arrangement,
/// because the objects behind the positions are not the ones on screen.
fn adopt(
    host: &mut Host,
    workspace: &mut Signal<SpecWorkspaceState>,
    positions: &mut Signal<Vec<[f32; 3]>>,
    reseed: bool,
) {
    let Some((id, json)) = host.with(|state| {
        state
            .as_ref()
            .ok()
            .map(|state| (state.workspace_spec().id.clone(), state.workspace_json()))
    }) else {
        return;
    };
    if workspace.peek().resolved.id != id {
        // Every read guard ends with its statement: the switch writes the signal
        // and a live read of it would panic the borrow.
        let switched = {
            let current = workspace.read();
            spec_workspace::switch_workspace_spec(&current, &json)
        };
        workspace.set(switched);
        // The restored layout carries the spec's authored viewport, and no new
        // resize arrives on a switch: re-measure so the grid fills the window.
        let current = workspace.read();
        spec_workspace::workspace_event_handler(&current).call(WorkspaceEvent::ViewportChanged {
            size: browser_viewport(),
            policy: current.resolved.surface.resize_policy,
        });
    }
    if reseed {
        retarget(host, positions, false);
    }
}

/// The regime after `active`, wrapping around: `t` walks `RegimeId::ALL`.
fn next_regime(active: RegimeId) -> RegimeId {
    RegimeId::ALL[(active as usize + 1) % RegimeId::ALL.len()]
}

#[component]
fn App() -> Element {
    let mut host = use_signal(|| TrioHost::load(&SPECS));
    // The first arrangement is the active regime's layout solved cold.
    let mut positions = use_signal(move || {
        host.peek()
            .as_ref()
            .map(|state| solve(&state.graph(), &state.graph_layout(), None))
            .unwrap_or_default()
    });

    // The host opens in the agentic regime, so that workspace spec is the one
    // to mount first.
    let initial = spec_workspace::use_spec_workspace(INITIAL_WORKSPACE.as_str());
    let mut workspace = use_signal(move || initial.clone());
    let header_clicks = use_signal(|| 0_u32);

    // The viewport observer captures the workspace it mounted with; a regime
    // switch installs a new one, so the signal is read per event instead.
    let observed = workspace;
    let _viewport = panel_kit::surface::observe_viewport(EventHandler::new(move |size: Viewport| {
        let current = observed.read();
        spec_workspace::workspace_event_handler(&current).call(WorkspaceEvent::ViewportChanged {
            size,
            policy: current.resolved.surface.resize_policy,
        });
    }));

    let mut switch_regime = move |regime: RegimeId| {
        let crossed = host.with_mut(|state| match state {
            Ok(state) if state.regime() != regime => {
                state.set_regime(regime);
                true
            }
            _ => false,
        });
        if crossed {
            // The pulled-back physics is one base, so the arrangement survives
            // the crossing: only the sorts and the colours move.
            adopt(&mut host, &mut workspace, &mut positions, false);
        }
    };

    let key_workspace = workspace.read().clone();
    let mut on_key = move |event: KeyboardEvent| {
        let mut handled = false;
        if !panel_kit::input::is_editing() {
            if let Key::Character(character) = event.key() {
                match character.as_str() {
                    "t" => {
                        let next = host.with(|state| state.as_ref().ok().map(|state| next_regime(state.regime())));
                        if let Some(regime) = next {
                            switch_regime(regime);
                        }
                        handled = true;
                    }
                    "g" => {
                        let cycled = host.with_mut(|state| match state {
                            Ok(state) => {
                                state.cycle_grammar();
                                true
                            }
                            Err(_) => false,
                        });
                        if cycled {
                            adopt(&mut host, &mut workspace, &mut positions, true);
                        }
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

    let header = host.with(|state| match state {
        Ok(state) => Header { regime: state.regime(), status: state.status_line() },
        Err(error) => Header { regime: RegimeId::Agentic, status: error.clone() },
    });

    // rsx cannot nest iterator combinators producing rsx inside a for-loop.
    let regime_buttons: Vec<Element> = RegimeId::ALL
        .iter()
        .map(|regime| {
            let regime = *regime;
            let mut switch = switch_regime;
            let active = regime == header.regime;
            rsx! {
                button {
                    key: "regime-{regime}",
                    title: "Read the site in this regime; the arrangement stays, the sorts and colours move",
                    class: if active { "active" },
                    aria_pressed: active,
                    onclick: move |_| switch(regime),
                    "{regime.label()}"
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
                h1 { "trio: one site × three regimes" }
                span { class: "axis", "regime" }
                div { class: "axis-buttons",
                    {regime_buttons.into_iter()}
                }
                span { class: "status", "{header.status}" }
            }
            div {
                class: "{workspace_class}",
                style: "{workspace_style}",
                onwheel: move |event| spec_workspace::handle_wheel(&wheel_workspace, &event),
                {panel_kit::canary::workspace_contents_kept(
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
                    Some(rsx! {
                        span { class: "keys", "t next regime · g grammar · n/p selection · l re-solve · r reset layout · click a node to select it" }
                    }),
                )}
            }
        }
    }
}

/// What the header reads out of the host in one render pass. The status line
/// names the grammar, so the header only offers the regime axis.
struct Header {
    regime: RegimeId,
    status: String,
}

/// The workspace spec the host mounts at startup: the site read in the agentic
/// regime. Resolved once for the spec resolver hook, which requires
/// `&'static str`.
static INITIAL_WORKSPACE: LazyLock<String> = LazyLock::new(|| {
    TrioHost::load(&SPECS)
        .expect("the trio specs are embedded from their store paths")
        .workspace_json()
});