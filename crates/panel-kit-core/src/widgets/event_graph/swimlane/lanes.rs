//! LANES mode: events split into one flat lane per agent and, beside them,
//! the events no agent owns, grouped by where they were read from.
//!
//! The drawing (rows, `+N earlier`, one-line captions) belongs to the shared
//! timeline component; this is the data it is handed.

use std::collections::BTreeMap;

use serde::Serialize;

use super::super::model::{is_named, Event, EventKind};
use super::super::time;

/// One event as a lane node.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Node {
    /// The event id as a DOM-safe slug.
    pub id: String,
    /// What happened.
    pub kind: EventKind,
    /// One-line caption: a link word, then the first line of the summary.
    pub label: String,
    /// Tooltip: kind, agent, item, branch and time, joined with ` · `.
    pub detail: String,
}

/// A lane: a label and its nodes, in the order the events arrived.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Lane {
    /// DOM-safe id: the agent's slug, or `unattributed-{source}`.
    pub id: String,
    /// The agent's name, or the source's.
    pub label: String,
    /// The lane's nodes.
    pub nodes: Vec<Node>,
}

/// The lanes of a set of events.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Split {
    /// One lane per named agent, sorted by agent name.
    pub lanes: Vec<Lane>,
    /// One lane per source for events with no (or a blank) agent, sorted by
    /// source name.
    pub unattributed: Vec<Lane>,
}

impl Split {
    /// Events behind the unattributed toggle: events, not lanes.
    pub fn unattributed_count(&self) -> usize {
        self.unattributed.iter().map(|lane| lane.nodes.len()).sum()
    }

    /// Whether there is nothing to draw in either section.
    pub fn is_empty(&self) -> bool {
        self.lanes.is_empty() && self.unattributed.is_empty()
    }
}

/// Splits `events` into lanes. A blank agent is nobody: it never makes a lane
/// with an empty label. Nodes keep the order of `events`; pass them oldest
/// first.
pub fn split<'a>(events: impl IntoIterator<Item = &'a Event>) -> Split {
    let mut by_agent: BTreeMap<&str, Vec<&Event>> = BTreeMap::new();
    let mut by_source: BTreeMap<&str, Vec<&Event>> = BTreeMap::new();
    for event in events {
        match event.agent.as_deref() {
            Some(agent) if is_named(Some(agent)) => by_agent.entry(agent).or_default().push(event),
            _ => by_source
                .entry(event.source.as_str())
                .or_default()
                .push(event),
        }
    }

    Split {
        lanes: by_agent
            .into_iter()
            .map(|(agent, events)| Lane {
                id: slug(agent),
                label: agent.to_string(),
                nodes: nodes(&events),
            })
            .collect(),
        unattributed: by_source
            .into_iter()
            .map(|(source, events)| Lane {
                id: format!("unattributed-{source}"),
                label: source.to_string(),
                nodes: nodes(&events),
            })
            .collect(),
    }
}

fn nodes(events: &[&Event]) -> Vec<Node> {
    events.iter().map(|event| node(event)).collect()
}

/// A DOM-safe id: each run of characters outside `A-Za-z0-9_-` becomes one `-`.
pub fn slug(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_run = false;
    for c in text.chars() {
        if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
            out.push(c);
            in_run = false;
        } else if !in_run {
            out.push('-');
            in_run = true;
        }
    }
    out
}

/// One event as a node.
pub fn node(event: &Event) -> Node {
    Node {
        id: slug(&event.id),
        kind: event.kind,
        label: label(event),
        detail: detail(event),
    }
}

/// A branch head is named by its branch, since it has no subject; any other
/// event reads as a link word and the first line of its summary.
fn label(event: &Event) -> String {
    if let (EventKind::BranchHead, Some(branch)) = (event.kind, event.branch.as_deref()) {
        let sha: String = event
            .commit
            .as_deref()
            .unwrap_or("")
            .chars()
            .take(8)
            .collect();
        return format!("branch {branch} @ {sha}");
    }
    let link = link_label(event);
    let first_line = event
        .summary
        .as_deref()
        .map(|summary| summary.split('\n').next().unwrap_or("").trim());
    [link.as_deref(), first_line]
        .into_iter()
        .flatten()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Where a node points: an item, a commit in a checkout, a pull request.
/// Events that can be attributed to none of them get no link.
fn link_label(event: &Event) -> Option<String> {
    match (event.item_id.as_deref(), event.commit.as_deref(), event.pr) {
        (_, _, Some(pr)) => Some(format!("#{pr}")),
        (_, Some(commit), None) => Some(commit.chars().take(12).collect()),
        (Some(item), None, None) => Some(item.to_string()),
        (None, None, None) => None,
    }
}

fn detail(event: &Event) -> String {
    let at = time::format_iso8601(event.at);
    [
        Some(event.kind.as_str()),
        event.agent.as_deref(),
        event.item_id.as_deref(),
        event.branch.as_deref(),
        Some(at.as_str()),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" · ")
}
