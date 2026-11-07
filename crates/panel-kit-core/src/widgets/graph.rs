//! Renderer-neutral graph data and layout configuration.
//!
//! An external graph component receives a [`GraphModel`] (nodes plus edges
//! indexed into `nodes`) and a [`GraphLayout`] naming the solver and its
//! parameters. Core stores neither positions nor solver state: a backend
//! solves positions from the layout and paints them, so the same model
//! drives a WebGPU view and a renderer-neutral fallback.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::badge::Rgb;

/// One node of a graph model.
///
/// `sort` is the node's category in the knowledge base (an object sort, a
/// regime role); layouts group by it and painters read it for labels.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GraphNode {
    /// Stable node identifier, also the event payload.
    pub id: String,
    /// Human-readable label.
    pub label: String,
    /// Category this node belongs to.
    pub sort: String,
    /// Node colour as sRGB channels.
    pub color: Rgb,
    /// Drawn radius in renderer units.
    pub size: f32,
    /// Force-simulation mass.
    pub mass: f64,
    /// Force-simulation repulsion.
    pub repulsion: f64,
    /// Particle species in the physics base (`hydrophilic`, `hydrophobic`,
    /// `solvent`); empty when the regime pulls none back.
    #[serde(default)]
    pub species: String,
}

/// One directed edge between two [`GraphModel::nodes`] indices.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GraphEdge {
    /// Index into [`GraphModel::nodes`] of the source node.
    pub source: u32,
    /// Index into [`GraphModel::nodes`] of the target node.
    pub target: u32,
    /// Morphism kind this edge stands for.
    pub kind: String,
    /// Edge weight for layout solvers.
    pub weight: f64,
    /// Preferred edge length for layout solvers.
    pub rest_length: f64,
}

/// A complete graph body: nodes in declaration order plus edges between them.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GraphModel {
    /// Nodes in declaration order; edge indices address this vector.
    pub nodes: Vec<GraphNode>,
    /// Edges referencing [`GraphModel::nodes`] by index.
    pub edges: Vec<GraphEdge>,
}

/// Borrowed graph body for runtime providers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GraphView<'a> {
    /// Borrowed nodes in declaration order.
    pub nodes: &'a [GraphNode],
    /// Borrowed edges referencing [`GraphView::nodes`] by index.
    pub edges: &'a [GraphEdge],
}

/// Solver that positions a [`GraphModel`].
///
/// Each name maps onto one vendored layout solver; [`Force`](Self::Force)
/// is the GPU force simulation alone, with no pre-solved arrangement.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum LayoutAlgorithm {
    /// Force simulation only.
    Force,
    /// Amphiphile self-assembly: species-pair potentials over head / tail /
    /// solvent particles, bonds emergent.
    Amphiphile,
    /// Circular arrangement.
    Circle,
    /// Grid arrangement.
    Grid,
    /// Concentric rings by category.
    Concentric,
    /// Spectral embedding.
    Spectral,
    /// Layered directed graph layout.
    Dagre,
    /// Force-directed with constraint overlap removal.
    Fcose,
    /// Sphere packing.
    Sphere,
}

impl LayoutAlgorithm {
    /// Whether the arrangement is a running simulation rather than a static
    /// solve: surfaces without a simulator relax it with their own force solve.
    pub const fn is_simulation(self) -> bool {
        matches!(self, LayoutAlgorithm::Force | LayoutAlgorithm::Amphiphile)
    }
}

/// Layout request attached to an external graph panel.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GraphLayout {
    /// Solver to run.
    pub algorithm: LayoutAlgorithm,
    /// Node field the solver groups by; absent means no grouping.
    pub group_by: Option<String>,
    /// Solver parameters keyed by parameter name.
    pub params: BTreeMap<String, f64>,
}

impl Default for GraphLayout {
    fn default() -> Self {
        Self {
            algorithm: LayoutAlgorithm::Force,
            group_by: None,
            params: BTreeMap::new(),
        }
    }
}

