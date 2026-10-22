//! The view pipeline: one read of the window's events, three derivations.
//!
//! The host hands over the events it holds and the toolbar's [`GraphState`];
//! filters narrow the graph and the sequence, and the collaboration table is
//! built from every event of the window, since a filtered table is a table
//! about nothing.

use std::collections::HashMap;

use super::super::model::{kinds_of, Event, SessionTree};
use super::super::query::{GraphState, Interval, Window, DEFAULT_WINDOW, ROW_CAP};
use super::collab::{current_items, Collaboration, ItemInfo};
use super::layout::{Graph, Options, Range};
use super::sequence::{sequence as build_sequence, Options as SequenceOptions, Sequence};
use super::strip::LooseOpen;

const MICROS_PER_SEC: i64 = 1_000_000;
const DAY_SECS: i64 = 86_400;
/// The tree of a host that holds none: every item's forks are derived.
static NO_TREE: SessionTree = SessionTree {
    agent: None,
    lanes: Vec::new(),
    edges: Vec::new(),
};

/// The window of `state` at `now` (epoch seconds). A label that is not a
/// window reads as the default one.
fn interval(state: &GraphState, now: i64) -> Interval {
    state
        .interval(now)
        .or_else(|| {
            Window::parse(DEFAULT_WINDOW)
                .ok()
                .map(|window| window.resolve(now))
        })
        .unwrap_or(Interval {
            since: now.saturating_sub(DAY_SECS),
            until: None,
        })
}

/// Whether `event` falls inside the window: `since <= at < until`.
fn in_window(window: &Interval, event: &Event) -> bool {
    window.contains(event.at.div_euclid(MICROS_PER_SEC))
}

/// The GRAPH mode: the swimlane graph of the window's events under the
/// state's filters, width, row cap, strip rows and fork toggle.
///
/// `events` may be any superset of the window; the window bounds the read.
/// `tree` is the merged session-tree window for these events, drawn only when
/// the state's `tree` toggle is on; items it has no lanes for get theirs from
/// their own events (see [`derive_forks`](super::derive_forks)), so the toggle
/// draws forks without a tree. `now` is the clock, in epoch seconds.
///
/// With the row cap lifted (`show_all`) the items are not built: the graph
/// holds every row's geometry and [`Graph::item`] builds the ones a painter
/// draws from the same `events`.
pub fn graph(events: &[Event], state: &GraphState, tree: Option<&SessionTree>, now: i64) -> Graph {
    let window = interval(state, now);
    let kinds = kinds_of(&state.loose_kinds);
    let mut options = Options::new(
        state.graph_width,
        Range::from_secs(window.since, window.until, now),
    )
    .limit((!state.show_all).then_some(ROW_CAP))
    .loose_open(LooseOpen::Kinds(&kinds))
    .lazy(state.show_all);
    if state.tree {
        options = options.tree(tree.unwrap_or(&NO_TREE));
    }
    Graph::layout_slice(
        events,
        |event| in_window(&window, event) && state.filters.keep(event),
        &options,
    )
}

/// The LANES mode: the sequence diagram of the window's events under the
/// state's filters. `activity` draws plain activity (`Some`) or leaves it to
/// the event count (`None`).
pub fn sequence(
    events: &[Event],
    state: &GraphState,
    now: i64,
    activity: Option<bool>,
) -> Sequence {
    let window = interval(state, now);
    build_sequence(
        events,
        |event| in_window(&window, event) && state.filters.keep(event),
        &SequenceOptions::default().activity(activity),
    )
}

/// The COLLAB mode: pairs and threads over every event of the window, the
/// filters ignored. Items are judged from every event `events` holds, the
/// window included or not, as of `now` in epoch seconds.
pub fn collaboration(events: &[Event], state: &GraphState, now: i64) -> Collaboration {
    let window = interval(state, now);
    let items: HashMap<String, ItemInfo> = current_items(events);
    Collaboration::build(
        events.iter().filter(|event| in_window(&window, event)),
        &items,
        now.saturating_mul(MICROS_PER_SEC),
    )
}
