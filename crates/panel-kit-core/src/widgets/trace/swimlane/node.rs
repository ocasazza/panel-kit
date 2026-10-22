//! What one click opened, and what the inspector shows for it.
//!
//! A painter keeps a [`Node`] and asks [`Node::inspect`] for the content: an
//! item header and the item's events (with the clicked ones open), or plain
//! fact blocks for a fork lane and for events that belong to no item. A
//! collaboration pair is answered from the table's own [`Pair`] by
//! [`Node::inspect_pair`]. Nothing here measures or draws.

use serde::{Deserialize, Serialize};

use super::super::facts::{event_facts, lane_facts, Fact};
use super::super::model::{is_named, Event, ItemRef, Lane, SessionTree};
use super::super::query::{tone_for_status, Tone};
use super::super::time::format_iso8601;
use super::collab::Pair;
use super::derive::derive_forks;
use super::layout::summarize;

/// Events a cluster opens (and fact blocks an item-less cluster shows); the
/// rest are counted in [`Inspection::more`].
pub const CLUSTER_OPEN_CAP: usize = 12;

/// Shared items, branches or handoffs a pair's facts list; the rest are
/// counted.
pub const PAIR_LIST_CAP: usize = 200;

/// The node an inspector shows. Every variant names the thing clicked; a
/// painter resolves it against the events and tree it already holds. Serialized
/// as `{"kind": .., "id": ..}` so hosts can persist the open node.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum Node {
    /// A work item row: the item's id.
    Item(String),
    /// A single event dot: the event's id.
    Event(String),
    /// A cluster bubble: every member event's id, in draw order.
    Cluster(Vec<String>),
    /// A fork leaf: the lane's id.
    Lane(String),
    /// A collaboration pair: the two agents' names, the lesser first.
    Pair(String, String),
}

/// What an item is, as the inspector's header states it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemHeader {
    /// Item id.
    pub id: String,
    /// Newest title.
    pub title: Option<String>,
    /// Newest status.
    pub status: Option<String>,
    /// Tone of `status`.
    pub tone: Tone,
    /// Events of the item.
    pub events: usize,
    /// ISO-8601 instant of the oldest event.
    pub first: String,
    /// ISO-8601 instant of the newest event.
    pub last: String,
    /// Named agents that acted on the item, sorted.
    pub agents: Vec<String>,
    /// Lanes the session tree forks off the item.
    pub lanes: usize,
}

/// A labelled block of facts: a lane, or an event with no item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FactBlock {
    /// Heading: the lane's label, or the event's id.
    pub label: String,
    /// The node's facts.
    pub facts: Vec<Fact>,
}

/// The inspector's content for one [`Node`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Inspection {
    /// The node this content answers.
    pub node: Node,
    /// The item's header; `None` for a lane or item-less events.
    pub header: Option<ItemHeader>,
    /// The item whose rail is drawn; `None` when there is no rail.
    pub item: Option<ItemRef>,
    /// The session tree pruned to that item.
    pub tree: SessionTree,
    /// Event ids the rail opens at first.
    pub open: Vec<String>,
    /// Clicked events beyond [`CLUSTER_OPEN_CAP`]: left closed, listed.
    pub more: usize,
    /// Fact blocks, for nodes without a rail.
    pub blocks: Vec<FactBlock>,
}

impl Node {
    /// A key that differs per node: the inspector remounts (and drops its own
    /// open rows) when it changes.
    pub fn key(&self) -> String {
        match self {
            Node::Item(id) => format!("item:{id}"),
            Node::Event(id) => format!("event:{id}"),
            Node::Cluster(ids) => format!("cluster:{}", ids.join(",")),
            Node::Lane(id) => format!("lane:{id}"),
            Node::Pair(a, b) => format!("pair:{a}|{b}"),
        }
    }

