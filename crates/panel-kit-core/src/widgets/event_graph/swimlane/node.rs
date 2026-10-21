//! What one click opened, and what the inspector shows for it.
//!
//! A painter keeps a [`Node`] and asks [`Node::inspect`] for the content: an
//! item header and the item's events (with the clicked ones open), or plain
//! fact blocks for a fork lane and for events that belong to no item. Nothing
//! here measures or draws.

use serde::{Deserialize, Serialize};

use super::super::facts::{event_facts, lane_facts, Fact};
use super::super::model::{is_named, Event, ItemRef, SessionTree};
use super::super::query::{tone_for_status, Tone};
use super::super::time::format_iso8601;
use super::layout::summarize;

/// Events a cluster opens (and fact blocks an item-less cluster shows); the
/// rest are counted in [`Inspection::more`].
pub const CLUSTER_OPEN_CAP: usize = 12;

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
                let lane = tree?.lanes.iter().find(|l| l.id == *id)?;
                Some(Inspection {
                    node: self.clone(),
                    header: None,
                    item: None,
                    tree: SessionTree::default(),
                    open: Vec::new(),
                    more: 0,
                    blocks: vec![FactBlock {
                        label: lane.label.clone(),
                        facts: lane_facts(lane, cols),
                    }],
                })
            }
        }
    }
}

/// The session tree restricted to `id`'s lanes and elbows.
fn pruned(tree: Option<&SessionTree>, id: &str) -> SessionTree {
    let Some(tree) = tree else {
        return SessionTree::default();
    };
    SessionTree {
        agent: tree.agent.clone(),
        lanes: tree
            .lanes
            .iter()
            .filter(|l| l.item_id == id)
            .cloned()
            .collect(),
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
    let tree = pruned(tree, id);
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
