//! Event graph demo: the board's activity as a swimlane graph, a session tree
//! and an item timeline, over deterministic sample data.
//!
//! Run with: `dx serve --example event_graph --platform web`
//!
//! The graph has GRAPH, LANES and COLLAB modes, a query line (`range:30d
//! tree:on`), a TREE toggle, a tone legend that filters, no-item strips and a
//! density strip. Clicking a node opens everything known about it in place;
//! the rails open a row's facts under it, with `expand all` and `collapse all`.

use std::rc::Rc;

use dioxus::prelude::*;
use panel_kit::widgets::event_graph::{EventGraph, EventRail};
use panel_kit::CSS;
use panel_kit_core::widgets::event_graph::facts::Expansion;
use panel_kit_core::widgets::event_graph::query::GraphState;
use panel_kit_core::widgets::event_graph::rails::{vertical_with, RailOptions};
use panel_kit_core::widgets::event_graph::sample::{self, Rail};

const DEMO_CSS: &str = "
body { overflow:hidden !important; }
.demo { height:100vh; box-sizing:border-box; padding:1.5rem 2rem; display:flex; flex-direction:column; gap:.5rem; }
.demo h1 { margin:0; font:700 11px/1.4 var(--mono); letter-spacing:.06em; text-transform:uppercase; color:var(--dim); }
.demo .graph { flex:1 1 55%; min-height:0; display:flex; flex-direction:column; border:1px solid var(--line); }
.demo .rails { flex:1 1 45%; min-height:0; display:grid; grid-template-columns:1fr 1fr; gap:1rem; }
.demo .rails > div { min-height:0; overflow:auto; border:1px solid var(--line); padding:.5rem; }
";

fn main() {
    dioxus::launch(App);
}

/// One rail with its own open rows and selection.
#[component]
fn RailDemo(data: Rc<Rail>, options: RailOptions) -> Element {
    let mut open = use_signal(Expansion::new);
    let mut selected = use_signal(|| None::<String>);
    let view = vertical_with(
        &data.events,
        &data.item,
        &data.tree,
        &open.read(),
        options.wrap,
    )
    .view(&RailOptions {
        selected: selected.read().clone(),
        on_select: true,
        ..options
    });
    rsx! {
        EventRail {
            view,
            expanded: open.read().clone(),
            on_toggle: move |id: String| {
                open.write().toggle(&id);
                selected.set(Some(id));
            },
            on_open_all: move |ids: Vec<String>| open.write().open_all(ids.iter().map(String::as_str)),
            on_close_all: move |_| open.write().close_all(),
        }
    }
}

#[component]
fn App() -> Element {
    let activity = use_hook(|| {
        let a = sample::activity();
        (Rc::new(a.events), Rc::new(a.tree), a.now)
    });
    let tree = use_hook(|| Rc::new(sample::session_tree()));
    let timeline = use_hook(|| Rc::new(sample::item_timeline()));
    let mut state = use_signal(|| GraphState {
        window: "30d".to_owned(),
        tree: true,
        ..GraphState::default()
    });
    rsx! {
        style { {CSS} }
        style { {DEMO_CSS} }
        main { class: "demo",
            h1 { "events" }
            div { class: "graph",
                EventGraph {
                    id: "activity",
                    state: state.read().clone(),
                    events: activity.0.clone(),
                    tree: activity.1.clone(),
                    now: activity.2,
                    on_action: move |action| {
                        let _ = state.write().reduce(action);
                    },
                }
            }
            div { class: "rails",
                div { RailDemo { data: tree, options: RailOptions::default() } }
                div {
                    RailDemo {
                        data: timeline,
                        options: RailOptions {
                            title: "timeline".to_owned(),
                            dom_id: "item-timeline".to_owned(),
                            separators: true,
                            ..RailOptions::default()
                        },
                    }
                }
            }
        }
    }
}
