//! Topos × grammar demo — one record set, two independent axes.
//!
//! Run with: `dx serve --example topos --platform web`
//! (dioxus-cli 0.6.x; provided by `nix develop`)
//!
//! What it demonstrates:
//! - A *grammar* is a declarative importer description: a wire format plus the
//!   field rules that type it. The Grammar panel shows the importer package,
//!   the Source panel shows the raw text with every diagnostic marked in place,
//!   and `g`/`n`/`x`/`r` (or the top bar) re-parse a different source into the
//!   same record schema. Only the data changes; the layout does not.
//! - A *topos* is one UI/UX regime over the site: `topos-agentic` reads the
//!   sessions as planner/coder/reviewer roles, `topos-membrane` as lipid
//!   species self-assembling into a bilayer. `t` (or the top bar) swaps both at
//!   once: the panel set, the layout persisted under this topos's own key, and
//!   the stalks. The two regimes run the SAME physics (one base topos, pulled
//!   back along `p_R`), so the Physics panel is identical under both; only the
//!   UI/UX sheaves differ.
//! - Each topos carries an **active subobject** (a set of regime sorts) on its
//!   per-sort badges (agentic Roles, membrane Species). Click a badge to toggle
//!   its sort. Switching regimes transports it through the geometric morphism:
//!   agentic→membrane by the image `∃f`, membrane→agentic by the inverse image
//!   `f*`; the status line names the morphism used.
//! - Every panel body is painted from `ToposDemo::resolve(binding_id)`, so no
//!   panel can show content that belongs to another panel or another surface's
//!   grammar.

use std::sync::LazyLock;

use dioxus::events::{Key, KeyboardEvent, PointerEvent as DioxusPointerEvent};
use dioxus::prelude::*;
use panel_kit::CSS;
use panel_kit_core::badge::BadgeAction;
use panel_kit_core::reducer::{Viewport, WorkspaceEvent};
use panel_kit_core::widgets::ContentSpec;
use panel_kit_core::Units;

#[allow(dead_code, unused_imports)]
#[path = "support/composable_workspace.rs"]
mod composable_workspace;
#[allow(dead_code)]
#[path = "support/spec_workspace.rs"]
mod spec_workspace;
#[allow(dead_code)]
#[path = "support/topos_demo.rs"]
mod topos_demo;
#[allow(dead_code)]
#[path = "support/section_view.rs"]
mod section_view;

use topos_demo::{ToposDemo, TOPOS_AGENTIC};

const DEMO_CSS: &str = "
.topbar { flex-wrap: wrap; height: auto; min-height: 36px; gap: .45rem; }
.topbar h1 { font-size: .74rem; text-transform: uppercase; }
.topbar button { background: var(--bg); color: var(--fg); border: 1px solid var(--line2);
  border-radius: 3px; padding: .15rem .5rem; font-size: .72rem; cursor: pointer; }
.topbar button:hover { border-color: var(--fg); }
.topbar button.active { background: var(--fg); color: var(--bg); }
.topbar .axis { color: var(--dim); font-size: .62rem; text-transform: uppercase;
  letter-spacing: .12em; }
.topbar .axis-buttons { display: flex; gap: .35rem; }
.topbar .actions { display: flex; gap: .35rem; }
.topbar .status { color: var(--accent); font-size: .7rem; }
.pk-content-unbound { color: var(--yellow); font-size: .78rem; }
";

fn main() {
    dioxus::launch(App);
}

/// The browser viewport, in the units the web backend projects in.
fn browser_viewport() -> Viewport {
    let (width, height) = panel_kit::surface::viewport_size();
    Viewport {
        width,
        height,
        units: Units::CssPx,
    }
}

/// Paint one binding id from the active demo state through the shared painter.
///
/// `section_view::paint` receives the panel's authored `ContentSpec`, so text
/// keeps its scroll policy and a series its unit; a per-sort badge panel gets a
/// toggle handler. An id the demo does not resolve names itself instead of
/// leaving a blank body.
fn demo_content(demo: Signal<ToposDemo>, id: &str, content: &ContentSpec) -> Element {
    let state = demo.read();
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
    let on_badge = (state.active_binding() == Some(id)).then(|| badge_toggle_action(demo));
    section_view::paint(section, content, None, on_badge)
}

/// A toggle on a per-sort badge flips that sort in the active subobject.
fn badge_toggle_action(mut demo: Signal<ToposDemo>) -> EventHandler<BadgeAction> {
    EventHandler::new(move |action: BadgeAction| {
        if let BadgeAction::Toggle { field, .. } = action {
            demo.with_mut(|state| state.toggle_active_sort(&field));
        }
    })
}

/// The agentic topos's workspace spec JSON, resolved once for the spec resolver
/// hook, which requires `&'static str`. Derived from a build-time constant.
static INITIAL_WORKSPACE: LazyLock<String> = LazyLock::new(|| {
    topos_demo::workspace_spec_json(TOPOS_AGENTIC)
        .expect("the agentic topos spec is embedded from its store path")
});

