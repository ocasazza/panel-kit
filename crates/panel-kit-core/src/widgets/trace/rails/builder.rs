//! Builds the [`TimelineGraph`] of an item from its ledger lines and run events.
//!
//! Pure: callers hand over already-decoded lines and events (timestamps as
//! microseconds); nothing here reads a clock.

use std::collections::BTreeMap;
use std::collections::HashSet;

use super::super::model::Micros;
use super::super::query::Tone;
use super::timeline::TimelineGraph;
use super::timeline::TlNode;

/// Snapshot of an item as a ledger line carries it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LedgerItem {
    /// Item id; a line without one yields no node.
    pub id: Option<String>,
    /// Item title.
    pub title: Option<String>,
    /// Item status (`open`, `in_progress`, `done`, ...).
    pub status: Option<String>,
    /// Current holder.
    pub assignee: Option<String>,
    /// Creator.
    pub created_by: Option<String>,
    /// Creation instant.
    pub created_at: Option<Micros>,
    /// Last update instant.
    pub updated_at: Option<Micros>,
    /// Instant of the current claim.
    pub claimed_at: Option<Micros>,
    /// Linked commit SHAs; may repeat.
    pub commits: Vec<String>,
}

/// One ledger line.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LedgerLine {
    /// `created`, `claimed`, `released`, `heartbeat`, or anything else.
    pub line_type: Option<String>,
    /// Top-level id, used when the line carries no item id.
    pub id: Option<String>,
    /// Item snapshot.
    pub item: Option<LedgerItem>,
}

impl LedgerLine {
    fn item_id(&self) -> Option<&str> {
        self.item
            .as_ref()
            .and_then(|item| item.id.as_deref())
            .or(self.id.as_deref())
    }
}

/// A node nested inside a run event's `graph`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NestedNode {
    /// Node id; `<run>:0:<index>` when absent.
    pub id: Option<String>,
    /// Node kind; `thought` when absent.
    pub kind: Option<String>,
    /// Instant.
    pub at: Option<Micros>,
    /// Caption.
    pub label: Option<String>,
    /// Tooltip text.
    pub detail: Option<String>,
    /// Parent node id.
    pub parent: Option<String>,
    /// Emitter metadata, kept on the node.
    pub meta: BTreeMap<String, String>,
    /// Position the author states; the index in the list when absent.
    pub order: Option<i64>,
}

/// One run event.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RunEventData {
    /// Event kind; `event` when absent.
    pub kind: Option<String>,
    /// The run's own counter; 0 when absent.
    pub sequence: Option<i64>,
    /// Event instant.
    pub timestamp: Option<Micros>,
    /// Caption; the kind when absent.
    pub label: Option<String>,
    /// Tooltip text.
    pub detail: Option<String>,
    /// Parent node id, used verbatim.
    pub parent: Option<String>,
    /// Event metadata without its `graph`.
    pub metadata: BTreeMap<String, String>,
    /// Nested subgraph; when present the event expands to these nodes.
    pub graph: Option<Vec<NestedNode>>,
}

/// A run event with the run it belongs to.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RunEntry {
    /// Run id.
    pub run_id: String,
    /// The event.
    pub event: RunEventData,
}

/// Node kinds a run emitter may name; any other kind becomes `event`.
const KNOWN_KINDS: &[&str] = &[
    "event",
    "created",
    "claim",
    "release",
    "heartbeat",
    "commit",
    "landing",
    "path",
    "branch",
    "collab",
    "note",
    "thought",
    "reasoning",
    "reasoning_graph",
    "tool_call",
    "tool_result",
    "model_request",
    "model_response",
    "tools_sent",
    "program",
    "prune",
    "review",
    "handoff",
    "run_started",
    "run_finished",
    "run_failed",
    "run_cancelled",
    "run_tool",
    "run_thought",
    "run_reasoning",
    "run_model",
    "item_created",
    "item_claimed",
    "item_released",
    "item_status",
    "branch_head",
    "pr_opened",
    "pr_reviewed",
    "pr_merged",
];

fn safe_kind(kind: &str) -> String {
    if KNOWN_KINDS.contains(&kind) {
        kind.to_owned()
    } else {
        "event".to_owned()
    }
}

