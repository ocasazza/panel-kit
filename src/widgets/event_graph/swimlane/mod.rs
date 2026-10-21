//! `EventGraph`: the activity swimlane. A toolbar (mode chips, query line,
//! TREE toggle, colour legend) over one of three bodies: the GRAPH canvas, the
//! LANES list or the COLLAB tables, all derived from the events the host holds
//! by [`panel_kit_core::widgets::event_graph::swimlane::view`].
//!
//! The host owns the [`GraphState`] and applies every [`GraphAction`] the
//! component raises with `GraphState::reduce`; the component keeps only the
//! query box's draft and, unless the host passes `selected`, the clicked item.
//! Layouts are memoised on the identity of `events` and `tree`, so a host that
//! keeps its `Rc`s keeps its layout. Styled by the `pk-ag-*` rules in
//! `assets/panel-kit.css`.
//!
//! ```no_run
//! use std::rc::Rc;
//!
//! use dioxus::prelude::*;
//! use panel_kit::widgets::event_graph::swimlane::EventGraph;
//! use panel_kit_core::widgets::event_graph::query::GraphState;
//! use panel_kit_core::widgets::event_graph::Event;
//!
//! #[component]
//! fn Activity(events: Rc<Vec<Event>>, now: i64) -> Element {
//!     let mut state = use_signal(GraphState::default);
//!     rsx! {
//!         EventGraph {
//!             state: state(),
//!             events,
//!             now,
//!             on_action: move |action| {
//!                 state.write().reduce(action);
//!             },
//!         }
//!     }
//! }
//! ```

mod canvas;
mod collab;
mod dom;
mod graph;
mod lanes;
mod legend;
mod shared;
mod toolbar;

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

use dioxus::prelude::*;
use panel_kit_core::widgets::event_graph::query::{
    Completion, Filters, Followed, GraphAction, GraphState, Mode, Queryable,
};
use panel_kit_core::widgets::event_graph::swimlane::{view, Collaboration, Graph, Split};
use panel_kit_core::widgets::event_graph::{Event, SessionTree};

use collab::CollabBody;
use graph::GraphBody;
use lanes::LanesBody;
use legend::legend;
use shared::{Shared, Slot};
use toolbar::{modes, tokens, QueryLine};

const MICROS_PER_SEC: i64 = 1_000_000;

/// The events and clock every derivation reads.
#[derive(PartialEq)]
struct Source {
    events: Shared<Vec<Event>>,
    now: i64,
}

/// What the GRAPH layout depends on: the state less everything that only
/// changes how the layout is drawn (mode, tone filter, errors, register).
#[derive(PartialEq)]
struct GraphKey {
    source: Source,
    tree: Option<Shared<SessionTree>>,
    state: GraphState,
}

/// What the LANES split depends on.
#[derive(PartialEq)]
struct LanesKey {
    source: Source,
    window: String,
    filters: Filters,
}

/// What the COLLAB tables depend on: the window alone, never the filters.
#[derive(PartialEq)]
struct CollabKey {
    source: Source,
    window: String,
}

/// What the datalist depends on.
#[derive(PartialEq)]
struct CompletionKey {
    source: Source,
    window: String,
    filters: Filters,
    tree: bool,
}

/// The derivations of one mounted graph, each rebuilt only when its key moves.
#[derive(Default)]
struct Caches {
    graph: Slot<GraphKey, Shared<Graph>>,
    lanes: Slot<LanesKey, Rc<Split>>,
    collab: Slot<CollabKey, Rc<Collaboration>>,
    completions: Slot<CompletionKey, Shared<Vec<Completion>>>,
}

/// The part of `state` the GRAPH layout reads.
fn layout_state(state: &GraphState) -> GraphState {
    GraphState {
        mode: Mode::Graph,
        query_errors: Vec::new(),
        unattributed_open: false,
        followed: Followed::Unset,
        compact: false,
        tones: BTreeSet::new(),
        ..state.clone()
    }
}

/// An event as the query layer reads it.
struct Row<'a>(&'a Event);

impl Queryable for Row<'_> {
    fn agent(&self) -> Option<&str> {
        self.0.agent.as_deref()
    }

    fn item_id(&self) -> Option<&str> {
        self.0.item_id.as_deref()
    }

    fn kind(&self) -> &str {
        self.0.kind.as_str()
    }
}

/// Datalist options over every event of the window, filters ignored.
fn completions(events: &[Event], state: &GraphState, now: i64) -> Vec<Completion> {
    let window = state.interval(now);
    let rows: Vec<Row<'_>> = events
        .iter()
        .filter(|event| {
            window
                .as_ref()
                .is_none_or(|window| window.contains(event.at.div_euclid(MICROS_PER_SEC)))
        })
        .map(Row)
        .collect();
    state.completions(&rows)
}

impl Caches {
    fn graph(
        &mut self,
        events: &Rc<Vec<Event>>,
        tree: Option<&Rc<SessionTree>>,
        state: &GraphState,
        now: i64,
    ) -> Shared<Graph> {
        let key = GraphKey {
            source: Source {
                events: Shared(Rc::clone(events)),
                now,
            },
            tree: tree
                .filter(|_| state.tree)
                .map(|tree| Shared(Rc::clone(tree))),
            state: layout_state(state),
        };
        self.graph.get(key, || {
            Shared::new(view::graph(events, state, tree.map(|tree| &**tree), now))
        })
    }

