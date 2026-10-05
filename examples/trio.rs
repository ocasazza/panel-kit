//! Trio host — two sites, three regimes, one physics base.
//!
//! Run with: `dx serve --example trio --platform web`, with the `trioEnv`
//! variables in the environment (`nix develop` provides them).
//!
//! - The `trace-lines` (or equivalent `trace-json`) grammar reads the
//!   agent-trace site and the `okf` grammar reads the pinned knowledge-catalog
//!   site. `agentic`, `membrane`, and `okf-catalog` are three regimes over the
//!   two, related by `f: agentic → membrane`, `ψ: okf-catalog → membrane`, and
//!   `χ: okf-catalog → agentic`. A regime reads a site only when the site's
//!   native regime reaches it along one of those, so the traces site in
//!   okf-catalog is offered as a *disabled* button carrying the reason rather
//!   than an invented morphism.
//! - Every regime pulls its tuning back from the one physics base, so a regime
//!   switch keeps the arrangement and only recolours it: the same nodes in the
//!   same places wearing the codomain's sorts. Switching site or grammar
//!   re-solves, because the objects themselves changed.
//! - The graph panel is the active regime's declared `graph` external
//!   component — jump-cannon's WebGPU view when `navigator.gpu` yields an
//!   adapter, panel-kit's own SVG painter over the same positions otherwise.
//!   The panel declares `keep_mounted`, so the canvas survives a spec swap.
//! - Every other panel body is painted from `TrioHost::resolve(binding)`, so no
//!   panel can show another panel's content, and a sheaf that cannot read the
//!   active site says so instead of inventing a value.
//!
//! Keys: `s` site, `t` next available regime, `g` grammar, `n`/`p` next/previous
//! selection, `l` re-solve the layout, `r` reset the workspace layout.

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
#[path = "support/trio_host.rs"]
mod trio_host;
#[allow(dead_code)]
#[path = "support/graph_component.rs"]
mod graph_component;

use graph_component::graph_layout::solve;
use graph_component::GraphComponent;
use spec_workspace::SpecWorkspaceState;
use trio_host::{RegimeId, SiteId, TrioHost, TrioSpecs};

const AGENTIC: &str = include_str!(env!("PANEL_KIT_TRIO_AGENTIC"));
const MEMBRANE: &str = include_str!(env!("PANEL_KIT_TRIO_MEMBRANE"));
const OKF_CATALOG: &str = include_str!(env!("PANEL_KIT_TRIO_OKF_CATALOG"));
const BASE: &str = include_str!(env!("PANEL_KIT_TRIO_BASE"));
const F: &str = include_str!(env!("PANEL_KIT_TRIO_F"));
const PSI: &str = include_str!(env!("PANEL_KIT_TRIO_PSI"));
const CHI: &str = include_str!(env!("PANEL_KIT_TRIO_CHI"));
const AGENTIC_PHYSICS: &str = include_str!(env!("PANEL_KIT_TRIO_AGENTIC_PHYSICS"));
const MEMBRANE_PHYSICS: &str = include_str!(env!("PANEL_KIT_TRIO_MEMBRANE_PHYSICS"));
const CATALOG_PHYSICS: &str = include_str!(env!("PANEL_KIT_TRIO_OKF_CATALOG_PHYSICS"));
const GRAMMAR_LINES: &str = include_str!(env!("PANEL_KIT_TRIO_GRAMMAR_LINES"));
const GRAMMAR_JSON: &str = include_str!(env!("PANEL_KIT_TRIO_GRAMMAR_JSON"));
const DATA_LINES: &str = include_str!(env!("PANEL_KIT_TRIO_DATA_LINES"));
const DATA_JSON: &str = include_str!(env!("PANEL_KIT_TRIO_DATA_JSON"));
const OKF_GRAMMAR: &str = include_str!(env!("PANEL_KIT_TRIO_OKF_GRAMMAR"));
const OKF_DATA: &str = include_str!(env!("PANEL_KIT_TRIO_OKF_DATA"));

