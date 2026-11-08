//! Bevy UI port of the web `graph` painter (`src/widgets/graph.rs`).
//!
//! The web paints an SVG whose 100-unit square `viewBox` is fitted to the
//! panel box with `xMidYMid meet`. Here the same square is a UI node sized to
//! the shorter side of the host box and centred, so every node, ring and label
//! is positioned in percent of the plot, and the edges are rasterised into one
//! image at the plot's physical size. Geometry (radius clamp, plot mapping,
//! which nodes and edges are placed) comes from
//! `panel_kit_core::widgets::graph`, shared with the web and terminal painters.
//!
//! Theme roles: edges `border`, the selection ring `highlight`, the selected
//! label `text`, the host fill `background`; node fills are the model's own
//! colours. Add [`GraphPlugin`] to the app to size the plot and paint edges;
//! without it the host stays empty.

use bevy::prelude::*;
use panel_kit_core::widgets::graph::{
    node_radius, placed_nodes, plotted_edges, selected_node, GraphLayout, GraphModel, GraphView,
    EDGE_STROKE, PLOT, RING_STROKE, SELECTION_RING_GAP,
};
use panel_kit_graph_layout::solve_2d;

use super::{raster, styled_text, TextStyle};
use crate::theme::BevyTheme;

/// `.pk-graph` `min-height`, in rem.
const MIN_HEIGHT_REM: f32 = 4.0;

/// Gap between a selected node's ring and its label, in plot units.
const LABEL_GAP: f32 = 1.0;

/// The box that fits the plot: `.pk-graph`.
#[derive(Component)]
struct GraphHost;

/// The square plot space and the edges rasterised into it.
#[derive(Component)]
struct GraphPlot {
    edges: Vec<([f64; 2], [f64; 2])>,
    color: Color,
    painted: Option<(UVec2, f32)>,
}

/// The absolutely positioned child of a [`GraphPlot`] that shows the edge image.
#[derive(Component)]
struct GraphEdgeLayer;

/// A drawn node: `index` addresses the model's node list.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct GraphNodeDot {
    /// Index into the painted model's nodes.
    pub index: usize,
}

/// The ring around the selected node (`.pk-graph-ring`).
#[derive(Component)]
pub struct GraphSelectionRing;

/// The selected node's label.
#[derive(Component)]
pub struct GraphNodeLabel;

/// Which graph and node a dot selects when pressed.
#[derive(Component, Clone, Debug)]
struct GraphNodeTarget {
    graph: Entity,
    id: String,
}

/// A node was pressed. The host owns the selection, as on the web and in the
/// terminal: repaint the graph with `node` as `selected`. Pressing empty plot
/// space emits nothing.
#[derive(Event, Clone, Debug, PartialEq, Eq)]
pub struct GraphSelectEvent {
    /// The graph entity [`spawn`] returned.
    pub graph: Entity,
    /// The pressed node's id.
    pub node: String,
}

/// Solve `model` under `layout` and paint it; the path a host takes when it
/// owns no positions of its own.
pub fn spawn_solved(
    parent: &mut ChildSpawnerCommands,
    theme: &BevyTheme,
    model: &GraphModel,
    layout: &GraphLayout,
    selected: Option<&str>,
) -> Option<Entity> {
    let positions = solve_2d(model, layout);
    spawn(
        parent,
        theme,
        GraphView {
            nodes: &model.nodes,
            edges: &model.edges,
        },
        &positions,
        selected,
    )
}

