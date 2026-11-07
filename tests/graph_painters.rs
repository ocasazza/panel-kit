//! The web fallback painter's structure: edges, coloured nodes, the selection
//! ring, node tooltips, and a summary that names the selection.

use dioxus::prelude::*;
use panel_kit::widgets::graph::{graph_notice, graph_svg};
use panel_kit_core::widgets::graph::{
    GraphEdge, GraphModel, GraphNode, GraphView,
};

fn node(id: &str, color: (u8, u8, u8)) -> GraphNode {
    GraphNode {
        id: id.into(),
        label: id.to_uppercase(),
        sort: "metric".into(),
        color,
        size: 2.0,
        mass: 1.0,
        repulsion: 1.0,
        species: String::new(),
    }
}

fn model() -> GraphModel {
    GraphModel {
        nodes: vec![node("a", (10, 20, 30)), node("b", (40, 50, 60))],
        edges: vec![GraphEdge {
            source: 0,
            target: 1,
            kind: "derived_from".into(),
            weight: 1.0,
            rest_length: 60.0,
        }],
    }
}

#[component]
fn GraphProbe(selected: Option<String>) -> Element {
    let model = model();
    graph_svg(
        GraphView {
            nodes: &model.nodes,
            edges: &model.edges,
        },
        &[[0.0, 0.0], [1.0, 1.0]],
        selected.as_deref(),
        EventHandler::new(|_: String| {}),
    )
}

#[test]
fn the_svg_painter_draws_edges_nodes_and_labels() {
    let html = dioxus_ssr::render_element(rsx! { GraphProbe {} });

    assert!(html.contains("<svg"), "{html}");
    assert!(html.contains("viewBox=\"0 0 100 100\""), "{html}");
    assert!(html.contains("pk-graph-edge"), "{html}");
    assert_eq!(html.matches("pk-graph-node").count(), 2, "{html}");
    assert!(html.contains("fill=\"rgb(10,20,30)\""), "{html}");
    assert!(html.contains("data-node=\"a\""), "{html}");
    assert!(html.contains("A — metric"), "node tooltip: {html}");
    assert!(html.contains("graph of 2 nodes and 1 morphisms"), "{html}");
    assert!(!html.contains("pk-graph-ring"), "no ring without a selection");
}

#[test]
fn the_selection_gets_a_ring_and_names_itself_in_the_summary() {
    let html = dioxus_ssr::render_element(rsx! { GraphProbe { selected: "b" } });

    assert_eq!(html.matches("pk-graph-ring").count(), 1, "{html}");
    assert!(html.contains("B, selected"), "{html}");
    assert!(html.contains(", selected B"), "{html}");
}

#[test]
fn a_node_without_a_position_is_not_drawn_at_the_origin() {
    #[component]
    fn Short() -> Element {
        let model = model();
        graph_svg(
            GraphView {
                nodes: &model.nodes,
                edges: &model.edges,
            },
            &[[0.5, 0.5]],
            None,
            EventHandler::new(|_: String| {}),
        )
    }

    let html = dioxus_ssr::render_element(rsx! { Short {} });

    assert_eq!(html.matches("pk-graph-node").count(), 1, "{html}");
    assert!(!html.contains("pk-graph-edge"), "an edge needs both ends: {html}");
}

#[test]
fn the_notice_fallback_states_the_absence_instead_of_a_picture() {
    let html = dioxus_ssr::render_element(rsx! {
        {graph_notice("Graph renderer unavailable", "no WebGPU here")}
    });

    assert!(html.contains("Graph renderer unavailable"), "{html}");
    assert!(html.contains("no WebGPU here"), "{html}");
    assert!(!html.contains("<svg"), "{html}");
}