const HOST_CSS: &str = "
.topbar { flex-wrap: wrap; height: auto; min-height: 36px; gap: .45rem; }
.topbar h1 { font-size: .74rem; text-transform: uppercase; }
.topbar button { background: var(--bg); color: var(--fg); border: 1px solid var(--line2);
  border-radius: 3px; padding: .15rem .5rem; font-size: .72rem; cursor: pointer; }
.topbar button:hover:not(:disabled) { border-color: var(--fg); }
.topbar button.active { background: var(--fg); color: var(--bg); }
.topbar button:disabled { color: var(--dim); border-style: dashed; cursor: not-allowed; }
.topbar .axis { color: var(--dim); font-size: .62rem; text-transform: uppercase;
  letter-spacing: .12em; }
.topbar .axis-buttons, .topbar .switches { display: flex; gap: .35rem; align-items: center; }
.topbar .refused { color: var(--yellow); font-size: .62rem; }
.topbar .status { color: var(--accent); font-size: .7rem; }
.topbar .hint { color: var(--dim); font-size: .62rem; }
.pk-content-unbound { color: var(--yellow); font-size: .78rem; }
";

/// The sixteen Nix-authored documents `trioEnv` names.
const SPECS: TrioSpecs<'static> = TrioSpecs {
    agentic: AGENTIC,
    membrane: MEMBRANE,
    okf_catalog: OKF_CATALOG,
    base: BASE,
    f: F,
    psi: PSI,
    chi: CHI,
    agentic_physics: AGENTIC_PHYSICS,
    membrane_physics: MEMBRANE_PHYSICS,
    catalog_physics: CATALOG_PHYSICS,
    trace_lines_grammar: GRAMMAR_LINES,
    trace_lines_data: DATA_LINES,
    trace_json_grammar: GRAMMAR_JSON,
    trace_json_data: DATA_JSON,
    okf_grammar: OKF_GRAMMAR,
    okf_data: OKF_DATA,
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
/// for the active regime's graph binding, the resolved section for every other.
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
    section_view::paint(section, content, None, None)
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

/// The other site, wrapping around: the two sites are read in `SiteId` order.
fn other_site(site: SiteId) -> SiteId {
    SiteId::ALL[(site as usize + 1) % SiteId::ALL.len()]
}