/// Paint `view` over `positions` (unit square, one per node); `selected` gets
/// a ring and its label. Returns the graph entity [`GraphSelectEvent`] names,
/// or `None` when there is nothing to draw.
///
/// A node without a usable position is skipped, and an edge needs both ends
/// placed. A view with no nodes paints the [`notice`] instead of an empty plot.
pub fn spawn(
    parent: &mut ChildSpawnerCommands,
    theme: &BevyTheme,
    view: GraphView<'_>,
    positions: &[[f32; 2]],
    selected: Option<&str>,
) -> Option<Entity> {
    if view.nodes.is_empty() {
        notice(parent, theme, "Empty graph", "The graph has no nodes to draw.");
        return None;
    }
    let placed = placed_nodes(view.nodes, positions);
    let plot = GraphPlot {
        edges: plotted_edges(view.edges, &placed),
        color: theme.border,
        painted: None,
    };
    let selected = selected_node(view.nodes, selected);

    let mut root = parent.spawn((
        GraphHost,
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            min_height: Val::Px(theme.metrics.rem(MIN_HEIGHT_REM)),
            // The SVG viewport clips a node centred on the plot edge.
            overflow: Overflow::clip(),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(theme.background),
    ));
    let graph = root.id();
    root.with_children(|host| {
        host.spawn((
            plot,
            Node {
                width: Val::Px(0.0),
                height: Val::Px(0.0),
                flex_shrink: 0.0,
                ..default()
            },
        ))
        .with_children(|plot| {
            plot.spawn((GraphEdgeLayer, fill_plot()));
            for &(index, at) in &placed {
                let node = &view.nodes[index];
                let radius = node_radius(node.size);
                let (r, g, b) = node.color;
                plot.spawn((
                    GraphNodeDot { index },
                    GraphNodeTarget {
                        graph,
                        id: node.id.clone(),
                    },
                    Button,
                    disc(at, radius),
                    BackgroundColor(Color::srgb_u8(r, g, b)),
                    BorderRadius::MAX,
                ));
            }
            let Some((index, node)) = selected else {
                return;
            };
            let Some(&(_, at)) = placed.iter().find(|&&(placed, _)| placed == index) else {
                return;
            };
            let radius = node_radius(node.size);
            // Ring box: stroke centred on `radius + gap`, so its outer
            // edge sits half a stroke further out.
            let outer = radius + SELECTION_RING_GAP + RING_STROKE / 2.0;
            plot.spawn((
                GraphSelectionRing,
                disc(at, outer),
                BorderColor(theme.highlight),
                BorderRadius::MAX,
            ));
            let style = TextStyle::rem(theme, 0.62).color(theme.text).line_height(1.0);
            let line_box = style.line_box();
            plot.spawn((
                GraphNodeLabel,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(at[0] as f32 + (outer as f32) + LABEL_GAP),
                    top: Val::Percent(at[1] as f32),
                    margin: UiRect::top(Val::Px(-line_box / 2.0)),
                    height: Val::Px(line_box),
                    ..default()
                },
            ))
            .with_children(|label| {
                styled_text(label, node.label.clone(), style);
            });
        });
    });
    Some(graph)
}

/// The honest body when a graph cannot be drawn: `.pk-graph-notice`, a muted
/// `.78rem` block holding a bold title over its detail.
pub fn notice(parent: &mut ChildSpawnerCommands, theme: &BevyTheme, title: &str, detail: &str) {
    let style = TextStyle::rem(theme, 0.78).color(theme.text_muted).wrapping();
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(style.size),
            padding: UiRect::all(Val::Px(theme.metrics.rem(0.5))),
            ..default()
        })
        .with_children(|column| {
            styled_text(column, title, style.bold());
            styled_text(column, detail, style);
        });
}

fn fill_plot() -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: Val::Px(0.0),
        top: Val::Px(0.0),
        width: Val::Percent(100.0),
        height: Val::Percent(100.0),
        ..default()
    }
}

/// A square of half-side `radius` centred on `at`, both in plot units, which
/// are percent of the plot because the plot is `PLOT` units on a side.
fn disc(at: [f64; 2], radius: f64) -> Node {
    let percent = |value: f64| Val::Percent((value / PLOT * 100.0) as f32);
    Node {
        position_type: PositionType::Absolute,
        left: percent(at[0] - radius),
        top: percent(at[1] - radius),
        width: percent(2.0 * radius),
        height: percent(2.0 * radius),
        ..default()
    }
}

/// Sizes each graph's plot, paints its edges and emits [`GraphSelectEvent`].
pub struct GraphPlugin;

impl Plugin for GraphPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<GraphSelectEvent>()
            .add_systems(Update, (fit_plots, paint_edges, select_pressed).chain());
    }
}