fn status_of_kind(kind: &str) -> Tone {
    match kind {
        "run_finished" => Tone::Green,
        "run_failed" => Tone::Red,
        "run_cancelled" | "prune" | "review" => Tone::Yellow,
        _ => Tone::Hollow,
    }
}

fn last_id(nodes: &[TlNode]) -> Option<String> {
    nodes.last().map(|n| n.id.clone())
}

fn line_kind(line_type: &str) -> Option<&'static str> {
    match line_type {
        "created" => Some("created"),
        "claimed" => Some("claim"),
        "released" => Some("release"),
        "heartbeat" => Some("heartbeat"),
        _ => None,
    }
}

fn history_node(id: String, kind: &str, label: String, at: Option<Micros>) -> TlNode {
    TlNode {
        id,
        kind: kind.to_owned(),
        at,
        label,
        status: Some(Tone::Hollow),
        ..TlNode::default()
    }
}

/// The item's recorded lines, each chained to the previous node.
fn line_nodes(item_lines: &[&LedgerLine], item_id: &str) -> Vec<TlNode> {
    let mut nodes: Vec<TlNode> = Vec::new();
    for (index, line) in item_lines.iter().enumerate() {
        let item = line.item.as_ref();
        let Some(kind) = line.line_type.as_deref().and_then(line_kind) else {
            continue;
        };
        if item.and_then(|i| i.id.as_ref()).is_none() {
            continue;
        }
        let empty = LedgerItem::default();
        let item = item.unwrap_or(&empty);
        let label = match kind {
            "created" => "created".to_owned(),
            "claim" => format!("claimed by {}", item.assignee.as_deref().unwrap_or("?")),
            "release" => format!("released by {}", item.created_by.as_deref().unwrap_or("?")),
            _ => "lease renewed".to_owned(),
        };
        let mut node = history_node(
            format!("{item_id}:{index}:{kind}"),
            kind,
            label,
            item.created_at.or(item.updated_at),
        );
        node.detail = (kind == "created").then(|| item.title.clone()).flatten();
        node.parent = nodes.last().map(|n| n.id.clone());
        nodes.push(node);
    }
    nodes
}

/// Claim, commit and landing nodes the ledger cannot state, from the item's current fields.
fn inferred_nodes(recorded: &[TlNode], item: &LedgerItem) -> Vec<TlNode> {
    let item_id = item.id.as_deref().unwrap_or("");
    let holding = matches!(item.status.as_deref(), Some("claimed" | "in_progress"))
        && item.assignee.is_some();
    let mut out: Vec<TlNode> = Vec::new();
    if holding && !recorded.iter().any(|n| n.kind == "claim") {
        let mut claim = history_node(
            format!("{item_id}:inferred:claim"),
            "claim",
            format!("claimed by {}", item.assignee.as_deref().unwrap_or("")),
            item.claimed_at.or(item.updated_at),
        );
        claim.detail =
            Some("inferred from the item's current claim — no claim line was recorded".to_owned());
        claim.parent = last_id(recorded);
        claim.inferred = true;
        out.push(claim);
    }
    let mut tail = last_id(recorded).or_else(|| last_id(&out));

    let mut seen: HashSet<&str> = HashSet::new();
    for sha in item.commits.iter().filter(|sha| seen.insert(sha.as_str())) {
        let mut node = history_node(
            format!("{item_id}:commit:{sha}"),
            "commit",
            sha.clone(),
            None,
        );
        node.detail = Some(format!("commit {sha} (linked, not run-traced)"));
        node.parent = tail.take();
        node.inferred = true;
        tail = Some(node.id.clone());
        out.push(node);
    }

    let landing = match item.status.as_deref() {
        Some("done") => Some(("done", Tone::Green)),
        Some("review") => Some(("in review", Tone::Yellow)),
        _ => None,
    };
    if let Some((label, tone)) = landing {
        let mut node = history_node(
            format!("{item_id}:landing"),
            "landing",
            label.to_owned(),
            item.updated_at,
        );
        node.status = Some(tone);
        node.parent = tail;
        out.push(node);
    }
    out
}