/// The next regime the active site can be read in, after the active one.
///
/// A regime the site cannot carry is skipped rather than refused: `t` walks
/// the offered regimes, while the header's disabled button is where the refusal
/// and its reason live.
fn next_regime(site: SiteId, active: RegimeId) -> Option<RegimeId> {
    RegimeId::ALL
        .iter()
        .copied()
        .filter(|regime| TrioHost::available(site, *regime).is_ok())
        .find(|regime| *regime != active)
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

    // The host opens on the trace site in its native regime, so the agentic
    // workspace spec is the one to mount first.
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

    let mut switch_site = move |site: SiteId| {
        let moved = host.with_mut(|state| match state {
            Ok(state) if state.site_id() != site => {
                state.set_site(site);
                true
            }
            _ => false,
        });
        if moved {
            // A site switch brings different objects, so it re-solves; whether
            // it also brought a new regime is `adopt`'s business.
            adopt(&mut host, &mut workspace, &mut positions, true);
        }
    };

    let mut switch_regime = move |regime: RegimeId| {
        let crossed = host.with_mut(|state| match state {
            Ok(state) => state.set_regime(regime).is_ok(),
            Err(_) => false,
        });
        if crossed {
            // The pulled-back physics is one base, so the arrangement survives
            // the crossing: only the sorts and the colours move.
            adopt(&mut host, &mut workspace, &mut positions, false);
        }
    };

    let switch_grammar = move |id: &str| {
        let read = host.with_mut(|state| match state {
            Ok(state) => state.set_grammar(id),
            Err(_) => false,
        });
        if read {
            adopt(&mut host, &mut workspace, &mut positions, true);
        }
    };

    let key_workspace = workspace.read().clone();
    let mut on_key = move |event: KeyboardEvent| {
        let mut handled = false;
        if !panel_kit::input::is_editing() {
            if let Key::Character(character) = event.key() {
                match character.as_str() {
                    "s" => {
                        let site = host.with(|state| state.as_ref().ok().map(TrioHost::site_id));
                        if let Some(site) = site {
                            switch_site(other_site(site));
                        }
                        handled = true;
                    }
                    "t" => {
                        let next = host.with(|state| {
                            state
                                .as_ref()
                                .ok()
                                .and_then(|state| next_regime(state.site_id(), state.regime()))
                        });
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
        Ok(state) => Header {
            site: state.site_id(),
            regime: state.regime(),
            grammars: state.grammar_ids(),
            grammar: state.grammar_id(),
            status: state.status_line(),
        },
        Err(error) => Header {
            site: SiteId::Traces,
            regime: RegimeId::Agentic,
            grammars: &[],
            grammar: "",
            status: error.clone(),
        },
    });

    // rsx cannot nest iterator combinators producing rsx inside a for-loop.
    let site_buttons: Vec<Element> = SiteId::ALL
        .iter()
        .map(|site| {
            let site = *site;
            let mut switch = switch_site;
            rsx! {
                button {
                    key: "site-{site}",
                    title: "Read the other site's objects; the regime carries over when a morphism reaches it",
                    class: if site == header.site { "active" },
                    aria_pressed: site == header.site,
                    onclick: move |_| switch(site),
                    "{site}"
                }
            }
        })
        .collect();

    let regime_buttons: Vec<Element> = RegimeId::ALL
        .iter()
        .map(|regime| {
            let regime = *regime;
            let mut switch = switch_regime;
            // A regime the site cannot carry is shown disabled, carrying the
            // reason the host names for refusing it.
            let offered = TrioHost::available(header.site, regime);
            let refused = offered.as_ref().err().cloned();
            let active = regime == header.regime;
            let label = if active {
                regime.label().to_owned()
            } else {
                match &refused {
                    Some(reason) => format!("{} — {reason}", regime.label()),
                    None => regime.label().to_owned(),
                }
            };
            rsx! {
                button {
                    key: "regime-{regime}",
                    title: match &offered {
                        Ok(route) => format!("Read this site along {}", route.text()),
                        Err(reason) => reason.clone(),
                    },
                    class: if active { "active" },
                    disabled: refused.is_some(),
                    aria_pressed: active,
                    onclick: move |_| switch(regime),
                    "{label}"
                }
            }
        })
        .collect();

    let grammar_buttons: Vec<Element> = header
        .grammars
        .iter()
        .map(|id| {
            let id = *id;
            let mut switch = switch_grammar;
            rsx! {
                button {
                    key: "grammar-{id}",
                    title: "Re-read this site with another of its grammars; the objects change, so the graph re-solves",
                    class: if id == header.grammar { "active" },
                    aria_pressed: id == header.grammar,
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
                h1 { "trio: sites × regimes" }
                span { class: "axis", "site" }
                div { class: "axis-buttons",
                    {site_buttons.into_iter()}
                }
                span { class: "axis", "regime" }
                div { class: "axis-buttons",
                    {regime_buttons.into_iter()}
                }
                span { class: "axis", "grammar" }
                div { class: "axis-buttons",
                    {grammar_buttons.into_iter()}
                }
                span { class: "status", "{header.status}" }
                span { class: "hint", "s site · t next regime · g grammar · n/p next/prev selection · l re-solve · r reset layout · click a node to select it" }
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

/// What the header reads out of the host in one render pass.
struct Header<'a> {
    site: SiteId,
    regime: RegimeId,
    grammars: &'a [&'a str],
    grammar: &'a str,
    status: String,
}

/// The workspace spec the host mounts at startup: the trace site read in its
/// own native regime. Resolved once for the spec resolver hook, which requires
/// `&'static str`.
static INITIAL_WORKSPACE: LazyLock<String> = LazyLock::new(|| {
    TrioHost::load(&SPECS)
        .expect("the trio specs are embedded from their store paths")
        .workspace_json()
});