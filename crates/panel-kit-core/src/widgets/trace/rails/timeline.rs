//! Flat per-actor lane list: `[dot][caption]` nodes joined by a connector.
//!
//! Entry points: [`lanes_of_graph`] / [`lanes_of_flat`] derive [`TimelineLane`]s,
//! [`timeline_view`] resolves ids, current marker and "+N earlier" for a painter.

use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;

use serde::Deserialize;
use serde::Serialize;

use super::super::model::Micros;
use super::super::query::tone_for_kind;
use super::super::query::Tone;
use super::one_line;

/// Caption shown instead of lanes when there are none.
pub const EMPTY_TEXT: &str = "nothing recorded yet";

/// One node of a timeline graph or flat lane.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TlNode {
    /// Unique id.
    pub id: String,
    /// Node kind (`created`, `claim`, `thought`, ...).
    pub kind: String,
    /// Instant, microseconds; `None` for steps that carry order only.
    #[serde(default)]
    pub at: Option<Micros>,
    /// Caption; whitespace is collapsed when drawn.
    pub label: String,
    /// Tooltip text; the label is used when absent.
    #[serde(default)]
    pub detail: Option<String>,
    /// Id of the node this one derives from.
    #[serde(default)]
    pub parent: Option<String>,
    /// Dot colour; [`tone_for_kind`] of the kind when absent.
    #[serde(default)]
    pub status: Option<Tone>,
    /// Reconstructed from current state, not recorded: drawn hollow.
    #[serde(default)]
    pub inferred: bool,
    /// Free-form metadata of the emitter.
    #[serde(default)]
    pub meta: BTreeMap<String, String>,
    /// Author's position in the list it wrote; breaks ties of equal instants.
    #[serde(default)]
    pub order: Option<i64>,
}

impl TlNode {
    /// Dot colour of this node.
    pub fn tone(&self) -> Tone {
        self.status.unwrap_or_else(|| tone_for_kind(&self.kind))
    }
}

/// Derivation edge: `to` derives from `from`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TlEdge {
    /// Parent node id.
    pub from: String,
    /// Child node id.
    pub to: String,
}

/// Nodes and the edges derived from their `parent` ids.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TimelineGraph {
    /// Nodes in the order the builder wrote them.
    pub nodes: Vec<TlNode>,
    /// Edges, newest node first.
    pub edges: Vec<TlEdge>,
}

impl TimelineGraph {
    /// The graph of an item with no history.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Graph over `nodes` with one edge per node whose parent is a known node.
    pub fn from_nodes(nodes: Vec<TlNode>) -> Self {
        let known: HashSet<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
        let mut edges: Vec<TlEdge> = nodes
            .iter()
            .filter_map(|n| {
                let parent = n.parent.as_deref().filter(|p| known.contains(p))?;
                Some(TlEdge {
                    from: parent.to_owned(),
                    to: n.id.clone(),
                })
            })
            .collect();
        edges.reverse();
        TimelineGraph { nodes, edges }
    }

    /// Node with the highest `order`; the first one wins a tie.
    pub fn current_step(&self) -> Option<&TlNode> {
        let mut best: Option<&TlNode> = None;
        for node in self.nodes.iter().filter(|n| n.order.is_some()) {
            if best.is_none_or(|b| node.order > b.order) {
                best = Some(node);
            }
        }
        best
    }
}

/// Lane given directly by the caller.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FlatLane {
    /// Lane id.
    pub id: String,
    /// Lane label (actor).
    pub label: String,
    /// Nodes, in drawing order.
    pub nodes: Vec<TlNode>,
}

/// A lane ready to draw.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct TimelineLane {
    /// Lane id.
    pub id: String,
    /// Lane label.
    pub label: String,
    /// Nodes dropped from the front by the limit.
    pub hidden: usize,
    /// Shown nodes, labels collapsed to one line.
    pub nodes: Vec<TlNode>,
}

fn normalized(node: &TlNode) -> TlNode {
    TlNode {
        label: one_line(&node.label),
        ..node.clone()
    }
}

/// Lanes of caller-supplied nodes; with `limit`, only the last `limit` nodes show.
pub fn lanes_of_flat(lanes: &[FlatLane], limit: Option<usize>) -> Vec<TimelineLane> {
    lanes
        .iter()
        .map(|lane| {
            let hidden = limit.map_or(0, |l| lane.nodes.len().saturating_sub(l));
            TimelineLane {
                id: lane.id.clone(),
                label: lane.label.clone(),
                hidden,
                nodes: lane.nodes[hidden..].iter().map(normalized).collect(),
            }
        })
        .collect()
}