#[component]
fn App() -> Element {
    let mut demo = use_signal(ToposDemo::new);
    let initial = spec_workspace::use_spec_workspace(INITIAL_WORKSPACE.as_str());
    let mut workspace = use_signal(move || initial.clone());
    let header_clicks = use_signal(|| 0_u32);

    // The viewport observer hook captures the workspace it mounted with; a
    // topos switch installs a new one, so the signal is read per event instead.
    let observed = workspace;
    let _viewport = panel_kit::surface::observe_viewport(EventHandler::new(move |size: Viewport| {
        let current = observed.read();
        spec_workspace::workspace_event_handler(&current).call(WorkspaceEvent::ViewportChanged {
            size,
            policy: current.resolved.surface.resize_policy,
        });
    }));

    // One transaction per axis: a topos swap exchanges the WorkspaceSpec and
    // the stalks together, a grammar switch only re-parses the source.
    let mut switch_topos = move |id: &str| {
        let Some(json) = demo.read().workspace_json(id).map(str::to_owned) else {
            return;
        };
        if !demo.with_mut(|state| state.set_topos(id)) {
            return;
        }
        // Every read guard ends with its statement: the switch writes both
        // signals, and a live read of either would panic the borrow.
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
    };

    let switch_grammar = move |id: &str| {
        demo.with_mut(|state| {
            state.set_grammar(id);
        });
    };

    let key_workspace = {
        let current = workspace.read();
        current.clone()
    };
    let mut on_demo_key = move |event: KeyboardEvent| {
        let mut handled = false;
        if !panel_kit::input::is_editing() {
            if let Key::Character(character) = event.key() {
                match character.as_str() {
                    "t" => {
                        let next = demo.read().next_topos_id();
                        switch_topos(next);
                        handled = true;
                    }
                    "g" => {
                        demo.with_mut(|state| state.cycle_grammar());
                        handled = true;
                    }
                    "n" => {
                        demo.with_mut(|state| state.append_valid_line());
                        handled = true;
                    }
                    "x" => {
                        demo.with_mut(|state| state.append_malformed_line());
                        handled = true;
                    }
                    "r" => {
                        demo.with_mut(|state| state.reset_source());
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

    let topos_ids = demo.read().topos_ids();
    let grammar_ids = demo.read().grammar_ids();
    let active_topos = demo.read().topos_id();
    let active_grammar = demo.read().grammar_id();
    let status = demo.read().status_line();

    // Precompute the switcher elements: rsx cannot nest iterator combinators
    // producing rsx inside a for-loop.
    let topos_buttons: Vec<Element> = topos_ids
        .iter()
        .map(|id| {
            let id = *id;
            let mut switch = switch_topos;
            rsx! {
                button {
                    key: "topos-{id}",
                    title: "Swap the WorkspaceSpec, its persisted layout, and the stalks",
                    class: if id == active_topos { "active" },
                    aria_pressed: id == active_topos,
                    onclick: move |_| switch(id),
                    "{id}"
                }
            }
        })
        .collect();

    let grammar_buttons: Vec<Element> = grammar_ids
        .iter()
        .map(|id| {
            let id = *id;
            let mut switch = switch_grammar;
            rsx! {
                button {
                    key: "grammar-{id}",
                    title: "Re-parse this importer's sample source into the same records",
                    class: if id == active_grammar { "active" },
                    aria_pressed: id == active_grammar,
                    onclick: move |_| switch(id),
                    "{id}"
                }
            }
        })
        .collect();

    let mut add_line = demo;
    let mut add_bad_line = demo;
    let mut reset_source = demo;
    let action_buttons: Vec<Element> = vec![
        rsx! {
            button {
                key: "add-line",
                title: "Append one well-formed line in this grammar's syntax (n)",
                onclick: move |_| add_line.with_mut(|state| state.append_valid_line()),
                "+ line"
            }
        },
        rsx! {
            button {
                key: "add-bad-line",
                title: "Append one line this grammar rejects, adding a diagnostic (x)",
                onclick: move |_| add_bad_line.with_mut(|state| state.append_malformed_line()),
                "+ bad line"
            }
        },
        rsx! {
            button {
                key: "reset-source",
                title: "Restore the grammar's sample source (r)",
                onclick: move |_| reset_source.with_mut(|state| state.reset_source()),
                "reset"
            }
        },
    ];

    let binding_content = move |id: &str, content: &ContentSpec| demo_content(demo, id, content);

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
            onkeydown: move |event: KeyboardEvent| on_demo_key(event),
            header { class: "topbar",
                h1 { "topos × grammar" }
                span { class: "axis", "topos" }
                div { class: "axis-buttons",
                    {topos_buttons.into_iter()}
                }
                span { class: "axis", "grammar" }
                div { class: "axis-buttons",
                    {grammar_buttons.into_iter()}
                }
                div { class: "actions",
                    {action_buttons.into_iter()}
                }
                span { class: "status", "{status}" }
                span { class: "hint", "t topos · g grammar · n line · x bad line · r reset · click a Roles/Species badge to toggle its sort" }
            }
            div {
                class: "{workspace_class}",
                style: "{workspace_style}",
                onwheel: move |event| spec_workspace::handle_wheel(&wheel_workspace, &event),
                {composable_workspace::web_canary::workspace_contents(
                    &frame,
                    &current.resolved,
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
