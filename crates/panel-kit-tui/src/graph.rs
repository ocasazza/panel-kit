//! Braille graph painter for the `graph` external component contract.
//!
//! The terminal fallback for a `graph` panel: a WebGPU canvas has no terminal
//! equivalent, so the same solved positions are drawn with ratatui's braille
//! canvas. The painter takes positions rather than solving, so the terminal and
//! the browser show one arrangement rather than two.
//!
//! Colours come from the model. Everything drawn outside the nodes — edges and
//! the selected node's label — comes from the resolved theme, so a retheme
//! restyles the graph without a repaint.

use std::collections::BTreeMap;
use panel_kit_core::badge::Rgb;
use panel_kit_core::widgets::graph::{GraphEdge, GraphView};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::symbols::Marker;
use ratatui::text::Line;
use ratatui::widgets::canvas::{Canvas, Line as CanvasLine, Points};

use crate::charts::rgb_color;
use crate::ResolvedTuiTheme;

/// Distance, in cell widths and heights, within which [`node_at`] claims a
/// node: one cell of slack keeps a single braille dot clickable.
const HIT_RADIUS: f64 = 1.0;

/// Paint `view` over `positions` in `area`; `selected` is highlighted and
/// labelled.
///
/// Positions are normalised to the unit square (see the host's `project_2d`),
/// one per node. A node without a usable position is skipped rather than stacked
/// at the origin, and the canvas clips to `area`, so a panel of any size —
/// including an empty one — stays inside its buffer.
pub fn graph(
    f: &mut Frame,
    area: Rect,
    t: &ResolvedTuiTheme,
    view: GraphView<'_>,
    positions: &[[f32; 2]],
    selected: Option<&str>,
) {
    if area.is_empty() || view.nodes.is_empty() {
        return;
    }

    // One point list per distinct node colour keeps the braille canvas' layer
    // count proportional to the palette rather than to the node count.
    let mut by_color: BTreeMap<Rgb, Vec<(f64, f64)>> = BTreeMap::new();
    let mut ring: Vec<(f64, f64)> = Vec::new();
    let mut label: Option<(f64, f64, String)> = None;
    for (index, node) in view.nodes.iter().enumerate() {
        let Some(point) = canvas_point(positions, index) else {
            continue;
        };
        if selected == Some(node.id.as_str()) {
            ring.push(point);
            label = Some((point.0, point.1, node.label.clone()));
        }
        by_color.entry(node.color).or_default().push(point);
    }

    let edges = edges(view.edges, positions, t.border);
    let ring_color = t.highlight;
    let label_style = Style::default().fg(t.text);

    f.render_widget(
        Canvas::default()
            .x_bounds([0.0, 1.0])
            .y_bounds([0.0, 1.0])
            .marker(Marker::Braille)
            .paint(move |ctx| {
                for edge in &edges {
                    ctx.draw(edge);
                }
                ctx.layer();
                for (color, points) in &by_color {
                    ctx.draw(&Points::new(points, rgb_color(*color)));
                }
                if !ring.is_empty() {
                    ctx.draw(&Points::new(&ring, ring_color));
                }
                if let Some((x, y, text)) = &label {
                    ctx.print(*x, *y, Line::from(format!(" {text}")).style(label_style));
                }
            }),
        area,
    );
}

/// The node under the cell at `(column, row)`, nearest node first.
///
/// Coordinates outside `area` clamp to its edges, so a click that lands on the
/// frame border resolves against the graph rather than returning nothing.
pub fn node_at(area: Rect, positions: &[[f32; 2]], at: (u16, u16)) -> Option<usize> {
    if area.is_empty() {
        return None;
    }
    let width = f64::from(area.width);
    let height = f64::from(area.height);
    let column = f64::from(at.0.clamp(area.left(), area.right().saturating_sub(1)))
        - f64::from(area.left());
    let row = f64::from(at.1.clamp(area.top(), area.bottom().saturating_sub(1)))
        - f64::from(area.top());

    let mut best: Option<(usize, f64)> = None;
    for (index, position) in positions.iter().enumerate() {
        if !position.iter().all(|value| value.is_finite()) {
            continue;
        }
        let dx = f64::from(position[0]) * width - column;
        let dy = f64::from(position[1]) * height - row;
        let distance = dx.hypot(dy);
        if distance <= HIT_RADIUS && best.is_none_or(|(_, near)| distance < near) {
            best = Some((index, distance));
        }
    }
    best.map(|(index, _)| index)
}