/// One lane per root node (no parent), holding the root and everything reachable
/// from it, sorted by `(at, order, id)`; absent `at` and `order` count as 0.
pub fn lanes_of_graph(graph: &TimelineGraph) -> Vec<TimelineLane> {
    let by_id: HashMap<&str, &TlNode> = graph.nodes.iter().map(|n| (n.id.as_str(), n)).collect();
    let mut children: HashMap<&str, Vec<&str>> = HashMap::new();
    for edge in &graph.edges {
        children
            .entry(edge.from.as_str())
            .or_default()
            .push(edge.to.as_str());
    }
    graph
        .nodes
        .iter()
        .filter(|root| root.parent.is_none())
        .map(|root| {
            let mut seen: HashSet<&str> = HashSet::new();
            let mut stack = vec![root.id.as_str()];
            let mut nodes: Vec<&TlNode> = Vec::new();
            while let Some(id) = stack.pop() {
                if !seen.insert(id) {
                    continue;
                }
                let Some(node) = by_id.get(id) else { continue };
                nodes.push(node);
                if let Some(next) = children.get(id) {
                    stack.extend(next.iter().rev());
                }
            }
            nodes.sort_by(|a, b| {
                (a.at.unwrap_or(0), a.order.unwrap_or(0), &a.id).cmp(&(
                    b.at.unwrap_or(0),
                    b.order.unwrap_or(0),
                    &b.id,
                ))
            });
            TimelineLane {
                id: root.id.clone(),
                label: root.label.clone(),
                hidden: 0,
                nodes: nodes.into_iter().map(normalized).collect(),
            }
        })
        .collect()
}

/// `+N earlier`.
pub fn earlier_text(hidden: usize) -> String {
    format!("+{hidden} earlier")
}

/// What the timeline draws from.
#[derive(Clone, Copy, Debug)]
pub enum TimelineInput<'a> {
    /// Nodes and edges; one lane per root.
    Graph(&'a TimelineGraph),
    /// Ready-made lanes.
    Lanes(&'a [FlatLane]),
}

/// Presentation options of one timeline.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TimelineOptions {
    /// Element id prefix.
    pub dom_id: String,
    /// Heading text and accessible label.
    pub title: String,
    /// Caption when there are no lanes.
    pub empty: String,
    /// Extra class, e.g. `timeline--dense`.
    pub class: Option<String>,
    /// Draw the heading.
    pub heading: bool,
    /// Nodes accept clicks.
    pub on_select: bool,
    /// Keep only the last N nodes of a flat lane.
    pub limit: Option<usize>,
    /// Id of the node drawn as current.
    pub current: Option<String>,
}

impl Default for TimelineOptions {
    fn default() -> Self {
        TimelineOptions {
            dom_id: "timeline".to_owned(),
            title: "timeline".to_owned(),
            empty: EMPTY_TEXT.to_owned(),
            class: None,
            heading: true,
            on_select: false,
            limit: None,
            current: None,
        }
    }
}

/// One node, resolved for painting.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct TimelineNodeView {
    /// Element id, `<dom_id>-node-<id>`.
    pub dom_id: String,
    /// Node id.
    pub id: String,
    /// Node kind.
    pub kind: String,
    /// Dot colour.
    pub tone: Tone,
    /// Node is the current step: cursor drawn.
    pub current: bool,
    /// Reconstructed, not recorded: dot drawn hollow.
    pub inferred: bool,
    /// Tooltip: detail, else label.
    pub title: String,
    /// One-line caption.
    pub caption: String,
    /// Node accepts clicks.
    pub selectable: bool,
}

/// One lane, resolved for painting.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct TimelineLaneView {
    /// Element id, `<dom_id>-lane-<id>`.
    pub dom_id: String,
    /// Lane id.
    pub id: String,
    /// Lane label.
    pub label: String,
    /// `+N earlier` when nodes were dropped.
    pub earlier: Option<String>,
    /// Element id of the earlier note.
    pub earlier_dom_id: Option<String>,
    /// Shown nodes.
    pub nodes: Vec<TimelineNodeView>,
}

/// The whole timeline.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct TimelineView {
    /// Element id prefix.
    pub dom_id: String,
    /// Accessible label.
    pub title: String,
    /// Class list: `timeline` plus any extra.
    pub class: String,
    /// Heading text, unless disabled.
    pub heading: Option<String>,
    /// Caption when there are no lanes.
    pub empty: Option<String>,
    /// Lanes, top to bottom.
    pub lanes: Vec<TimelineLaneView>,
}

