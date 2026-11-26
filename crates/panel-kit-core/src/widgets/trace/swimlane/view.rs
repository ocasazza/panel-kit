//! The view pipeline: one read of the window's events, three derivations.
//!
//! The host hands over the events it holds and the toolbar's [`GraphState`];
//! filters narrow the graph and the sequence, and the collaboration table is
//! built from every event of the window, since a filtered table is a table
//! about nothing.

use std::collections::{BTreeMap, HashMap};

use super::super::model::{kinds_of, Event, EventKind, SessionTree};
use super::super::rails::timeline::{FlatLane, TlNode};
use super::super::time;
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

/// The LANES mode drawn as a dense timeline: one lane per agent, in name
/// order, each the window's filtered events of that agent oldest first. An
/// event with a blank agent is unattributed and belongs to no lane.
pub fn timeline_lanes(events: &[Event], state: &GraphState, now: i64) -> Vec<FlatLane> {
    let window = interval(state, now);
    let mut by_agent: BTreeMap<&str, Vec<&Event>> = BTreeMap::new();
    for event in events {
        let Some(agent) = event.agent.as_deref().filter(|agent| !agent.trim().is_empty()) else {
            continue;
        };
        if in_window(&window, event) && state.filters.keep(event) {
            by_agent.entry(agent).or_default().push(event);
        }
    }
    by_agent
        .into_iter()
        .map(|(agent, mut lane)| {
            lane.sort_by_key(|event| event.order_key());
            FlatLane {
                id: agent.to_owned(),
                label: agent.to_owned(),
                nodes: lane.into_iter().map(timeline_node).collect(),
            }
        })
        .collect()
}

/// One event as a timeline node: the caption names where it points and its
/// first summary line; the tooltip carries kind, agent, item, branch and time.
fn timeline_node(event: &Event) -> TlNode {
    let kind = event.kind.as_str();
    let label = match (&event.kind, &event.branch) {
        (EventKind::BranchHead, Some(branch)) => {
            let sha = event.commit.as_deref().unwrap_or("");
            format!("branch {branch} @ {}", &sha[..sha.len().min(8)])
        }
        _ => {
            let link = match (&event.pr, &event.commit, &event.item_id) {
                (Some(pr), _, _) => Some(format!("#{pr}")),
                (None, Some(sha), _) => Some(sha[..sha.len().min(12)].to_owned()),
                (None, None, Some(item)) => Some(item.clone()),
                (None, None, None) => None,
            };
            let first = event
                .summary
                .as_deref()
                .and_then(|text| text.lines().find(|line| !line.trim().is_empty()))
                .map(str::trim);
            [link.as_deref(), first]
                .into_iter()
                .flatten()
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
                .join(" ")
        }
    };
    let detail = [
        Some(kind.to_owned()),
        event.agent.clone(),
        event.item_id.clone(),
        event.branch.clone(),
        Some(time::format_iso8601(event.at)),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" · ");
    TlNode {
        id: event.id.clone(),
        kind: kind.to_owned(),
        at: Some(event.at),
        label: if label.is_empty() { kind.replace('_', " ") } else { label },
        detail: Some(detail),
        ..TlNode::default()
    }
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