/// The item's own history: the ledger lines that mention it, plus inferred nodes.
///
/// Lines of other items and of unmodelled types are skipped.
pub fn from_ledger(lines: &[LedgerLine], id: &str) -> TimelineGraph {
    let item_lines: Vec<&LedgerLine> = lines.iter().filter(|l| l.item_id() == Some(id)).collect();
    let mut nodes = line_nodes(&item_lines, id);
    let latest = item_lines
        .iter()
        .rev()
        .find_map(|l| l.item.as_ref())
        .cloned()
        .unwrap_or_default();
    let inferred = inferred_nodes(&nodes, &latest);
    nodes.extend(inferred);
    TimelineGraph::from_nodes(nodes)
}

fn flat_node(event: &RunEventData, run_id: &str) -> TlNode {
    let sequence = event.sequence.unwrap_or(0);
    let kind = event.kind.as_deref().unwrap_or("event");
    TlNode {
        id: format!("{run_id}:{sequence}:{kind}"),
        kind: safe_kind(kind),
        at: event.timestamp,
        label: event.label.clone().unwrap_or_else(|| kind.to_owned()),
        detail: event.detail.clone(),
        parent: event.parent.clone(),
        status: Some(status_of_kind(kind)),
        inferred: false,
        meta: event.metadata.clone(),
        order: Some(sequence),
    }
}

fn nested_nodes(nodes: &[NestedNode], run_id: &str) -> Vec<TlNode> {
    nodes
        .iter()
        .enumerate()
        .map(|(index, node)| {
            let kind = node.kind.as_deref().unwrap_or("thought");
            TlNode {
                id: node
                    .id
                    .clone()
                    .unwrap_or_else(|| format!("{run_id}:0:{index}")),
                kind: safe_kind(kind),
                at: node.at,
                label: node.label.clone().unwrap_or_default(),
                detail: node.detail.clone(),
                parent: node.parent.clone(),
                status: Some(status_of_kind(kind)),
                inferred: false,
                meta: node.meta.clone(),
                order: Some(node.order.unwrap_or(index as i64)),
            }
        })
        .collect()
}

/// One session's subgraph. With `chain`, a node that names no parent hangs off
/// the node before it; a named parent is kept.
pub fn from_run_events(entries: &[RunEntry], run_id: &str, chain: bool) -> TimelineGraph {
    let mut nodes: Vec<TlNode> = entries
        .iter()
        .flat_map(|entry| match &entry.event.graph {
            Some(graph) => nested_nodes(graph, run_id),
            None => vec![flat_node(&entry.event, run_id)],
        })
        .collect();
    if chain {
        let mut tail: Option<String> = None;
        for node in &mut nodes {
            if node.parent.is_none() {
                node.parent = tail.take();
            }
            tail = Some(node.id.clone());
        }
    }
    TimelineGraph::from_nodes(nodes)
}