/// Resolve `input` under `opts` for painting.
pub fn timeline_view(input: TimelineInput<'_>, opts: &TimelineOptions) -> TimelineView {
    let lanes = match input {
        TimelineInput::Graph(graph) => lanes_of_graph(graph),
        TimelineInput::Lanes(lanes) => lanes_of_flat(lanes, opts.limit),
    };
    let views: Vec<TimelineLaneView> = lanes
        .into_iter()
        .map(|lane| {
            let dom_id = format!("{}-lane-{}", opts.dom_id, lane.id);
            let hidden = lane.hidden > 0;
            TimelineLaneView {
                earlier: hidden.then(|| earlier_text(lane.hidden)),
                earlier_dom_id: hidden.then(|| format!("{dom_id}-earlier")),
                dom_id,
                nodes: lane
                    .nodes
                    .iter()
                    .map(|node| TimelineNodeView {
                        dom_id: format!("{}-node-{}", opts.dom_id, node.id),
                        id: node.id.clone(),
                        kind: node.kind.clone(),
                        tone: node.tone(),
                        current: opts.current.as_deref() == Some(node.id.as_str()),
                        inferred: node.inferred,
                        title: node.detail.clone().unwrap_or_else(|| node.label.clone()),
                        caption: node.label.clone(),
                        selectable: opts.on_select,
                    })
                    .collect(),
                id: lane.id,
                label: lane.label,
            }
        })
        .collect();
    TimelineView {
        dom_id: opts.dom_id.clone(),
        title: opts.title.clone(),
        class: match &opts.class {
            Some(extra) => format!("timeline {extra}"),
            None => "timeline".to_owned(),
        },
        heading: opts.heading.then(|| opts.title.clone()),
        empty: views.is_empty().then(|| opts.empty.clone()),
        lanes: views,
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;
    use rstest::rstest;

    use super::*;

    fn node(id: &str, kind: &str) -> TlNode {
        TlNode {
            id: id.to_owned(),
            kind: kind.to_owned(),
            label: id.to_owned(),
            status: Some(Tone::Hollow),
            ..TlNode::default()
        }
    }

    fn child(id: &str, parent: &str) -> TlNode {
        TlNode {
            parent: Some(parent.to_owned()),
            ..node(id, "note")
        }
    }

    #[test]
    fn a_lone_root_renders_as_a_node_in_a_plain_timeline() {
        let graph = TimelineGraph::from_nodes(vec![node("a", "note")]);
        let view = timeline_view(
            TimelineInput::Graph(&graph),
            &TimelineOptions {
                dom_id: "tl".to_owned(),
                ..TimelineOptions::default()
            },
        );
        assert_eq!(view.class, "timeline");
        assert_eq!(view.lanes.len(), 1);
        assert_eq!(view.lanes[0].nodes[0].dom_id, "tl-node-a");
        assert_eq!(view.empty, None);
    }

    #[test]
    fn no_lanes_shows_the_empty_text() {
        let view = timeline_view(
            TimelineInput::Graph(&TimelineGraph::empty()),
            &TimelineOptions::default(),
        );
        assert_eq!(view.empty.as_deref(), Some("nothing recorded yet"));
        assert!(view.lanes.is_empty());
        assert_eq!(view.heading.as_deref(), Some("timeline"));
    }

    #[test]
    fn heading_and_class_options() {
        let view = timeline_view(
            TimelineInput::Graph(&TimelineGraph::empty()),
            &TimelineOptions {
                heading: false,
                class: Some("timeline--dense".to_owned()),
                ..TimelineOptions::default()
            },
        );
        assert_eq!(view.heading, None);
        assert_eq!(view.class, "timeline timeline--dense");
    }

    #[rstest]
    #[case("item_created", Tone::Hollow)]
    #[case("item_released", Tone::Hollow)]
    #[case("item_status", Tone::Hollow)]
    #[case("note", Tone::Hollow)]
    #[case("heartbeat", Tone::Hollow)]
    #[case("run_model", Tone::Hollow)]
    #[case("commit", Tone::Hollow)]
    #[case("collab", Tone::Hollow)]
    #[case("item_claimed", Tone::Blue)]
    #[case("run_started", Tone::Blue)]
    #[case("run_tool", Tone::Blue)]
    #[case("run_thought", Tone::Blue)]
    #[case("run_reasoning", Tone::Blue)]
    #[case("branch_head", Tone::Blue)]
    #[case("pr_opened", Tone::Blue)]
    #[case("pr_reviewed", Tone::Blue)]
    #[case("path", Tone::Blue)]
    #[case("branch", Tone::Blue)]
    #[case("handoff", Tone::Yellow)]
    #[case("run_cancelled", Tone::Yellow)]
    #[case("run_failed", Tone::Red)]
    #[case("run_finished", Tone::Green)]
    #[case("pr_merged", Tone::Green)]
    #[case("anything_else", Tone::Hollow)]
    fn tone_for_table(#[case] kind: &str, #[case] want: Tone) {
        assert_eq!(tone_for_kind(kind), want);
    }

    #[test]
    fn status_overrides_the_kind_tone() {
        let mut n = node("a", "run_finished");
        assert_eq!(n.tone(), Tone::Hollow);
        n.status = None;
        assert_eq!(n.tone(), Tone::Green);
    }

    #[rstest]
    #[case(None, 5, 0)]
    #[case(Some(10), 5, 0)]
    #[case(Some(5), 5, 0)]
    #[case(Some(3), 5, 2)]
    #[case(Some(0), 5, 5)]
    fn flat_clip_keeps_the_last_nodes(
        #[case] limit: Option<usize>,
        #[case] total: usize,
        #[case] hidden: usize,
    ) {
        let nodes: Vec<TlNode> = (0..total).map(|i| node(&format!("n{i}"), "note")).collect();
        let lanes = lanes_of_flat(
            &[FlatLane {
                id: "l".into(),
                label: "omp".into(),
                nodes,
            }],
            limit,
        );
        assert_eq!(lanes[0].hidden, hidden);
        assert_eq!(lanes[0].nodes.len(), total - hidden);
        if hidden > 0 && hidden < total {
            assert_eq!(lanes[0].nodes[0].id, format!("n{hidden}"));
        }
    }

    #[test]
    fn earlier_note_is_named_after_its_lane() {
        let nodes: Vec<TlNode> = (0..4).map(|i| node(&format!("n{i}"), "note")).collect();
        let flat = [FlatLane {
            id: "l1".into(),
            label: "omp".into(),
            nodes,
        }];
        let view = timeline_view(
            TimelineInput::Lanes(&flat),
            &TimelineOptions {
                limit: Some(1),
                ..TimelineOptions::default()
            },
        );
        assert_eq!(view.lanes[0].earlier.as_deref(), Some("+3 earlier"));
        assert_eq!(
            view.lanes[0].earlier_dom_id.as_deref(),
            Some("timeline-lane-l1-earlier")
        );
        assert_eq!(view.lanes[0].dom_id, "timeline-lane-l1");
    }

    #[test]
    fn flat_labels_collapse_to_one_line() {
        let mut n = node("a", "note");
        n.label = "  two\n  lines\t here ".to_owned();
        let lanes = lanes_of_flat(
            &[FlatLane {
                id: "l".into(),
                label: "x".into(),
                nodes: vec![n],
            }],
            None,
        );
        assert_eq!(lanes[0].nodes[0].label, "two lines here");
    }

    #[test]
    fn graph_lane_sorts_by_at_then_order_then_id() {
        let mut root = node("r", "note");
        root.at = Some(0);
        let mut a = child("a", "r");
        a.at = Some(5);
        let mut b = child("b", "r");
        b.at = Some(5);
        b.order = Some(-1);
        let mut c = child("c", "r");
        c.at = Some(2);
        let graph = TimelineGraph::from_nodes(vec![root, a, b, c]);
        let lanes = lanes_of_graph(&graph);
        let ids: Vec<&str> = lanes[0].nodes.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(ids, ["r", "c", "b", "a"]);
    }

    #[test]
    fn steps_without_instants_sort_by_order_not_alphabet() {
        let mut nodes = vec![node("step-1", "thought")];
        for (id, order) in [("step-10", 9), ("step-2", 1)] {
            let mut n = child(id, "step-1");
            n.order = Some(order);
            nodes.push(n);
        }
        nodes[0].order = Some(0);
        let lanes = lanes_of_graph(&TimelineGraph::from_nodes(nodes));
        let ids: Vec<&str> = lanes[0].nodes.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(ids, ["step-1", "step-2", "step-10"]);
    }

    #[test]
    fn one_lane_per_root_in_node_order() {
        let graph = TimelineGraph::from_nodes(vec![
            node("r1", "note"),
            node("r2", "note"),
            child("c1", "r1"),
        ]);
        let lanes = lanes_of_graph(&graph);
        assert_eq!(
            lanes.iter().map(|l| l.id.as_str()).collect::<Vec<_>>(),
            ["r1", "r2"]
        );
        assert_eq!(lanes[0].nodes.len(), 2);
        assert_eq!(lanes[1].nodes.len(), 1);
    }

    #[test]
    fn cycles_and_dangling_edges_are_safe() {
        let mut graph = TimelineGraph::from_nodes(vec![node("r", "note"), child("a", "r")]);
        graph.edges.push(TlEdge {
            from: "a".into(),
            to: "r".into(),
        });
        graph.edges.push(TlEdge {
            from: "a".into(),
            to: "ghost".into(),
        });
        let lanes = lanes_of_graph(&graph);
        assert_eq!(lanes[0].nodes.len(), 2);
    }

    #[test]
    fn unattached_node_with_unknown_parent_is_in_no_lane() {
        let graph = TimelineGraph::from_nodes(vec![node("r", "note"), child("orphan", "ghost")]);
        assert!(graph.edges.is_empty());
        let lanes = lanes_of_graph(&graph);
        assert_eq!(lanes.len(), 1);
        assert_eq!(lanes[0].nodes.len(), 1);
    }

    #[test]
    fn current_node_is_marked_and_tooltip_prefers_detail() {
        let mut a = node("a", "note");
        a.detail = Some("full text".into());
        let b = node("b", "note");
        let flat = [FlatLane {
            id: "l".into(),
            label: "x".into(),
            nodes: vec![a, b],
        }];
        let view = timeline_view(
            TimelineInput::Lanes(&flat),
            &TimelineOptions {
                current: Some("b".into()),
                on_select: true,
                ..TimelineOptions::default()
            },
        );
        let nodes = &view.lanes[0].nodes;
        assert_eq!((nodes[0].current, nodes[1].current), (false, true));
        assert_eq!(
            (nodes[0].title.as_str(), nodes[1].title.as_str()),
            ("full text", "b")
        );
        assert!(nodes.iter().all(|n| n.selectable));
    }

    #[test]
    fn current_step_is_the_highest_order_zero_counts() {
        let mut a = node("a", "note");
        a.order = Some(0);
        let b = node("b", "note");
        let mut c = node("c", "note");
        c.order = Some(2);
        let mut d = node("d", "note");
        d.order = Some(1);
        let g = TimelineGraph {
            nodes: vec![a.clone(), b.clone(), c, d],
            edges: vec![],
        };
        assert_eq!(g.current_step().map(|n| n.id.as_str()), Some("c"));
        let only_zero = TimelineGraph {
            nodes: vec![a, b],
            edges: vec![],
        };
        assert_eq!(only_zero.current_step().map(|n| n.id.as_str()), Some("a"));
        assert_eq!(TimelineGraph::empty().current_step(), None);
        let none = TimelineGraph {
            nodes: vec![node("x", "note")],
            edges: vec![],
        };
        assert_eq!(none.current_step(), None);
    }

    proptest! {
        #[test]
        fn lane_nodes_are_sorted_and_cover_the_tree(parents in proptest::collection::vec(0usize..64, 0..40), ats in proptest::collection::vec(proptest::option::of(0i64..20), 41)) {
            let mut nodes = vec![TlNode { at: ats[0], ..node("n0", "note") }];
            for (i, p) in parents.iter().enumerate() {
                let parent = format!("n{}", p % (i + 1));
                nodes.push(TlNode { at: ats[i + 1], ..child(&format!("n{}", i + 1), &parent) });
            }
            let total = nodes.len();
            let lanes = lanes_of_graph(&TimelineGraph::from_nodes(nodes));
            prop_assert_eq!(lanes.len(), 1);
            prop_assert_eq!(lanes[0].nodes.len(), total);
            let keys: Vec<(i64, i64, String)> = lanes[0].nodes.iter().map(|n| (n.at.unwrap_or(0), n.order.unwrap_or(0), n.id.clone())).collect();
            prop_assert!(keys.windows(2).all(|w| w[0] <= w[1]));
        }

        #[test]
        fn flat_clip_never_loses_count(total in 0usize..30, limit in proptest::option::of(0usize..40)) {
            let nodes: Vec<TlNode> = (0..total).map(|i| node(&format!("n{i}"), "note")).collect();
            let lanes = lanes_of_flat(&[FlatLane { id: "l".into(), label: "x".into(), nodes }], limit);
            prop_assert_eq!(lanes[0].hidden + lanes[0].nodes.len(), total);
            if let Some(l) = limit { prop_assert!(lanes[0].nodes.len() <= l); }
        }
    }
}
