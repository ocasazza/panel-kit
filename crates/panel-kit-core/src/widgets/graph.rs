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
}