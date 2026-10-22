//! Trace demo: the board's activity as a swimlane graph, a session tree and an
//! item timeline, from a few thousand events to a million.
//!
//! Run with: `dx serve --example trace --platform web`
//!
//! GRAPH, LANES (a sequence diagram: one lifeline per agent, handoffs and
//! shared actions as arrows between them) and COLLAB (pairs and threads as
//! tables), a query line (`range:30d tree:on`), a TREE toggle, a tone legend
//! that filters, and an inspector that opens everything known about a clicked
//! node. The rails open a row's facts under it. The size buttons regenerate
//! the sample and report how long that took and what it holds.

use std::rc::Rc;

use dioxus::prelude::*;
use js_sys::{ArrayBuffer, Function, Promise};
use panel_kit::loading::ProgressBar;
use panel_kit::widgets::trace::{Trace, TraceRail};
use panel_kit::widgets::Button;
use panel_kit::CSS;
use panel_kit_core::widgets::trace::facts::Expansion;
use panel_kit_core::widgets::trace::query::GraphState;
use panel_kit_core::widgets::trace::rails::{vertical_with, RailOptions};
use panel_kit_core::widgets::trace::sample::{self, Activity, Rail};
use panel_kit_core::widgets::trace::{Event, SessionTree};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

const DEMO_CSS: &str = "
body { overflow:hidden !important; }
.demo { height:100vh; box-sizing:border-box; padding:1.5rem 2rem; display:flex; flex-direction:column; gap:.5rem; }
.demo h1 { margin:0; font:700 11px/1.4 var(--mono); letter-spacing:.06em; text-transform:uppercase; color:var(--dim); }
.demo .sizes { display:flex; flex-wrap:wrap; gap:4px; align-items:center; font:11px/1.4 var(--mono); color:var(--dim); }
.demo .sizes span { margin-left:8px; }
.demo .graph { flex:1 1 55%; min-height:0; display:flex; flex-direction:column; border:1px solid var(--line); }
.demo .rails { flex:1 1 45%; min-height:0; display:grid; grid-template-columns:1fr 1fr; gap:1rem; }
.demo .rails > div { min-height:0; overflow:auto; border:1px solid var(--line); padding:.5rem; }
";

const SIZES: [usize; 4] = [sample::DEFAULT_EVENTS, 10_000, 100_000, 1_000_000];

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = performance, js_name = now)]
    fn clock() -> f64;
    #[wasm_bindgen(js_name = requestAnimationFrame)]
    fn request_animation_frame(callback: &Function) -> i32;
}

/// Resolves at the next animation frame.
async fn next_frame() {
    let promise = Promise::new(&mut |resolve, _| {
        request_animation_frame(&resolve);
    });
    let _ = JsFuture::from(promise).await;
}

/// Bytes of the wasm linear memory.
fn wasm_bytes() -> f64 {
    wasm_bindgen::memory()
        .dyn_into::<js_sys::WebAssembly::Memory>()
        .map(|memory| ArrayBuffer::from(memory.buffer()).byte_length() as f64)
        .unwrap_or(0.0)
}

/// Bytes the events hold: the structs plus their strings.
fn data_bytes(events: &[Event]) -> usize {
    let text = |s: &Option<String>| s.as_ref().map_or(0, String::len);
    std::mem::size_of_val(events)
        + events
            .iter()
            .map(|e| {
                e.id.len()
                    + text(&e.agent)
                    + text(&e.item_id)
                    + text(&e.run_id)
                    + text(&e.branch)
                    + text(&e.commit)
                    + text(&e.summary)
                    + text(&e.detail.status)
                    + text(&e.detail.title)
            })
            .sum::<usize>()
}

/// The sample at one size and how long building it took.
#[derive(Clone)]
struct Loaded {
    events: Rc<Vec<Event>>,
    tree: Rc<SessionTree>,
    now: i64,
    built: f64,
    bytes: usize,
}

impl PartialEq for Loaded {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.events, &other.events)
    }
}

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
        TraceRail {
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
    let mut size = use_signal(|| SIZES[0]);
    let mut loaded = use_signal(|| None::<Loaded>);
    let mut loading = use_signal(|| false);
    let mut state = use_signal(|| GraphState {
        window: "30d".to_owned(),
        tree: true,
        ..GraphState::default()
    });
    let tree = use_hook(|| Rc::new(sample::session_tree()));
    let timeline = use_hook(|| Rc::new(sample::item_timeline()));

    // A new size replaces the data; the bar paints before the blocking build.
    let _build = use_resource(move || async move {
        let count = size();
        loading.set(true);
        next_frame().await;
        next_frame().await;
        let started = clock();
        let Activity {
            events, tree, now, ..
        } = sample::activity_sized(count);
        let built = clock() - started;
        let bytes = data_bytes(&events);
        loaded.set(Some(Loaded {
            events: Rc::new(events),
            tree: Rc::new(tree),
            now,
            built,
            bytes,
        }));
        loading.set(false);
    });

    let current = loaded();
    let wasm_mb = wasm_bytes() / 1_048_576.0;
    rsx! {
        style { {CSS} }
        style { {DEMO_CSS} }
        main { class: "demo",
            h1 { "trace" }
            div { class: "sizes",
                for n in SIZES {
                    Button {
                        pressed: size() == n,
                        "data-events": "{n}",
                        on_press: move |_| size.set(n),
                        "{n} events"
                    }
                }
                if let Some(Loaded { built, bytes, events, .. }) = &current {
                    span { id: "built", "data-ms": "{built}", "data built in {built:.0} ms" }
                    span {
                        id: "mem",
                        "data-heap": "{bytes}",
                        "data {*bytes as f64 / 1_048_576.0:.1} MB, wasm {wasm_mb:.0} MB, {events.len()} events"
                    }
                }
            }
            if loading() {
                ProgressBar { fraction: None, label: format!("building {} events", size()) }
            }
            if let Some(Loaded { events, tree: forks, now, .. }) = current {
                div { class: "graph",
                    Trace {
                        id: "activity",
                        state: state.read().clone(),
                        events,
                        tree: forks,
                        now,
                        on_action: move |action| {
                            let _ = state.write().reduce(action);
                        },
                    }
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