impl GraphLayout {
    /// Whether this request groups nodes by `field`.
    pub fn groups_by(&self, field: &str) -> bool {
        self.group_by.as_deref() == Some(field)
    }
}

/// Side of the square plot space painters draw in; positions in the unit
/// square map onto it, and a surface scales it to the panel box (the shorter
/// side, centred).
pub const PLOT: f64 = 100.0;

/// Smallest drawn node radius in plot units, so a node stays clickable at any
/// model size.
pub const MIN_NODE_RADIUS: f64 = 2.5;

/// Largest drawn node radius in plot units, so a hub in a dense graph does not
/// swallow its panel.
pub const MAX_NODE_RADIUS: f64 = 9.0;

/// Gap in plot units between a selected node and its ring.
pub const SELECTION_RING_GAP: f64 = 2.0;

/// Edge stroke width in plot units.
pub const EDGE_STROKE: f64 = 1.0;

/// Selection-ring stroke width in plot units.
pub const RING_STROKE: f64 = 1.5;

/// The drawn radius, in plot units, of a node whose model size is `size`.
pub fn node_radius(size: f32) -> f64 {
    f64::from(size).clamp(MIN_NODE_RADIUS, MAX_NODE_RADIUS)
}

/// A unit-square position in plot units, or `None` when it is not finite.
pub fn plot_point(position: &[f32; 2]) -> Option<[f64; 2]> {
    position
        .iter()
        .all(|value| value.is_finite())
        .then(|| [f64::from(position[0]) * PLOT, f64::from(position[1]) * PLOT])
}

/// Every node of `nodes` that has a usable position, as `(index, plot point)`
/// in declaration order. A node without one is skipped rather than drawn at
/// the origin.
pub fn placed_nodes(nodes: &[GraphNode], positions: &[[f32; 2]]) -> Vec<(usize, [f64; 2])> {
    positions
        .iter()
        .take(nodes.len())
        .enumerate()
        .filter_map(|(index, position)| plot_point(position).map(|point| (index, point)))
        .collect()
}

/// Every edge whose two ends are both placed, as `(from, to)` plot points. An
/// edge needs both ends, so one reaching an unplaced node is dropped.
pub fn plotted_edges(edges: &[GraphEdge], placed: &[(usize, [f64; 2])]) -> Vec<([f64; 2], [f64; 2])> {
    let at = |index: u32| {
        placed
            .binary_search_by_key(&(index as usize), |&(node, _)| node)
            .ok()
            .map(|found| placed[found].1)
    };
    edges
        .iter()
        .filter_map(|edge| Some((at(edge.source)?, at(edge.target)?)))
        .collect()
}