    /// The content of this node over `events` and `tree`, values wrapped to
    /// `cols`. `None` when the node names nothing the events or tree hold.
    pub fn inspect(
        &self,
        events: &[Event],
        tree: Option<&SessionTree>,
        cols: usize,
    ) -> Option<Inspection> {
        match self {
            Node::Item(id) => item(self, id, Vec::new(), events, tree),
            Node::Event(id) => {
                let event = events.iter().find(|e| e.id == *id)?;
                match event.item_id.as_deref() {
                    Some(item_id) => item(self, item_id, vec![id.clone()], events, tree),
                    None => Some(loose(self, &[event], cols)),
                }
            }
            Node::Cluster(ids) => {
                let members: Vec<&Event> = ids
                    .iter()
                    .filter_map(|id| events.iter().find(|e| e.id == *id))
                    .collect();
                let first = members.first()?;
                match first.item_id.as_deref() {
                    Some(item_id) => {
                        let same: Vec<String> = members
                            .iter()
                            .filter(|e| e.item_id.as_deref() == Some(item_id))
                            .map(|e| e.id.clone())
                            .collect();
                        item(self, item_id, same, events, tree)
                    }
                    None => Some(loose(self, &members, cols)),
                }
            }
            Node::Lane(id) => {
                let lane = lane_of(events, tree?, id)?;
                Some(Inspection {
                    node: self.clone(),
                    header: None,
                    item: None,
                    tree: SessionTree::default(),
                    open: Vec::new(),
                    more: 0,
                    blocks: vec![FactBlock {
                        label: lane.label.clone(),
                        facts: lane_facts(&lane, cols),
                    }],
                })
            }
            // A pair's numbers are the table's, over its window: see `inspect_pair`.
            Node::Pair(..) => None,
        }
    }
}

/// `items` as lines of at most `cols` characters, the first `cap` only and a
/// `+N more` line for the rest; `none` for an empty list.
fn list_lines(items: &[String], cap: usize, cols: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut line = String::new();
    for item in items.iter().take(cap) {
        if !line.is_empty() && line.len() + 2 + item.len() > cols {
            line.push(',');
            lines.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push_str(", ");
        }
        line.push_str(item);
    }
    if !line.is_empty() {
        lines.push(line);
    }
    if items.len() > cap {
        lines.push(format!("+{} more", items.len() - cap));
    }
    if lines.is_empty() {
        lines.push("none".to_string());
    }
    lines
}

impl Node {
    /// The content of a [`Node::Pair`] over `pair`, the row the table drew:
    /// the agents, the shared items, notes each way, reviews, handoffs,
    /// branches and co-commits, lists capped at [`PAIR_LIST_CAP`]. `None` for
    /// any other node, or a pair other than `pair`.
    pub fn inspect_pair(&self, pair: &Pair, cols: usize) -> Option<Inspection> {
        let Node::Pair(a, b) = self else {
            return None;
        };
        if *a != pair.a || *b != pair.b {
            return None;
        }
        let one = |label: &'static str, value: String| Fact {
            label,
            lines: vec![value],
        };
        let handoffs: Vec<String> = pair
            .handoffs
            .iter()
            .map(|h| {
                format!(
                    "{} \u{2192} {} {} {}",
                    h.from,
                    h.to,
                    h.item_id,
                    format_iso8601(h.at)
                )
            })
            .collect();
        let facts = vec![
            one("agent a", pair.a.clone()),
            one("agent b", pair.b.clone()),
            one("shared", pair.shared_items.len().to_string()),
            Fact {
                label: "items",
                lines: list_lines(&pair.shared_items, PAIR_LIST_CAP, cols),
            },
            one(
                "notes",
                format!("{} \u{2192} {}", pair.notes_ab, pair.notes_ba),
            ),
            one("reviews", pair.reviews.to_string()),
            one("handoffs", pair.handoffs.len().to_string()),
            Fact {
                label: "moves",
                lines: list_lines(&handoffs, PAIR_LIST_CAP, cols),
            },
            one("branches", pair.branches.len().to_string()),
            Fact {
                label: "names",
                lines: list_lines(&pair.branches, PAIR_LIST_CAP, cols),
            },
            one("co-commits", pair.co_commits.to_string()),
        ];
        Some(Inspection {
            node: self.clone(),
            header: None,
            item: None,
            tree: SessionTree::default(),
            open: Vec::new(),
            more: 0,
            blocks: vec![FactBlock {
                label: format!("{} \u{2194} {}", pair.a, pair.b),
                facts,
            }],
        })
    }
}