fn select_pressed(
    dots: Query<(&Interaction, &GraphNodeTarget), Changed<Interaction>>,
    mut selected: EventWriter<GraphSelectEvent>,
) {
    for (interaction, target) in &dots {
        if *interaction == Interaction::Pressed {
            selected.write(GraphSelectEvent {
                graph: target.graph,
                node: target.id.clone(),
            });
        }
    }
}

/// Make each plot the shorter side of its host, and give each ring its stroke
/// at that scale.
fn fit_plots(
    hosts: Query<(&ComputedNode, &Children), With<GraphHost>>,
    mut plots: Query<(&mut Node, &Children), (With<GraphPlot>, Without<GraphSelectionRing>)>,
    mut rings: Query<&mut Node, (With<GraphSelectionRing>, Without<GraphPlot>)>,
) {
    for (computed, host_children) in &hosts {
        let logical = computed.size() * computed.inverse_scale_factor();
        let side = logical.x.min(logical.y).max(0.0);
        for plot in host_children {
            let Ok((mut node, plot_children)) = plots.get_mut(*plot) else {
                continue;
            };
            if node.width != Val::Px(side) || node.height != Val::Px(side) {
                node.width = Val::Px(side);
                node.height = Val::Px(side);
            }
            let stroke = Val::Px(RING_STROKE as f32 * side / PLOT as f32);
            for child in plot_children {
                if let Ok(mut ring) = rings.get_mut(*child) {
                    if ring.border != UiRect::all(stroke) {
                        ring.border = UiRect::all(stroke);
                    }
                }
            }
        }
    }
}

/// Repaint a plot's edge image when its physical size or scale factor changed.
fn paint_edges(
    mut commands: Commands,
    images: Option<ResMut<Assets<Image>>>,
    mut plots: Query<(&ComputedNode, &mut GraphPlot, &Children)>,
    layers: Query<(), With<GraphEdgeLayer>>,
) {
    let Some(mut images) = images else {
        return;
    };
    for (computed, mut plot, children) in &mut plots {
        let size = computed.size().round().as_uvec2();
        let scale = computed.inverse_scale_factor();
        if size.x == 0 || size.y == 0 || plot.painted == Some((size, scale)) {
            continue;
        }
        plot.painted = Some((size, scale));
        let pixels = rasterize(&plot.edges, plot.color, size);
        let handle = images.add(raster::rgba_image(size, pixels));
        for layer in children.iter().filter(|child| layers.contains(*child)) {
            commands.entity(layer).insert(ImageNode::new(handle.clone()));
        }
    }
}