/// A node's canvas coordinate: the unit square's x across, y flipped because
/// canvas y grows upward while screen rows grow down.
fn canvas_point(positions: &[[f32; 2]], index: usize) -> Option<(f64, f64)> {
    let position = *positions.get(index)?;
    position
        .iter()
        .all(|value| value.is_finite())
        .then_some((f64::from(position[0]), 1.0 - f64::from(position[1])))
}

fn edges(
    edges: &[GraphEdge],
    positions: &[[f32; 2]],
    color: Color,
) -> Vec<CanvasLine> {
    edges
        .iter()
        .filter_map(|edge| {
            let from = canvas_point(positions, edge.source as usize)?;
            let to = canvas_point(positions, edge.target as usize)?;
            Some(CanvasLine::new(from.0, from.1, to.0, to.1, color))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use panel_kit_core::widgets::graph::{GraphEdge, GraphNode, GraphView};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn model() -> (Vec<GraphNode>, Vec<GraphEdge>) {
        let node = |id: &str| GraphNode {
            id: id.into(),
            label: id.into(),
            sort: "s".into(),
            color: (10, 20, 30),
            size: 2.0,
            mass: 1.0,
            repulsion: 1.0,
            species: String::new(),
        };
        (
            vec![node("a"), node("bb")],
            vec![GraphEdge {
                source: 0,
                target: 1,
                kind: "k".into(),
                weight: 1.0,
                rest_length: 1.0,
            }],
        )
    }

    fn render(area: Rect, positions: &[[f32; 2]], selected: Option<&str>) -> String {
        let (nodes, edges) = model();
        let backend = TestBackend::new(area.width, area.height);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        let theme = ResolvedTuiTheme::default();
        terminal
            .draw(|frame| {
                graph(
                    frame,
                    area,
                    &theme,
                    GraphView {
                        nodes: &nodes,
                        edges: &edges,
                    },
                    positions,
                    selected,
                )
            })
            .expect("draw");
        let buffer = terminal.backend().buffer().clone();
        buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>()
    }

    #[test]
    fn a_tiny_area_stays_inside_its_buffer() {
        let positions = [[0.0, 0.0], [1.0, 1.0]];
        for (width, height) in [(0, 0), (1, 1), (2, 1), (1, 3)] {
            let text = render(Rect::new(0, 0, width, height), &positions, Some("bb"));
            assert_eq!(
                text.chars().count(),
                usize::from(width) * usize::from(height),
                "{width}x{height} must paint exactly its own cells"
            );
        }
    }

    #[test]
    fn a_huge_area_stays_inside_its_buffer() {
        let positions = [[0.0, 0.0], [1.0, 1.0]];
        let text = render(Rect::new(0, 0, 400, 200), &positions, Some("bb"));
        assert_eq!(text.chars().count(), 400 * 200);
    }

    #[test]
    fn positions_outside_the_unit_square_are_clipped_not_panicking() {
        let wild = [[-9.0, 12.0], [1.0e9, -1.0e9], [f32::NAN, 0.0], [0.5, 0.5]];
        assert!(!render(Rect::new(0, 0, 20, 6), &wild, None).is_empty());
    }

    #[test]
    fn the_selection_labels_its_node() {
        let positions = [[0.0, 0.0], [0.5, 0.5]];
        let with = render(Rect::new(0, 0, 40, 8), &positions, Some("bb"));
        assert!(with.contains("bb"), "selected node must be labelled: {with:?}");
        let without = render(Rect::new(0, 0, 40, 8), &positions, None);
        assert!(!without.contains("bb"), "no label without a selection");
    }

    #[test]
    fn hit_testing_finds_the_node_under_the_cell() {
        let area = Rect::new(0, 0, 40, 20);
        let positions = [[0.25, 0.25], [0.75, 0.75]];
        assert_eq!(node_at(area, &positions, (10, 5)), Some(0));
        assert_eq!(node_at(area, &positions, (30, 15)), Some(1));
        assert_eq!(node_at(area, &positions, (20, 10)), None);
    }

    #[test]
    fn hit_testing_clamps_outside_the_area_and_survives_an_empty_one() {
        let area = Rect::new(10, 10, 40, 20);
        let positions = [[0.0, 0.0], [0.0, 1.0]];
        assert_eq!(node_at(area, &positions, (0, 0)), Some(0));
        assert_eq!(node_at(area, &positions, (0, 1000)), Some(1));
        assert_eq!(node_at(Rect::new(0, 0, 0, 0), &positions, (1, 1)), None);
        assert_eq!(node_at(area, &[], (11, 11)), None);
    }
}
