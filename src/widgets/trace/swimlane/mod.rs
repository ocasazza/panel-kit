//! `Trace`: the activity swimlane. A toolbar (mode chips, query line,
//! TREE toggle, colour legend) over one of three bodies: the GRAPH canvas, the
//! LANES sequence diagram or the COLLAB pair table, all derived from the events the host holds
//! by [`panel_kit_core::widgets::trace::swimlane::view`].
//!
//! The host owns the [`GraphState`] and applies every [`GraphAction`] the
//! component raises with `GraphState::reduce`; the component keeps only the
//! query box's draft, the open inspector node and, unless the host passes
//! `selected`, the clicked item. Clicking an item, a dot, a burst or a fork
//! leaf of the GRAPH, an arrow, bar or dot of LANES, or a pair of COLLAB docks
//! that node's facts under the body (`open` seeds it, `on_open` reports it; the
//! close button or Escape clears it).
//! Layouts are memoised on the identity of `events` and `tree`, so a host that
//! keeps its `Rc`s keeps its layout. The GRAPH canvas scrolls inside the body
//! with its strip and axis pinned, and mounts only the item rows near the
//! viewport (the rest is height), so show-all over a million events is a few
//! thousand nodes; the window moves when the rows on screen leave it. Styled by
//! the `pk-tr-*` rules in `assets/panel-kit.css`.
//!
//! ```no_run
//! use std::rc::Rc;
//!
//! use dioxus::prelude::*;
//! use panel_kit::widgets::trace::swimlane::Trace;
//! use panel_kit_core::widgets::trace::query::GraphState;
//! use panel_kit_core::widgets::trace::Event;
//!
//! #[component]
//! fn Activity(events: Rc<Vec<Event>>, now: i64) -> Element {
//!     let mut state = use_signal(GraphState::default);
//!     rsx! {
//!         Trace {
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
mod inspector;
mod legend;
mod sequence;
mod shared;
mod toolbar;
mod window;

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

use dioxus::prelude::*;
use panel_kit_core::widgets::trace::query::{
    Completion, Filters, Followed, GraphAction, GraphState, LanePick, Mode, Queryable,
};
use panel_kit_core::widgets::trace::swimlane::{view, Collaboration, Graph, Node, Sequence};
use panel_kit_core::widgets::trace::{Event, SessionTree};

use collab::CollabBody;
use graph::GraphBody;
use inspector::Inspector;
use legend::legend;
use sequence::SequenceBody;
use shared::{Shared, Slot};
use toolbar::{modes, tokens, QueryLine};

const MICROS_PER_SEC: i64 = 1_000_000;
/// Datalist options one query key offers; a longer list is left to typing.
const OPTIONS_PER_KEY: usize = 200;

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

/// What the LANES sequence depends on.
#[derive(PartialEq)]
struct SequenceKey {
    source: Source,
    window: String,
    filters: Filters,
    activity: Option<bool>,
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
    sequence: Slot<SequenceKey, Shared<Sequence>>,
    shown: Slot<(Shared<Sequence>, LanePick), Shared<Sequence>>,
    collab: Slot<CollabKey, Rc<Collaboration>>,
    completions: Slot<CompletionKey, Shared<Vec<Completion>>>,
}