/// The node `selected` names, if any.
pub fn selected_node<'a>(nodes: &'a [GraphNode], selected: Option<&str>) -> Option<(usize, &'a GraphNode)> {
    let id = selected?;
    nodes.iter().enumerate().find(|(_, node)| node.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: &str, sort: &str) -> GraphNode {
        GraphNode {
            id: id.into(),
            label: id.into(),
            sort: sort.into(),
            color: (94, 243, 140),
            size: 3.0,
            mass: 1.0,
            repulsion: 40.0,
            species: String::new(),
        }
    }

    #[test]
    fn graph_model_encodes_rgb_triples_and_indexed_edges() {
        let model = GraphModel {
            nodes: vec![node("a", "metric"), node("b", "policy")],
            edges: vec![GraphEdge {
                source: 0,
                target: 1,
                kind: "derived_from".into(),
                weight: 1.0,
                rest_length: 120.0,
            }],
        };
        let encoded = serde_json::to_value(&model).expect("graph model serializes");

        assert_eq!(encoded["nodes"][0]["color"], serde_json::json!([94, 243, 140]));
        assert_eq!(encoded["edges"][0]["source"], serde_json::json!(0));
        assert_eq!(encoded["edges"][0]["target"], serde_json::json!(1));
        assert_eq!(
            serde_json::from_value::<GraphModel>(encoded).expect("graph model decodes"),
            model
        );
    }

    #[test]
    fn graph_view_borrows_node_and_edge_slices() {
        let model = GraphModel { nodes: vec![node("a", "metric")], edges: Vec::new() };
        let view = GraphView {
            nodes: &model.nodes,
            edges: &model.edges,
        };

        assert_eq!(view.nodes.len(), 1);
        assert!(view.edges.is_empty());
    }

    #[test]
    fn layout_algorithm_names_are_snake_case() {
        let names: Vec<_> = [
            LayoutAlgorithm::Force,
            LayoutAlgorithm::Amphiphile,
            LayoutAlgorithm::Circle,
            LayoutAlgorithm::Grid,
            LayoutAlgorithm::Concentric,
            LayoutAlgorithm::Spectral,
            LayoutAlgorithm::Dagre,
            LayoutAlgorithm::Fcose,
            LayoutAlgorithm::Sphere,
        ]
        .iter()
        .map(|algorithm| serde_json::to_value(algorithm).expect("algorithm serializes"))
        .map(|value| value.as_str().expect("algorithm is a string").to_owned())
        .collect();

        assert_eq!(
            names,
            [
                "force",
                "amphiphile",
                "circle",
                "grid",
                "concentric",
                "spectral",
                "dagre",
                "fcose",
                "sphere",
            ]
        );
    }

    #[test]
    fn layout_groups_by_matches_only_the_named_field() {
        let mut layout = GraphLayout::default();
        assert!(!layout.groups_by("sort"));

        layout.group_by = Some("sort".into());
        layout.params.insert("rank_sep".into(), 2.0);

        assert!(layout.groups_by("sort"));
        assert!(!layout.groups_by("kind"));
        let encoded = serde_json::to_value(&layout).expect("layout serializes");
        assert_eq!(encoded["algorithm"], serde_json::json!("force"));
        assert_eq!(encoded["params"]["rank_sep"], serde_json::json!(2.0));
        assert_eq!(encoded["group_by"], serde_json::json!("sort"));
    }

    fn edge(source: u32, target: u32) -> GraphEdge {
        GraphEdge { source, target, kind: String::new(), weight: 1.0, rest_length: 1.0 }
    }

    #[test]
    fn node_radius_clamps_to_the_plot_bounds() {
        assert_eq!(node_radius(0.5), MIN_NODE_RADIUS);
        assert_eq!(node_radius(4.0), 4.0);
        assert_eq!(node_radius(50.0), MAX_NODE_RADIUS);
    }

    #[test]
    fn placed_nodes_scale_to_the_plot_and_skip_unusable_positions() {
        let nodes = [node("a", "x"), node("b", "x"), node("c", "x"), node("d", "x")];
        let positions = [[0.25, 0.5], [f32::NAN, 0.1], [1.0, 0.0]];

        assert_eq!(
            placed_nodes(&nodes, &positions),
            vec![(0, [25.0, 50.0]), (2, [100.0, 0.0])],
            "node 1 is not finite and node 3 has no position"
        );
    }

    #[test]
    fn plotted_edges_need_both_ends_placed() {
        let nodes = [node("a", "x"), node("b", "x"), node("c", "x")];
        let positions = [[0.0, 0.0], [f32::INFINITY, 0.0], [0.5, 0.5]];
        let placed = placed_nodes(&nodes, &positions);

        assert_eq!(
            plotted_edges(&[edge(0, 2), edge(0, 1), edge(2, 9)], &placed),
            vec![([0.0, 0.0], [50.0, 50.0])]
        );
    }

    #[test]
    fn selected_node_names_the_node_by_id() {
        let nodes = [node("a", "x"), node("b", "x")];

        assert_eq!(selected_node(&nodes, Some("b")).map(|(index, _)| index), Some(1));
        assert!(selected_node(&nodes, Some("zz")).is_none());
        assert!(selected_node(&nodes, None).is_none());
    }
}