    fn lanes(&mut self, events: &Rc<Vec<Event>>, state: &GraphState, now: i64) -> Rc<Split> {
        let key = LanesKey {
            source: Source {
                events: Shared(Rc::clone(events)),
                now,
            },
            window: state.window.clone(),
            filters: state.filters.clone(),
        };
        self.lanes
            .get(key, || Rc::new(view::lanes(events, state, now)))
    }

    fn collab(
        &mut self,
        events: &Rc<Vec<Event>>,
        state: &GraphState,
        now: i64,
    ) -> Rc<Collaboration> {
        let key = CollabKey {
            source: Source {
                events: Shared(Rc::clone(events)),
                now,
            },
            window: state.window.clone(),
        };
        self.collab
            .get(key, || Rc::new(view::collaboration(events, state, now)))
    }

    fn completions(
        &mut self,
        events: &Rc<Vec<Event>>,
        state: &GraphState,
        now: i64,
    ) -> Shared<Vec<Completion>> {
        let key = CompletionKey {
            source: Source {
                events: Shared(Rc::clone(events)),
                now,
            },
            window: state.window.clone(),
            filters: state.filters.clone(),
            tree: state.tree,
        };
        self.completions
            .get(key, || Shared::new(completions(events, state, now)))
    }
}

/// The swimlane event graph: toolbar and body for the mode `state` names.
#[component]
pub fn EventGraph(
    /// DOM id; every id inside derives from it.
    #[props(default = "activity".to_string())]
    id: String,
    /// Toolbar, mode, tree, legend and strip state; the host owns it.
    state: GraphState,
    /// Every event the host holds; the window bounds the read.
    events: Rc<Vec<Event>>,
    /// Session-tree window of those events, drawn while TREE is on.
    #[props(default)]
    tree: Option<Rc<SessionTree>>,
    /// The clock, epoch seconds; the component never reads one.
    now: i64,
    /// The highlighted item; `None` defers to the graph's own click selection.
    #[props(default)]
    selected: Option<String>,
    /// Every toolbar action, for the host to apply with `GraphState::reduce`.
    on_action: EventHandler<GraphAction>,
    /// An item row was activated.
    #[props(default)]
    on_select: EventHandler<String>,
) -> Element {
    // Handles that outlive the host's closures keep memoised children from
    // re-rendering when only a closure was rebuilt.
    let act = use_callback(move |action: GraphAction| on_action.call(action));
    let mut picked = use_signal(|| None::<String>);
    let controlled = selected.is_some();
    let pick = use_callback(move |item: String| {
        if !controlled {
            picked.set(Some(item.clone()));
        }
        on_select.call(item);
    });
    let highlighted = selected.or_else(|| picked.read().clone());
    let caches = use_hook(|| Rc::new(RefCell::new(Caches::default())));

    let mode = state.mode;
    let compact = state.compact;
    let mode_chips = state.mode_chips();
    let legend_chips = state.legend_chips();

    let query = mode.has_query_line().then(|| {
        let options = caches.borrow_mut().completions(&events, &state, now);
        rsx! {
            QueryLine {
                id: id.clone(),
                line: state.line(),
                tree: state.tree,
                errors: state.errors_text(),
                completions: options,
                on_action: act,
            }
        }
    });

    let body = match mode {
        Mode::Graph => {
            let layout = caches
                .borrow_mut()
                .graph(&events, tree.as_ref(), &state, now);
            rsx! {
                GraphBody {
                    id: id.clone(),
                    graph: layout,
                    chips: if compact { Vec::new() } else { legend_chips.clone() },
                    inline_legend: !compact,
                    show_all: state.show_all,
                    selected: highlighted.clone(),
                    on_action: act,
                    on_select: pick,
                }
            }
        }
        Mode::Lanes => {
            let split = caches.borrow_mut().lanes(&events, &state, now);
            rsx! {
                LanesBody {
                    id: id.clone(),
                    split,
                    unattributed_open: state.unattributed_open,
                    on_action: act,
                }
            }
        }
        Mode::Collab => {
            let table = caches.borrow_mut().collab(&events, &state, now);
            rsx! {
                CollabBody {
                    id: id.clone(),
                    collab: table,
                    selected: highlighted.clone(),
                    on_select: pick,
                }
            }
        }
    };

    let mut root = String::from("pk-widget pk-ag");
    if compact {
        root.push_str(" pk-ag-compact");
    }
    if mode.has_legend() && !state.tones.is_empty() {
        root.push_str(" pk-ag-filtering");
        for tone in &state.tones {
            root.push_str(" pk-ag-f-");
            root.push_str(tone.name());
        }
    }
    let chips = modes(&id, &mode_chips, act);
    let active = state.tokens();

    rsx! {
        div {
            id: "{id}",
            class: "{root}",
            role: "group",
            aria_label: "event graph",
            "data-mode": mode.token(),
            if compact {
                div {
                    id: "{id}-bar",
                    class: "pk-ag-bar",
                    role: "toolbar",
                    aria_label: "event graph controls",
                    {chips}
                    {query}
                    if mode.has_legend() {
                        {legend(&id, &legend_chips, true, act)}
                    }
                }
            } else {
                div { class: "pk-ag-stack", role: "toolbar", aria_label: "event graph controls",
                    {chips}
                    {query}
                }
            }
            if mode.has_query_line() {
                {tokens(&id, &active, act)}
            }
            div { class: "pk-ag-body", {body} }
        }
    }
}