/// The part of `state` the GRAPH layout reads.
fn layout_state(state: &GraphState) -> GraphState {
    GraphState {
        mode: Mode::Graph,
        query_errors: Vec::new(),
        lanes: LanePick::Auto,
        lane: None,
        lane_filter: String::new(),
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

/// Datalist options over every event of the window, filters ignored, at most
/// [`OPTIONS_PER_KEY`] each of agents, items and kinds.
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
    let mut seen = [0usize; 3];
    state
        .completions(&rows)
        .into_iter()
        .filter(|completion| {
            let key = match completion.label.split(':').next() {
                Some("agent") => 0,
                Some("item") => 1,
                Some("kind") => 2,
                _ => return true,
            };
            seen[key] += 1;
            seen[key] <= OPTIONS_PER_KEY
        })
        .collect()
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

    /// `sequence` with the lanes `pick` draws standing and the rest folded.
    fn shown(&mut self, sequence: &Shared<Sequence>, pick: &LanePick) -> Shared<Sequence> {
        self.shown.get((sequence.clone(), pick.clone()), || {
            Shared::new(sequence.show(pick))
        })
    }

    fn sequence(
        &mut self,
        events: &Rc<Vec<Event>>,
        state: &GraphState,
        now: i64,
        activity: Option<bool>,
    ) -> Shared<Sequence> {
        let key = SequenceKey {
            source: Source {
                events: Shared(Rc::clone(events)),
                now,
            },
            window: state.window.clone(),
            filters: state.filters.clone(),
            activity,
        };
        self.sequence.get(key, || {
            Shared::new(view::sequence(events, state, now, activity))
        })
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

/// The swimlane trace: toolbar and body for the mode `state` names.
#[component]
pub fn Trace(
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
    /// The node open in the inspector when the graph mounts; the component
    /// owns it from then on.
    #[props(default)]
    open: Option<Node>,
    /// The inspector opened a node (`Some`) or was closed (`None`).
    #[props(default)]
    on_open: EventHandler<Option<Node>>,
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
    let mut revealed = use_signal(|| open.clone());
    let reveal = use_callback(move |node: Node| {
        revealed.set(Some(node.clone()));
        on_open.call(Some(node));
    });
    let dismiss = use_callback(move |_: ()| {
        if revealed.peek().is_some() {
            revealed.set(None);
            on_open.call(None);
        }
    });
    let mut activity = use_signal(|| None::<bool>);
    let set_activity = use_callback(move |on: bool| activity.set(Some(on)));
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

    let mut extra: Option<Element> = None;
    let mut pairs = None::<Rc<Collaboration>>;
    let picked_event = match &*revealed.read() {
        Some(Node::Event(id)) => Some(id.clone()),
        _ => None,
    };
    let open_pair = match &*revealed.read() {
        Some(Node::Pair(a, b)) => Some((a.clone(), b.clone())),
        _ => None,
    };
    let body = match mode {
        Mode::Graph => {
            let layout = caches
                .borrow_mut()
                .graph(&events, tree.as_ref(), &state, now);
            rsx! {
                GraphBody {
                    id: id.clone(),
                    graph: layout,
                    events: Shared(events.clone()),
                    chips: if compact { Vec::new() } else { legend_chips.clone() },
                    inline_legend: !compact,
                    show_all: state.show_all,
                    selected: highlighted.clone(),
                    on_action: act,
                    on_select: pick,
                    on_open: reveal,
                }
            }
        }
        Mode::Lanes => {
            let base = caches
                .borrow_mut()
                .sequence(&events, &state, now, *activity.read());
            let sequence = caches.borrow_mut().shown(&base, &state.lanes);
            extra = Some(rsx! {
                {toolbar::activity_chip(&id, sequence.activity, sequence.quiet, set_activity)}
                {toolbar::lane_menu(&id, &sequence, &state.lane_filter, state.lane.as_deref(), act)}
            });
            rsx! {
                SequenceBody {
                    id: id.clone(),
                    sequence,
                    events: Shared(events.clone()),
                    picked: picked_event,
                    focus: state.lane.clone(),
                    on_open: reveal,
                    on_action: act,
                }
            }
        }
        Mode::Collab => {
            let table = caches.borrow_mut().collab(&events, &state, now);
            pairs = Some(table.clone());
            rsx! {
                CollabBody {
                    id: id.clone(),
                    collab: table,
                    open: open_pair,
                    on_open: reveal,
                }
            }
        }
    };

    let mut root = String::from("pk-widget pk-tr");
    if compact {
        root.push_str(" pk-tr-compact");
    }
    if mode.has_legend() && !state.tones.is_empty() {
        root.push_str(" pk-tr-filtering");
        for tone in &state.tones {
            root.push_str(" pk-tr-f-");
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
            aria_label: "trace",
            "data-mode": mode.token(),
            if compact {
                div {
                    id: "{id}-bar",
                    class: "pk-tr-bar",
                    role: "toolbar",
                    aria_label: "trace controls",
                    {chips}
                    {query}
                    {extra.clone()}
                    if mode.has_legend() {
                        {legend(&id, &legend_chips, true, act)}
                    }
                }
            } else {
                div { class: "pk-tr-stack", role: "toolbar", aria_label: "trace controls",
                    {chips}
                    {query}
                    {extra}
                }
            }
            if mode.has_query_line() {
                {tokens(&id, &active, act)}
            }
            div {
                class: "pk-tr-body",
                onkeydown: move |event| {
                    if inspector::dismisses(&event) {
                        dismiss.call(());
                    }
                },
                {body}
            }
            if let Some(node) = revealed.read().clone() {
                Inspector {
                    key: "{node.key()}",
                    id: id.clone(),
                    node: node.clone(),
                    events: events.clone(),
                    tree: if state.tree { tree.clone() } else { None },
                    collab: pairs,
                    on_close: dismiss,
                }
            }
        }
    }
}