/// The lane `id` names: one of the tree's, else one derived from the events
/// of the item whose id starts it.
fn lane_of(events: &[Event], tree: &SessionTree, id: &str) -> Option<Lane> {
    if let Some(lane) = tree.lanes.iter().find(|lane| lane.id == id) {
        return Some(lane.clone());
    }
    id.match_indices('/').find_map(|(at, _)| {
        let item = &id[..at];
        let run: Vec<&Event> = events
            .iter()
            .filter(|event| event.item_id.as_deref() == Some(item))
            .collect();
        derive_forks(item, &run)
            .0
            .into_iter()
            .find(|lane| lane.id == id)
    })
}

/// The session tree restricted to `id`'s lanes and elbows; an item the tree
/// holds no lane for gets the lanes its `run` of events derives. `None` is
/// the tree off.
fn pruned(tree: Option<&SessionTree>, id: &str, run: &[&Event]) -> SessionTree {
    let Some(tree) = tree else {
        return SessionTree::default();
    };
    let lanes: Vec<Lane> = tree
        .lanes
        .iter()
        .filter(|l| l.item_id == id)
        .cloned()
        .collect();
    if lanes.is_empty() {
        let (lanes, edges) = derive_forks(id, run);
        return SessionTree {
            agent: tree.agent.clone(),
            lanes,
            edges,
        };
    }
    SessionTree {
        agent: tree.agent.clone(),
        lanes,
        edges: tree
            .edges
            .iter()
            .filter(|e| e.item_id == id)
            .cloned()
            .collect(),
    }
}

/// Item content with `clicked` event ids to open.
fn item(
    node: &Node,
    id: &str,
    mut clicked: Vec<String>,
    events: &[Event],
    tree: Option<&SessionTree>,
) -> Option<Inspection> {
    let mut run: Vec<&Event> = events
        .iter()
        .filter(|e| e.item_id.as_deref() == Some(id))
        .collect();
    run.sort_by(|a, b| a.at.cmp(&b.at).then_with(|| a.id.cmp(&b.id)));
    let (first, last) = (run.first()?, run.last()?);
    let summary = summarize(&run, None)?;
    let mut agents: Vec<String> = run
        .iter()
        .filter_map(|e| e.agent.as_deref())
        .filter(|a| is_named(Some(a)))
        .map(str::to_owned)
        .collect();
    agents.sort();
    agents.dedup();
    let tree = pruned(tree, id, &run);
    let more = clicked.len().saturating_sub(CLUSTER_OPEN_CAP);
    clicked.truncate(CLUSTER_OPEN_CAP);
    Some(Inspection {
        node: node.clone(),
        header: Some(ItemHeader {
            id: id.to_owned(),
            title: summary.title,
            tone: tone_for_status(summary.status.as_deref()),
            status: summary.status,
            events: run.len(),
            first: format_iso8601(first.at),
            last: format_iso8601(last.at),
            agents,
            lanes: tree.lanes.len(),
        }),
        item: Some(ItemRef {
            id: id.to_owned(),
            status: None,
        }),
        tree,
        open: clicked,
        more,
        blocks: Vec::new(),
    })
}

/// Fact blocks of events no item owns, the first [`CLUSTER_OPEN_CAP`].
fn loose(node: &Node, events: &[&Event], cols: usize) -> Inspection {
    Inspection {
        node: node.clone(),
        header: None,
        item: None,
        tree: SessionTree::default(),
        open: Vec::new(),
        more: events.len().saturating_sub(CLUSTER_OPEN_CAP),
        blocks: events
            .iter()
            .take(CLUSTER_OPEN_CAP)
            .map(|e| FactBlock {
                label: e.id.clone(),
                facts: event_facts(e, cols),
            })
            .collect(),
    }
}
