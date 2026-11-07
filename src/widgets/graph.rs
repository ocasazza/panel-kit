//! SVG graph painter for the `graph` external component contract.
//!
//! The fallback path of a `graph` panel: the backend's renderer-neutral painter
//! over the same positions a WebGPU view would draw. It takes solved positions
//! rather than solving, so a browser without WebGPU shows the same arrangement
//! the solver produced instead of a second opinion.
//!
//! Colours come from the model; everything else — edges, the selection ring,
//! the hit target — comes from the panel-kit theme variables so a retheme
//! restyles the graph without a repaint.

use dioxus::prelude::*;
use panel_kit_core::widgets::graph::{
    node_radius, placed_nodes, plotted_edges, GraphView, PLOT, SELECTION_RING_GAP,
};

/// Styles for [`graph_svg`]. Hosts embed this with their own stylesheet; the
/// graph view's `CSS` sits alongside it for the WebGPU body.
pub const CSS: &str = "
.pk-graph { position:relative; width:100%; height:100%; min-height:4rem;
  background:var(--color-background); }
.pk-graph-svg { display:block; width:100%; height:100%; }
.pk-graph-edge { stroke:var(--color-border); stroke-width:1; }
.pk-graph-node { stroke:none; cursor:pointer; }
.pk-graph-node:focus-visible { outline:2px solid var(--color-highlight); outline-offset:2px; }
.pk-graph-ring { fill:none; stroke:var(--color-highlight); stroke-width:1.5; }
.pk-graph-label { fill:var(--color-text); font:0.62rem/1 var(--mono, monospace); }
.pk-graph-notice { color:var(--color-text-muted); font-size:0.78rem; padding:0.5rem; }
@media (prefers-reduced-motion: reduce) {
  .pk-graph-node, .pk-graph-ring { transition:none; }
}
";

/// Paint `view` over `positions` as inline SVG; clicking a node reports its id.
///
/// `positions` are normalised to the unit square (see the host's
/// `project_2d`), one per node, and a node without a position is skipped rather
/// than drawn at the origin. The `role="img"` summary names the graph, its size
/// and the selection, so the picture is not the only way to read it; each node
/// carries its label as a `<title>` tooltip.
pub fn graph_svg(
    view: GraphView<'_>,
    positions: &[[f32; 2]],
    selected: Option<&str>,
    on_select: EventHandler<String>,
) -> Element {
    let placed = placed_nodes(view.nodes, positions);
    let edges = plotted_edges(view.edges, &placed).into_iter().map(|(from, to)| {
        rsx! {
            line {
                class: "pk-graph-edge",
                x1: "{from[0]:.2}", y1: "{from[1]:.2}",
                x2: "{to[0]:.2}", y2: "{to[1]:.2}"
            }
        }
    });

    let nodes = placed.iter().map(|&(index, point)| {
        let node = &view.nodes[index];
        let radius = node_radius(node.size);
        let is_selected = selected == Some(node.id.as_str());
        let label = node.label.clone();
        let id = node.id.clone();
        let sort = node.sort.clone();
        let color = fill(node.color);
        let summary = if is_selected {
            format!("{label}, selected")
        } else {
            label.clone()
        };
        let select = on_select;
        rsx! {
            if is_selected {
                circle {
                    class: "pk-graph-ring",
                    cx: "{point[0]:.2}", cy: "{point[1]:.2}", r: "{(radius + SELECTION_RING_GAP):.2}"
                }
            }
            circle {
                class: "pk-graph-node",
                role: "button",
                tabindex: "0",
                "aria-label": "select {label}",
                "data-node": "{id}",
                cx: "{point[0]:.2}", cy: "{point[1]:.2}", r: "{radius:.2}",
                fill: "{color}",
                onclick: move |_| select.call(id.clone()),
                title { "{summary} — {sort}" }
            }
        }
    });

    let selected_label = selected
        .and_then(|id| view.nodes.iter().find(|node| node.id == id))
        .map(|node| format!(", selected {}", node.label))
        .unwrap_or_default();
    let summary = format!(
        "graph of {} nodes and {} morphisms{}",
        view.nodes.len(),
        view.edges.len(),
        if selected_label.is_empty() {
            String::new()
        } else {
            format!(", {selected_label}")
        }
    );

    rsx! {
        div { class: "pk-widget pk-graph", role: "img", aria_label: "{summary}",
            svg {
                class: "pk-graph-svg",
                view_box: "0 0 {PLOT} {PLOT}",
                preserve_aspect_ratio: "xMidYMid meet",
                {edges}
                {nodes}
            }
        }
    }
}

/// The honest body when the host lacks a capability the component requires and
/// the spec asks for a notice rather than a picture.
pub fn graph_notice(title: &str, detail: &str) -> Element {
    rsx! {
        div { class: "pk-widget pk-graph-notice", role: "status",
            strong { "{title}" }
            p { "{detail}" }
        }
    }
}

fn fill(color: panel_kit_core::badge::Rgb) -> String {
    let (r, g, b) = color;
    format!("rgb({r},{g},{b})")
}