/// One item's ledger history and its runs in one graph.
///
/// Runs are laid end to end in order of first appearance; each run's first
/// node hangs off the previous tail unless it names a parent itself.
pub fn for_item(lines: &[LedgerLine], run_entries: &[RunEntry], id: &str) -> TimelineGraph {
    let base = from_ledger(lines, id);
    let mut tail = last_id(&base.nodes);
    let mut nodes = base.nodes;

    let mut run_ids: Vec<&str> = Vec::new();
    for entry in run_entries {
        if !run_ids.contains(&entry.run_id.as_str()) {
            run_ids.push(entry.run_id.as_str());
        }
    }
    for run_id in run_ids {
        let entries: Vec<RunEntry> = run_entries
            .iter()
            .filter(|e| e.run_id == run_id)
            .cloned()
            .collect();
        let mut run_nodes = from_run_events(&entries, run_id, false).nodes;
        if let Some(first) = run_nodes.first_mut() {
            if first.parent.is_none() {
                first.parent = tail.clone();
            }
        }
        if let Some(last) = run_nodes.last() {
            tail = Some(last.id.clone());
        }
        nodes.extend(run_nodes);
    }
    TimelineGraph::from_nodes(nodes)
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::widgets::trace::time::from_utc;

    fn line(kind: &str, id: &str) -> LedgerLine {
        LedgerLine {
            line_type: Some(kind.to_owned()),
            id: None,
            item: Some(LedgerItem {
                id: Some(id.to_owned()),
                title: Some("t".to_owned()),
                assignee: Some("omp".to_owned()),
                ..LedgerItem::default()
            }),
        }
    }

    fn snap(kind: &str, item: LedgerItem) -> LedgerLine {
        LedgerLine {
            line_type: Some(kind.to_owned()),
            id: None,
            item: Some(item),
        }
    }

    fn item(id: &str) -> LedgerItem {
        LedgerItem {
            id: Some(id.to_owned()),
            title: Some("t".to_owned()),
            ..LedgerItem::default()
        }
    }

    fn entry(run: &str, kind: &str, seq: Option<i64>, parent: Option<&str>) -> RunEntry {
        RunEntry {
            run_id: run.to_owned(),
            event: RunEventData {
                kind: Some(kind.to_owned()),
                sequence: seq,
                parent: parent.map(str::to_owned),
                ..RunEventData::default()
            },
        }
    }

    fn nested(id: &str, kind: &str, parent: Option<&str>) -> NestedNode {
        NestedNode {
            id: Some(id.to_owned()),
            kind: Some(kind.to_owned()),
            parent: parent.map(str::to_owned),
            ..NestedNode::default()
        }
    }

    fn graph_entry(run: &str, nodes: Vec<NestedNode>) -> RunEntry {
        RunEntry {
            run_id: run.to_owned(),
            event: RunEventData {
                kind: Some("run_finished".to_owned()),
                graph: Some(nodes),
                ..RunEventData::default()
            },
        }
    }

    fn ids(g: &TimelineGraph) -> Vec<&str> {
        g.nodes.iter().map(|n| n.id.as_str()).collect()
    }

    fn kinds(g: &TimelineGraph) -> Vec<&str> {
        g.nodes.iter().map(|n| n.kind.as_str()).collect()
    }

    // -- from_ledger ----------------------------------------------------------

    #[test]
    fn ledger_no_lines_is_the_empty_graph() {
        assert_eq!(from_ledger(&[], "imp-1"), TimelineGraph::empty());
    }

    #[test]
    fn ledger_reads_created_claimed_and_released_as_nodes() {
        let lines = [
            line("created", "imp-1"),
            line("claimed", "imp-1"),
            line("released", "imp-1"),
        ];
        let g = from_ledger(&lines, "imp-1");
        assert_eq!(kinds(&g), ["created", "claim", "release"]);
        assert_eq!(g.nodes[1].label, "claimed by omp");
        assert_eq!(g.nodes[2].label, "released by ?");
        assert_eq!(g.nodes[0].detail.as_deref(), Some("t"));
        assert_eq!(g.nodes[1].detail, None);
    }

    #[test]
    fn ledger_skips_lines_that_name_another_item() {
        let g = from_ledger(
            &[line("created", "imp-1"), line("claimed", "imp-2")],
            "imp-1",
        );
        assert_eq!(g.nodes.len(), 1);
        assert!(g.nodes.iter().all(|n| n.id.starts_with("imp-1")));
        assert!(!g.nodes.iter().any(|n| n.id.contains("imp-2")));
    }

    #[test]
    fn ledger_skips_a_line_type_it_does_not_model() {
        let g = from_ledger(
            &[line("created", "imp-1"), line("some_future_type", "imp-1")],
            "imp-1",
        );
        assert_eq!(g.nodes.len(), 1);
    }

    #[test]
    fn ledger_skips_a_line_with_no_item_id() {
        let lines = [LedgerLine {
            line_type: Some("created".into()),
            id: None,
            item: Some(LedgerItem::default()),
        }];
        assert!(from_ledger(&lines, "imp-1").nodes.is_empty());
    }

    #[test]
    fn ledger_index_counts_skipped_lines() {
        let lines = [
            line("created", "imp-1"),
            line("updated", "imp-1"),
            line("claimed", "imp-1"),
        ];
        let g = from_ledger(&lines, "imp-1");
        assert_eq!(ids(&g), ["imp-1:0:created", "imp-1:2:claim"]);
        assert_eq!(g.nodes[1].parent.as_deref(), Some("imp-1:0:created"));
    }

    #[test]
    fn ledger_node_time_prefers_created_at() {
        let mut it = item("imp-1");
        it.created_at = Some(5);
        it.updated_at = Some(9);
        let mut other = item("imp-1");
        other.updated_at = Some(9);
        let g = from_ledger(&[snap("created", it), snap("claimed", other)], "imp-1");
        assert_eq!((g.nodes[0].at, g.nodes[1].at), (Some(5), Some(9)));
    }

    // -- inferred -------------------------------------------------------------

    fn legacy(id: &str) -> Vec<LedgerLine> {
        let created = LedgerItem {
            status: Some("open".into()),
            created_at: Some(from_utc(2026, 10, 3, 7, 54, 35)),
            updated_at: Some(from_utc(2026, 10, 3, 7, 54, 35)),
            ..item(id)
        };
        let claimed = LedgerItem {
            status: Some("in_progress".into()),
            assignee: Some("orchestrator".into()),
            claimed_at: Some(from_utc(2026, 10, 3, 17, 0, 0)),
            created_at: created.created_at,
            updated_at: Some(from_utc(2026, 10, 3, 17, 59, 47)),
            commits: vec!["621ae53b".into(), "df018694".into()],
            ..item(id)
        };
        vec![snap("created", created), snap("updated", claimed)]
    }

    #[test]
    fn inferred_claim_for_an_item_claimed_before_claim_lines() {
        let g = from_ledger(&legacy("imp-11"), "imp-11");
        let claims: Vec<&TlNode> = g.nodes.iter().filter(|n| n.kind == "claim").collect();
        assert_eq!(claims.len(), 1);
        assert_eq!(claims[0].label, "claimed by orchestrator");
        assert!(claims[0].inferred);
        assert_eq!(claims[0].at, Some(from_utc(2026, 10, 3, 17, 0, 0)));
        assert_eq!(claims[0].parent.as_deref(), Some("imp-11:0:created"));
    }

    #[test]
    fn a_real_claim_line_gets_no_inferred_duplicate() {
        let lines = [
            snap("created", item("imp-1")),
            snap(
                "claimed",
                LedgerItem {
                    assignee: Some("omp".into()),
                    status: Some("claimed".into()),
                    ..item("imp-1")
                },
            ),
        ];
        let g = from_ledger(&lines, "imp-1");
        assert_eq!(g.nodes.iter().filter(|n| n.kind == "claim").count(), 1);
        assert!(!g.nodes.iter().any(|n| n.inferred));
    }

    #[test]
    fn commit_links_become_one_node_each_deduped() {
        let lines = [
            snap("created", item("imp-29")),
            snap(
                "updated",
                LedgerItem {
                    status: Some("in_progress".into()),
                    assignee: Some("claude-imp-29".into()),
                    commits: vec!["abc1234".into(), "abc1234".into()],
                    ..item("imp-29")
                },
            ),
        ];
        let g = from_ledger(&lines, "imp-29");
        let commits: Vec<&TlNode> = g.nodes.iter().filter(|n| n.kind == "commit").collect();
        assert_eq!(commits.len(), 1);
        assert_eq!(commits[0].label, "abc1234");
        assert_eq!(
            commits[0].detail.as_deref(),
            Some("commit abc1234 (linked, not run-traced)")
        );
        assert_eq!(commits[0].at, None);
        assert!(commits[0].inferred);
    }

    #[test]
    fn a_done_item_gets_a_landing_node() {
        let lines = [
            snap("created", item("imp-20")),
            snap(
                "updated",
                LedgerItem {
                    status: Some("done".into()),
                    assignee: Some("x".into()),
                    commits: vec!["deadbee".into()],
                    updated_at: Some(77),
                    ..item("imp-20")
                },
            ),
        ];
        let g = from_ledger(&lines, "imp-20");
        let landing: Vec<&TlNode> = g.nodes.iter().filter(|n| n.kind == "landing").collect();
        assert_eq!(landing.len(), 1);
        assert_eq!(landing[0].status, Some(Tone::Green));
        assert_eq!(landing[0].label, "done");
        assert_eq!(landing[0].at, Some(77));
        assert!(!landing[0].inferred);
        assert_eq!(landing[0].parent.as_deref(), Some("imp-20:commit:deadbee"));
    }

    #[test]
    fn a_review_item_lands_amber() {
        let lines = [snap(
            "updated",
            LedgerItem {
                status: Some("review".into()),
                ..item("imp-2")
            },
        )];
        let g = from_ledger(&lines, "imp-2");
        assert_eq!(g.nodes.len(), 1);
        assert_eq!(
            (g.nodes[0].label.as_str(), g.nodes[0].status),
            ("in review", Some(Tone::Yellow))
        );
        assert_eq!(g.nodes[0].parent, None);
    }

    #[test]
    fn an_open_item_gets_neither_an_inferred_claim_nor_a_landing() {
        let lines = [snap(
            "created",
            LedgerItem {
                status: Some("open".into()),
                ..item("imp-30")
            },
        )];
        let g = from_ledger(&lines, "imp-30");
        assert!(g.nodes.iter().all(|n| n.kind == "created"));
    }

    #[test]
    fn every_history_node_has_its_own_id() {
        let g = from_ledger(&legacy("imp-11"), "imp-11");
        let mut seen = HashSet::new();
        assert!(g.nodes.iter().all(|n| seen.insert(n.id.clone())));
        assert!(g.nodes.len() >= 4);
    }

    #[test]
    fn the_graph_is_one_connected_chain_from_created() {
        let g = from_ledger(&legacy("imp-11"), "imp-11");
        let roots: Vec<&TlNode> = g.nodes.iter().filter(|n| n.parent.is_none()).collect();
        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].kind, "created");
        let known: HashSet<&str> = g.nodes.iter().map(|n| n.id.as_str()).collect();
        assert!(g
            .nodes
            .iter()
            .filter_map(|n| n.parent.as_deref())
            .all(|p| known.contains(p)));
        assert_eq!(g.edges.len(), g.nodes.len() - 1);
    }

    #[test]
    fn inferred_claim_roots_the_chain_when_nothing_was_recorded() {
        let lines = [snap(
            "updated",
            LedgerItem {
                status: Some("in_progress".into()),
                assignee: Some("a".into()),
                commits: vec!["c1".into()],
                ..item("imp-5")
            },
        )];
        let g = from_ledger(&lines, "imp-5");
        assert_eq!(ids(&g), ["imp-5:inferred:claim", "imp-5:commit:c1"]);
        assert_eq!(g.nodes[0].parent, None);
        assert_eq!(g.nodes[1].parent.as_deref(), Some("imp-5:inferred:claim"));
    }

    #[test]
    fn a_line_without_item_id_matches_by_its_own_id() {
        let l = LedgerLine {
            line_type: Some("created".into()),
            id: Some("imp-1".into()),
            item: Some(LedgerItem::default()),
        };
        // matched by line id, but a node still needs the item's id
        assert!(from_ledger(&[l], "imp-1").nodes.is_empty());
    }

    // -- run events -----------------------------------------------------------

    #[test]
    fn flat_one_node_per_event_linked_by_parent() {
        let entries = [
            RunEntry {
                run_id: "r1".into(),
                event: RunEventData {
                    label: Some("root".into()),
                    ..entry("r1", "thought", Some(1), None).event
                },
            },
            entry("r1", "branch", Some(2), Some("r1:1:thought")),
        ];
        let g = from_run_events(&entries, "r1", false);
        assert_eq!(g.nodes.len(), 2);
        assert_eq!(g.edges.len(), 1);
        assert_eq!(g.edges[0].from, "r1:1:thought");
        assert_eq!(g.nodes[0].label, "root");
        assert_eq!(g.nodes[1].label, "branch");
        assert_eq!(g.nodes[1].order, Some(2));
    }

    #[test]
    fn flat_unknown_parent_produces_no_edge() {
        let g = from_run_events(
            &[entry("r1", "thought", None, Some("does-not-exist"))],
            "r1",
            false,
        );
        assert!(g.edges.is_empty());
        assert_eq!(g.nodes[0].id, "r1:0:thought");
    }

    #[test]
    fn flat_unknown_kind_is_kept_as_event() {
        let g = from_run_events(
            &[entry("r1", "a_kind_imp_does_not_know", None, None)],
            "r1",
            false,
        );
        assert_eq!(g.nodes.len(), 1);
        assert_eq!(g.nodes[0].kind, "event");
        assert_eq!(g.nodes[0].id, "r1:0:a_kind_imp_does_not_know");
    }

    #[test]
    fn nested_graph_becomes_the_same_shape() {
        let e = graph_entry(
            "r1",
            vec![
                nested("t1", "thought", None),
                nested("b1", "branch", Some("t1")),
            ],
        );
        let g = from_run_events(&[e], "r1", false);
        assert_eq!(ids(&g), ["t1", "b1"]);
        assert_eq!(kinds(&g), ["thought", "branch"]);
        assert_eq!(
            (g.edges[0].from.as_str(), g.edges[0].to.as_str()),
            ("t1", "b1")
        );
    }

    #[test]
    fn nested_status_maps_to_the_token_vocabulary() {
        let e = graph_entry(
            "r1",
            vec![
                nested("ok", "run_finished", None),
                nested("bad", "run_failed", None),
                nested("human", "review", None),
            ],
        );
        let g = from_run_events(&[e], "r1", false);
        let statuses: Vec<Option<Tone>> = g.nodes.iter().map(|n| n.status).collect();
        assert_eq!(
            statuses,
            [Some(Tone::Green), Some(Tone::Red), Some(Tone::Yellow)]
        );
    }

    #[test]
    fn nested_meta_is_not_hoisted() {
        let mut n = nested("t1", "thought", None);
        n.meta.insert("program".into(), "print(1)".into());
        let mut e = graph_entry("r1", vec![n]);
        e.event.metadata.insert("outer".into(), "x".into());
        let g = from_run_events(&[e], "r1", false);
        assert_eq!(g.nodes.len(), 1);
        assert_eq!(
            g.nodes[0].meta.get("program").map(String::as_str),
            Some("print(1)")
        );
        assert_eq!(g.nodes[0].meta.len(), 1);
    }

    #[test]
    fn nested_order_defaults_to_position_and_id_to_run() {
        let a = NestedNode::default();
        let b = NestedNode {
            order: Some(7),
            ..NestedNode::default()
        };
        let g = from_run_events(&[graph_entry("r1", vec![a, b])], "r1", false);
        assert_eq!(
            g.nodes.iter().map(|n| n.order).collect::<Vec<_>>(),
            [Some(0), Some(7)]
        );
        assert_eq!(ids(&g), ["r1:0:0", "r1:0:1"]);
        assert_eq!(g.nodes[0].kind, "thought");
    }

    #[test]
    fn both_shapes_coexist_in_one_session() {
        let entries = [
            entry("r1", "run_started", None, None),
            graph_entry("r1", vec![nested("t1", "thought", None)]),
        ];
        let g = from_run_events(&entries, "r1", false);
        assert_eq!(g.nodes.len(), 2);
        assert!(g.nodes.iter().all(|n| !n.id.is_empty()));
    }

    #[test]
    fn chain_fills_only_a_missing_parent() {
        let entries = [
            entry("r1", "thought", Some(1), None),
            entry("r1", "tool_call", Some(2), None),
            entry("r1", "tool_result", Some(3), Some("r1:1:thought")),
        ];
        let g = from_run_events(&entries, "r1", true);
        assert_eq!(g.nodes[0].parent, None);
        assert_eq!(g.nodes[1].parent.as_deref(), Some("r1:1:thought"));
        assert_eq!(g.nodes[2].parent.as_deref(), Some("r1:1:thought"));
        assert_eq!(g.edges.len(), 2);
    }

    #[test]
    fn current_step_follows_the_highest_sequence() {
        let entries = [
            entry("run-1", "tool_call", Some(1), None),
            entry("run-1", "tool_call", Some(2), None),
        ];
        let g = from_run_events(&entries, "run-1", true);
        assert_eq!(
            g.current_step().map(|n| n.id.as_str()),
            Some("run-1:2:tool_call")
        );
    }

    // -- for_item -------------------------------------------------------------

    #[test]
    fn for_item_without_runs_draws_exactly_the_ledger_graph() {
        let lines = [line("created", "imp-1")];
        assert_eq!(for_item(&lines, &[], "imp-1"), from_ledger(&lines, "imp-1"));
    }

    #[test]
    fn for_item_run_events_are_nodes_of_the_items_graph() {
        let g = for_item(
            &[line("created", "imp-1")],
            &[entry("r1", "run_finished", Some(1), None)],
            "imp-1",
        );
        let k = kinds(&g);
        assert!(k.contains(&"created") && k.contains(&"run_finished"));
    }

    #[test]
    fn for_item_first_run_node_hangs_off_the_last_recorded_line() {
        let g = for_item(
            &[line("created", "imp-1")],
            &[entry("r1", "run_finished", Some(1), None)],
            "imp-1",
        );
        assert_eq!(g.edges.len(), 1);
        assert_eq!(g.edges[0].from, "imp-1:0:created");
        assert_eq!(g.edges[0].to, "r1:1:run_finished");
    }

    #[test]
    fn for_item_two_runs_chain_end_to_end() {
        let entries = [
            entry("r1", "run_finished", Some(1), None),
            entry("r2", "run_failed", Some(1), None),
        ];
        let g = for_item(&[line("created", "imp-1")], &entries, "imp-1");
        assert!(ids(&g).contains(&"r1:1:run_finished"));
        let second = g.nodes.iter().find(|n| n.id == "r2:1:run_failed");
        assert_eq!(
            second.and_then(|n| n.parent.as_deref()),
            Some("r1:1:run_finished")
        );
    }

    #[test]
    fn for_item_interleaved_runs_group_by_first_appearance() {
        let entries = [
            entry("r1", "thought", Some(1), None),
            entry("r2", "thought", Some(1), None),
            entry("r1", "branch", Some(2), Some("r1:1:thought")),
        ];
        let g = for_item(&[], &entries, "imp-1");
        assert_eq!(ids(&g), ["r1:1:thought", "r1:2:branch", "r2:1:thought"]);
        assert_eq!(g.nodes[2].parent.as_deref(), Some("r1:2:branch"));
    }

    #[test]
    fn for_item_an_empty_run_leaves_the_tail_in_place() {
        let empty = RunEntry {
            run_id: "r2".into(),
            event: RunEventData {
                graph: Some(vec![]),
                ..RunEventData::default()
            },
        };
        let entries = [
            entry("r1", "thought", Some(1), None),
            empty,
            entry("r3", "thought", Some(1), None),
        ];
        let g = for_item(&[], &entries, "imp-1");
        let r3 = g.nodes.iter().find(|n| n.id == "r3:1:thought");
        assert_eq!(r3.and_then(|n| n.parent.as_deref()), Some("r1:1:thought"));
    }

    #[test]
    fn for_item_an_emitter_chosen_parent_is_not_overridden() {
        let entries = [
            entry("r1", "branch", Some(1), None),
            entry("r1", "tool_result", Some(2), Some("r1:1:branch")),
        ];
        let g = for_item(&[line("created", "imp-1")], &entries, "imp-1");
        let branch = g.nodes.iter().find(|n| n.id == "r1:1:branch");
        assert_eq!(
            branch.and_then(|n| n.parent.as_deref()),
            Some("imp-1:0:created")
        );
        let result = g.nodes.iter().find(|n| n.id == "r1:2:tool_result");
        assert_eq!(
            result.and_then(|n| n.parent.as_deref()),
            Some("r1:1:branch")
        );
    }

    #[test]
    fn for_item_a_nested_graph_is_its_own_subgraph() {
        let e = graph_entry(
            "r1",
            vec![
                nested("n1", "thought", None),
                nested("n2", "branch", Some("n1")),
            ],
        );
        let g = for_item(&[line("created", "imp-1")], &[e], "imp-1");
        assert!(ids(&g).contains(&"n1") && ids(&g).contains(&"n2"));
        assert!(g.edges.iter().any(|e| e.from == "n1" && e.to == "n2"));
    }

    proptest! {
        #[test]
        fn edges_never_dangle_and_every_chained_non_root_has_one(kinds in proptest::collection::vec(0usize..4, 0..30)) {
            let names = ["thought", "branch", "tool_call", "weird_kind"];
            let entries: Vec<RunEntry> = kinds.iter().enumerate().map(|(i, k)| entry("r", names[*k], Some(i as i64), None)).collect();
            let g = from_run_events(&entries, "r", true);
            let known: HashSet<&str> = g.nodes.iter().map(|n| n.id.as_str()).collect();
            prop_assert!(g.edges.iter().all(|e| known.contains(e.from.as_str()) && known.contains(e.to.as_str())));
            prop_assert_eq!(g.edges.len(), g.nodes.len().saturating_sub(1));
        }
    }
}