/// Rasterise `edges` (plot units) into straight-alpha sRGB RGBA8 pixels of
/// `size`, each edge [`EDGE_STROKE`] plot units wide.
fn rasterize(edges: &[([f64; 2], [f64; 2])], color: Color, size: UVec2) -> Vec<u8> {
    let (width, height) = (size.x as usize, size.y as usize);
    let to_px = |point: [f64; 2]| {
        (
            point[0] / PLOT * f64::from(size.x),
            point[1] / PLOT * f64::from(size.y),
        )
    };
    let half = EDGE_STROKE / PLOT * f64::from(size.x) / 2.0;
    let mut coverage = vec![0.0f32; width * height];
    for &(from, to) in edges {
        raster::stroke_segment(&mut coverage, width, height, to_px(from), to_px(to), half);
    }
    let mut over = vec![[0.0f32; 4]; width * height];
    raster::composite(&mut over, &coverage, color);
    raster::flatten(&over)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use panel_kit_core::theme::ThemeTokens;
    use panel_kit_core::widgets::graph::{GraphEdge, GraphNode};

    /// A theme whose colours differ from the defaults' graph roles, so a
    /// hard-coded colour cannot pass.
    fn theme() -> BevyTheme {
        BevyTheme::from_tokens(&ThemeTokens::dark())
    }

    fn node(id: &str, color: (u8, u8, u8), size: f32) -> GraphNode {
        GraphNode {
            id: id.into(),
            label: id.to_uppercase(),
            sort: "metric".into(),
            color,
            size,
            mass: 1.0,
            repulsion: 1.0,
            species: String::new(),
        }
    }

    fn edge(source: u32, target: u32) -> GraphEdge {
        GraphEdge {
            source,
            target,
            kind: "derived_from".into(),
            weight: 1.0,
            rest_length: 60.0,
        }
    }

    /// Two nodes (one oversized) and the edge between them.
    fn model() -> GraphModel {
        GraphModel {
            nodes: vec![node("a", (10, 20, 30), 1.0), node("b", (40, 50, 60), 40.0)],
            edges: vec![edge(0, 1)],
        }
    }

    const POSITIONS: [[f32; 2]; 2] = [[0.25, 0.5], [0.75, 0.5]];

    fn paint(
        model: GraphModel,
        positions: &'static [[f32; 2]],
        selected: Option<&'static str>,
    ) -> World {
        let theme = theme();
        let mut world = World::new();
        let root = world.spawn(Node::default()).id();
        world
            .run_system_once(move |mut commands: Commands| {
                commands.entity(root).with_children(|parent| {
                    spawn(
                        parent,
                        &theme,
                        GraphView {
                            nodes: &model.nodes,
                            edges: &model.edges,
                        },
                        positions,
                        selected,
                    );
                });
            })
            .expect("the graph spawns");
        world
    }

    /// Each node dot's index, layout and fill, in model order.
    fn dots(world: &mut World) -> Vec<(usize, Node, Color)> {
        let mut query = world.query::<(&GraphNodeDot, &Node, &BackgroundColor)>();
        let mut found: Vec<_> = query
            .iter(world)
            .map(|(dot, node, fill)| (dot.index, node.clone(), fill.0))
            .collect();
        found.sort_by_key(|(index, ..)| *index);
        found
    }

    fn percent(value: Val) -> f32 {
        match value {
            Val::Percent(value) => value,
            other => panic!("not a percentage: {other:?}"),
        }
    }

    #[test]
    fn nodes_paint_at_their_plot_position_in_the_model_colour_with_a_clamped_radius() {
        let mut world = paint(model(), &POSITIONS, None);
        let dots = dots(&mut world);

        assert_eq!(dots.len(), 2);
        // `a`: size 1.0 clamps up to the 2.5 minimum, so a 5-unit box centred on x = 25.
        let (_, a, a_fill) = &dots[0];
        assert_eq!(*a_fill, Color::srgb_u8(10, 20, 30));
        assert_eq!(
            [percent(a.width), percent(a.height), percent(a.left), percent(a.top)],
            [5.0, 5.0, 22.5, 47.5]
        );
        // `b`: size 40 clamps down to the 9 maximum, an 18-unit box centred on x = 75.
        let (_, b, b_fill) = &dots[1];
        assert_eq!(*b_fill, Color::srgb_u8(40, 50, 60));
        assert_eq!(
            [percent(b.width), percent(b.height), percent(b.left), percent(b.top)],
            [18.0, 18.0, 66.0, 41.0]
        );
    }

    #[test]
    fn the_host_clips_nodes_that_reach_past_the_plot_edge() {
        let mut world = paint(model(), &POSITIONS, None);
        let mut hosts = world.query_filtered::<&Node, With<GraphHost>>();

        assert_eq!(hosts.single(&world).expect("one host").overflow, Overflow::clip());
    }

    #[test]
    fn a_node_without_a_position_paints_nothing() {
        let mut world = paint(model(), &POSITIONS[..1], None);

        let painted: Vec<usize> = dots(&mut world).iter().map(|dot| dot.0).collect();
        assert_eq!(painted, [0]);
    }

    #[test]
    fn edges_are_painted_from_the_placed_ends() {
        let mut world = paint(model(), &POSITIONS, None);

        let mut query = world.query::<&GraphPlot>();
        let plot = query.single(&world).expect("one plot");
        assert_eq!(plot.edges, [([25.0, 50.0], [75.0, 50.0])]);
        assert_eq!(plot.color, theme().border);

        let mut world = paint(model(), &POSITIONS[..1], None);
        let mut query = world.query::<&GraphPlot>();
        assert!(
            query.single(&world).expect("one plot").edges.is_empty(),
            "an edge needs both ends placed"
        );
    }

    #[test]
    fn edge_pixels_are_the_border_colour_on_the_line_and_clear_off_it() {
        // 200 px for 100 units: the 1-unit stroke is 2 px, so row 99 and 100
        // are covered whole.
        let size = UVec2::new(200, 200);
        let ink = theme().border.to_srgba();
        let pixels = rasterize(&[([25.0, 50.0], [75.0, 50.0])], theme().border, size);
        let at = |x: usize, y: usize| &pixels[(y * 200 + x) * 4..][..4];

        assert_eq!(
            at(100, 100),
            [
                (ink.red * 255.0).round() as u8,
                (ink.green * 255.0).round() as u8,
                (ink.blue * 255.0).round() as u8,
                255
            ]
        );
        assert_eq!(at(100, 106)[3], 0, "outside the stroke");
        assert_eq!(at(20, 100)[3], 0, "left of the edge's end");
    }

    #[test]
    fn only_the_selected_node_gets_a_ring_and_its_label() {
        let mut world = paint(model(), &POSITIONS, None);
        assert_eq!(world.query::<&GraphSelectionRing>().iter(&world).count(), 0);
        assert_eq!(world.query::<&GraphNodeLabel>().iter(&world).count(), 0);

        let mut world = paint(model(), &POSITIONS, Some("a"));
        let theme = theme();
        let mut rings = world.query::<(&GraphSelectionRing, &Node, &BorderColor)>();
        let (_, ring, color) = rings.single(&world).expect("one ring");
        // Radius 2.5 + gap 2 + half the 1.5 stroke, centred on (25, 50).
        let outer = 2.5 + SELECTION_RING_GAP + RING_STROKE / 2.0;
        assert_eq!(percent(ring.width) as f64, 2.0 * outer);
        assert_eq!(percent(ring.left) as f64, 25.0 - outer);
        assert_eq!(color.0, theme.highlight);

        let mut labels = world.query::<(&GraphNodeLabel, &Children)>();
        let (_, children) = labels.single(&world).expect("one label");
        let text_entity = children[0];
        let mut texts = world.query::<(&Text, &TextColor)>();
        let (text, tint) = texts.get(&world, text_entity).expect("label text");
        assert_eq!(text.0, "A");
        assert_eq!(tint.0, theme.text);
    }

    #[test]
    fn an_empty_model_paints_the_notice_instead_of_a_plot() {
        let mut world = paint(GraphModel::default(), &[], None);

        assert_eq!(world.query::<&GraphPlot>().iter(&world).count(), 0);
        let mut texts = world.query::<&Text>();
        let shown: String = texts.iter(&world).map(|text| text.0.as_str()).collect::<Vec<_>>().join("|");
        assert!(shown.contains("Empty graph"), "{shown}");
    }

    #[test]
    fn spawn_solved_places_every_node_of_an_inline_graph() {
        let theme = theme();
        let model = model();
        let layout = GraphLayout {
            algorithm: panel_kit_core::widgets::graph::LayoutAlgorithm::Circle,
            ..GraphLayout::default()
        };
        let mut world = World::new();
        let root = world.spawn(Node::default()).id();
        world
            .run_system_once(move |mut commands: Commands| {
                commands.entity(root).with_children(|parent| {
                    spawn_solved(parent, &theme, &model, &layout, None);
                });
            })
            .expect("the graph spawns");

        assert_eq!(dots(&mut world).len(), 2);
        let mut query = world.query::<&GraphPlot>();
        assert_eq!(query.single(&world).expect("one plot").edges.len(), 1);
    }

    #[test]
    fn the_plot_is_the_shorter_side_of_its_host() {
        let mut world = paint(model(), &POSITIONS, Some("a"));
        let mut hosts = world.query_filtered::<&mut ComputedNode, With<GraphHost>>();
        let mut computed = hosts.single_mut(&mut world).expect("one host");
        // A 300 x 200 logical box at scale 1.
        computed.size = Vec2::new(300.0, 200.0);
        computed.inverse_scale_factor = 1.0;
        world.run_system_once(fit_plots).expect("fit runs");

        let mut plots = world.query_filtered::<&Node, With<GraphPlot>>();
        let plot = plots.single(&world).expect("one plot");
        assert_eq!((plot.width, plot.height), (Val::Px(200.0), Val::Px(200.0)));
        let mut rings = world.query_filtered::<&Node, With<GraphSelectionRing>>();
        assert_eq!(rings.single(&world).expect("one ring").border, UiRect::all(Val::Px(3.0)));
    }

    /// An app with [`GraphPlugin`] and a spawned graph; returns the graph entity.
    fn app_with_graph(model: GraphModel) -> (App, Entity) {
        let theme = theme();
        let mut app = App::new();
        app.add_plugins(GraphPlugin);
        let root = app.world_mut().spawn(Node::default()).id();
        let graph = app
            .world_mut()
            .run_system_once(move |mut commands: Commands| {
                let mut graph = None;
                commands.entity(root).with_children(|parent| {
                    graph = spawn(
                        parent,
                        &theme,
                        GraphView {
                            nodes: &model.nodes,
                            edges: &model.edges,
                        },
                        &POSITIONS,
                        None,
                    );
                });
                graph
            })
            .expect("the graph spawns")
            .expect("a graph with nodes returns its entity");
        (app, graph)
    }

    fn dot_entity(app: &mut App, index: usize) -> Entity {
        let world = app.world_mut();
        let mut query = world.query::<(Entity, &GraphNodeDot)>();
        query
            .iter(world)
            .find(|(_, dot)| dot.index == index)
            .map(|(entity, _)| entity)
            .expect("the dot exists")
    }

    fn set_interaction(app: &mut App, entity: Entity, state: Interaction) {
        *app.world_mut().get_mut::<Interaction>(entity).expect("a dot carries Interaction") = state;
        app.update();
    }

    fn selections(app: &mut App) -> Vec<GraphSelectEvent> {
        app.world_mut().resource_mut::<Events<GraphSelectEvent>>().drain().collect()
    }

    #[test]
    fn pressing_a_node_emits_its_id_for_the_graph() {
        let (mut app, graph) = app_with_graph(model());
        let b = dot_entity(&mut app, 1);

        set_interaction(&mut app, b, Interaction::Pressed);
        assert_eq!(selections(&mut app), vec![GraphSelectEvent { graph, node: "b".into() }]);

        let a = dot_entity(&mut app, 0);
        set_interaction(&mut app, a, Interaction::Pressed);
        assert_eq!(selections(&mut app), vec![GraphSelectEvent { graph, node: "a".into() }]);
    }

    #[test]
    fn hovering_a_node_or_pressing_it_again_without_release_selects_nothing_new() {
        let (mut app, _) = app_with_graph(model());
        let a = dot_entity(&mut app, 0);

        set_interaction(&mut app, a, Interaction::Hovered);
        assert!(selections(&mut app).is_empty());
        set_interaction(&mut app, a, Interaction::Pressed);
        assert_eq!(selections(&mut app).len(), 1);
        // Unchanged Interaction is not a new press.
        app.update();
        assert!(selections(&mut app).is_empty());
        // Release then press is a second selection of the same node.
        set_interaction(&mut app, a, Interaction::Hovered);
        set_interaction(&mut app, a, Interaction::Pressed);
        assert_eq!(selections(&mut app).len(), 1);
    }

    #[test]
    fn an_empty_graph_has_no_entity_to_select_on() {
        let theme = theme();
        let mut world = World::new();
        let root = world.spawn(Node::default()).id();
        let graph = world
            .run_system_once(move |mut commands: Commands| {
                let mut graph = Some(root);
                commands.entity(root).with_children(|parent| {
                    graph = spawn(parent, &theme, GraphView { nodes: &[], edges: &[] }, &[], None);
                });
                graph
            })
            .expect("the notice spawns");
        assert_eq!(graph, None);
    }
}
