//! Bevy UI port of panel-kit core's trace views: the activity swimlane
//! ([`swimlane`]) and the session rail ([`rails`]).
//!
//! Both core models arrive with every coordinate resolved in CSS pixels, so
//! this module only turns model values into absolutely positioned [`Node`]s.
//! The swimlane canvas is laid out at `Graph::width` x `Graph::height` and the
//! rail rows at `RailCanvas::height`; a host may clip or scroll that box
//! without the widget re-deriving time.
//!
//! Core behaviour that Bevy UI has no primitive for is left to the host:
//! SVG elbow paths (`Item::edges`, `RailCanvas::edges`) and the pointer
//! tooltips carried by `Item::tooltip`, `Dot::title` and `RailRow::title`.
//! The axis ticks, idle hatching and closed strip density cells are not
//! spawned either: a closed strip row draws its caption and count, and the
//! axis band stays empty for a host's own time labels.
//! Selection is read from the models, never computed here: pass the id a host
//! already selected and the matching item or row is highlighted.

use bevy::prelude::*;

use panel_kit_core::widgets::trace::rails::view::EMPTY_TEXT;
use panel_kit_core::widgets::trace::rails::{
    Fork as RailFork, RailCanvas, RailRow, RailView, ROW_PX,
};
use panel_kit_core::widgets::trace::swimlane::metrics::{label_px, FORK_NODE_PX, ROW};
use panel_kit_core::widgets::trace::swimlane::{Dot, Fork, Graph, Item, LooseRow, StripRow};
use panel_kit_core::widgets::trace::{EventKind, Tone};

use super::label_text;
use crate::theme::BevyTheme;

/// Opacity of a landed item's line, matching the `done` rule.
const DONE_ALPHA: f32 = 0.45;
/// Opacity of a fork lane that lost.
const DEAD_ALPHA: f32 = 0.35;
/// Side of a single event dot.
const DOT_PX: f32 = 6.0;
/// Side of a burst glyph.
const CLUSTER_PX: f32 = 10.0;
/// Side of a fork lane's tip node.
const FORK_TIP_PX: f32 = 7.0;
/// Width of the bar marking a selected rail row.
const SELECTION_BAR_PX: f32 = 3.0;

/// Root entity of a swimlane canvas.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TraceSwimlane;

/// One work item's lane row; `lane` is its packed row rank.
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub struct TraceLane {
    /// The item id.
    pub id: String,
    /// Row rank, 0 at the top.
    pub lane: usize,
}

/// The item's time span: the line from its first dot to its last.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct TraceSpan {
    /// Left end in canvas px, `Item::x0`.
    pub x0: f32,
    /// Right end in canvas px, `Item::x1`.
    pub x1: f32,
}

/// One event dot, or the single glyph standing for a burst.
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub struct TraceDot {
    /// Event id, or the first event's id for a burst.
    pub id: String,
    /// Events the glyph stands for; 2 or more is a burst.
    pub count: u32,
    /// Dot colour class.
    pub tone: Tone,
}

/// The item id drawn at the tip of its lane, or at its start for a long line.
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub struct TraceLaneLabel {
    /// The item id.
    pub id: String,
    /// `true` for the start label, `false` for the tip label.
    pub start: bool,
}

/// One fork lane under its item.
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub struct TraceForkLane {
    /// The lane id.
    pub id: String,
}

/// The horizontal line of a fork lane.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct TraceForkLine {
    /// The lane id.
    pub id: String,
}

/// One closed no-item strip row.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct TraceStripRow {
    /// The joined event kind.
    pub kind: EventKind,
    /// Events in the row.
    pub count: usize,
}

/// One open no-item row with its events as dots.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct TraceLooseRow {
    /// The joined event kind.
    pub kind: EventKind,
    /// Events in the row.
    pub count: usize,
}

/// The picked item's band, or the selected rail row.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TraceSelected;

/// Root entity of a rail view.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TraceRails;

/// One rail row's clickable band.
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub struct TraceRailRow {
    /// Event id, or `lane:<lane id>` for a marker.
    pub id: String,
    /// The row stands for a lane start rather than an event.
    pub marker: bool,
}

/// One row's dot in the rail gutter.
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub struct TraceRailDot {
    /// Event id, or `lane:<lane id>` for a marker.
    pub id: String,
    /// The row is the view's selected one.
    pub selected: bool,
}

/// Spawn the swimlane canvas: one lane per item, each with its span, dots,
/// labels and fork lanes, plus the no-item strip rows.
///
/// `selected` is the item id a host has picked; the matching lane gets a
/// tinted, ringed band over `Item::hit_rect`. Unknown ids draw no highlight.
///
/// The graph must have its items built: pass a [`Graph::layout`] layout, not
/// the lazy [`Graph::layout_slice`] one, whose items are built on demand from
/// events this renderer does not receive.
pub fn swimlane(
    parent: &mut ChildSpawnerCommands,
    theme: &BevyTheme,
    graph: &Graph,
    selected: Option<&str>,
) {
    parent
        .spawn((
            TraceSwimlane,
            Node {
                width: Val::Px(graph.width as f32),
                height: Val::Px(graph.height as f32),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(theme.background),
        ))
        .with_children(|canvas| {
            if graph.is_empty() {
                label_at(canvas, EMPTY_TEXT, 10.0, theme.text_muted, 8.0, graph.axis_y as f32);
                return;
            }
            for row in &graph.strip {
                strip_row(canvas, theme, row, graph.width as f32);
            }
            for row in &graph.loose {
                loose_row(canvas, theme, row, graph.width as f32);
            }
            for item in &graph.items {
                item_lane(canvas, theme, graph.width as f32, item, selected == Some(item.id.as_str()));
            }
        });
}

/// Spawn a rail view: its heading, empty state, gutter lines, and one band per
/// row with the row's stamp, chip and caption.
///
/// The view already resolves `on_select` and `selected`, so a selected row is
/// highlighted exactly when [`RailRow::selected`] is set; a lane marker is
/// never selectable and so never highlighted.
pub fn rails(parent: &mut ChildSpawnerCommands, theme: &BevyTheme, view: &RailView) {
    parent
        .spawn((
            TraceRails,
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                ..default()
            },
        ))
        .with_children(|rail| {
            if let Some(heading) = &view.heading {
                label_text(rail, heading.title.clone(), 11.0, theme.text);
                if let Some(meta) = &heading.meta {
                    label_text(rail, meta.clone(), 10.0, theme.text_muted);
                }
            }
            match &view.canvas {
                Some(canvas) => rail_canvas(rail, theme, canvas),
                None => label_text(rail, view.empty.unwrap_or(EMPTY_TEXT), 11.0, theme.text_muted),
            }
        });
}

/// Colour of a core tone class; `Hollow` draws as the muted outline colour.
fn tone_color(theme: &BevyTheme, tone: Tone) -> Color {
    match tone {
        Tone::Blue => theme.primary,
        Tone::Green => theme.success,
        Tone::Yellow => theme.warning,
        Tone::Red => theme.error,
        Tone::Hollow => theme.text_muted,
    }
}

/// Pixel value of a resolved coordinate.
fn px(value: f32) -> Val {
    Val::Px(value)
}

/// An absolutely positioned box in its parent's coordinate space.
fn boxed(left: f32, top: f32, width: f32, height: f32) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: px(left),
        top: px(top),
        width: px(width),
        height: px(height),
        ..default()
    }
}

/// A solid rectangle: the primitive Bevy UI has for lines, dots and bars.
fn rect(parent: &mut ChildSpawnerCommands, node: Node, tint: Color) {
    parent.spawn((node, BackgroundColor(tint)));
}

/// Text placed at an absolute canvas coordinate.
fn label_at(
    parent: &mut ChildSpawnerCommands,
    value: impl Into<String>,
    size: f32,
    tint: Color,
    left: f32,
    top: f32,
) {
    parent.spawn((
        Text::new(value),
        TextFont {
            font_size: size,
            ..default()
        },
        TextColor(tint),
        Node {
            position_type: PositionType::Absolute,
            left: px(left),
            top: px(top),
            ..default()
        },
    ));
}

/// One item lane: its span, dots, fork lanes, and tip and start labels.
fn item_lane(
    parent: &mut ChildSpawnerCommands,
    theme: &BevyTheme,
    width: f32,
    item: &Item,
    picked: bool,
) {
    parent
        .spawn((
            TraceLane {
                id: item.id.clone(),
                lane: item.lane,
            },
            boxed(0.0, item.y as f32, width, 0.0),
        ))
        .with_children(|lane| {
            if picked {
                selection_band(lane, theme, item);
            }
            let span_width = (item.x1 - item.x0).max(0.0) as f32;
            let tint = if item.done {
                tone_color(theme, item.tone).with_alpha(DONE_ALPHA)
            } else {
                tone_color(theme, item.tone)
            };
            lane.spawn((
                TraceSpan {
                    x0: item.x0 as f32,
                    x1: item.x1 as f32,
                },
                boxed(item.x0 as f32, -1.0, span_width, 2.0),
                BackgroundColor(tint),
            ));
            for dot in &item.dots {
                dot_node(lane, theme, dot, dot.x as f32 - dot_side(dot) / 2.0, -dot_side(dot) / 2.0);
            }
            for fork in &item.forks {
                fork_lane(lane, theme, width, item, fork);
            }
            lane_label(lane, item, false, item.label_x as f32, tip_tint(theme, picked));
            if let Some(at) = item.start_label_x {
                lane_label(lane, item, true, (at - label_px(&item.id)) as f32, theme.text_muted);
            }
        });
}

/// One of an item's two id labels: at its start for a long line, else at its tip.
fn lane_label(parent: &mut ChildSpawnerCommands, item: &Item, start: bool, left: f32, tint: Color) {
    parent.spawn((
        TraceLaneLabel {
            id: item.id.clone(),
            start,
        },
        Text::new(item.id.clone()),
        TextFont {
            font_size: 10.0,
            ..default()
        },
        TextColor(tint),
        Node {
            position_type: PositionType::Absolute,
            left: px(left),
            top: px(-7.0),
            ..default()
        },
    ));
}

/// The picked lane's tinted band with a primary ring, over `Item::hit_rect`.
fn selection_band(parent: &mut ChildSpawnerCommands, theme: &BevyTheme, item: &Item) {
    let hit = item.hit_rect();
    parent.spawn((
        TraceSelected,
        Node {
            border: UiRect::all(Val::Px(1.0)),
            ..boxed(hit.x as f32, (hit.y - item.y) as f32, hit.w as f32, hit.h as f32)
        },
        BorderRadius::all(Val::Px(1.0)),
        BackgroundColor(theme.primary.with_alpha(0.14)),
        BorderColor(theme.primary),
    ));
}

/// Tip labels use full-strength text only for the picked lane.
fn tip_tint(theme: &BevyTheme, picked: bool) -> Color {
    if picked {
        theme.text
    } else {
        theme.text_muted
    }
}

/// Side of a dot glyph: small for one event, larger for a burst.
fn dot_side(dot: &Dot) -> f32 {
    if dot.is_cluster() {
        CLUSTER_PX
    } else {
        DOT_PX
    }
}

/// One dot: a ringed disc for an event, a numbered disc for a burst.
fn dot_node(parent: &mut ChildSpawnerCommands, theme: &BevyTheme, dot: &Dot, left: f32, top: f32) {
    let side = dot_side(dot);
    let (fill, ring) = if dot.tone == Tone::Hollow {
        (theme.background, theme.text_muted)
    } else {
        (tone_color(theme, dot.tone), tone_color(theme, dot.tone))
    };
    let mut glyph = parent.spawn((
        TraceDot {
            id: dot.id.clone(),
            count: dot.count,
            tone: dot.tone,
        },
        Node {
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            border: UiRect::all(Val::Px(1.0)),
            ..boxed(left, top, side, side)
        },
        BorderRadius::MAX,
        BackgroundColor(fill),
        BorderColor(ring),
    ));
    if dot.is_cluster() {
        let count_tint = if dot.tone == Tone::Hollow {
            theme.text
        } else {
            theme.background
        };
        glyph.with_children(|glyph| label_text(glyph, dot.count.to_string(), 8.0, count_tint));
    }
}

/// A fork lane: its line, tip node and caption, on the row under its item.
fn fork_lane(
    parent: &mut ChildSpawnerCommands,
    theme: &BevyTheme,
    width: f32,
    item: &Item,
    fork: &Fork,
) {
    let lane_tint = if fork.dead {
        theme.text_muted.with_alpha(DEAD_ALPHA)
    } else {
        theme.text_muted
    };
    let node_tint = if fork.dead {
        tone_color(theme, fork.tone).with_alpha(DEAD_ALPHA)
    } else {
        tone_color(theme, fork.tone)
    };
    let caption_tint = if fork.dead {
        theme.text.with_alpha(DEAD_ALPHA)
    } else {
        theme.text
    };
    parent
        .spawn((
            TraceForkLane {
                id: fork.id.clone(),
            },
            boxed(0.0, (fork.y - item.y) as f32, width, 0.0),
        ))
        .with_children(|lane| {
            lane.spawn((
                TraceForkLine {
                    id: fork.id.clone(),
                },
                boxed(fork.x0 as f32, -0.5, (fork.x1 - fork.x0).max(0.0) as f32, 1.0),
                BackgroundColor(lane_tint),
            ));
            rect(
                lane,
                boxed(fork.node_left() as f32, -FORK_TIP_PX / 2.0, FORK_TIP_PX, FORK_TIP_PX),
                node_tint,
            );
            label_at(
                lane,
                fork.label.clone(),
                10.0,
                caption_tint,
                (fork.x1 + FORK_NODE_PX) as f32,
                -6.0,
            );
        });
}

/// A closed no-item row: its caption and count on the strip row.
fn strip_row(parent: &mut ChildSpawnerCommands, theme: &BevyTheme, row: &StripRow, width: f32) {
    parent
        .spawn((
            TraceStripRow {
                kind: row.kind,
                count: row.count,
            },
            Node {
                align_items: AlignItems::Center,
                padding: UiRect::left(Val::Px(4.0)),
                ..boxed(0.0, row.y0 as f32, width, ROW as f32)
            },
        ))
        .with_children(|line| {
            label_text(line, format!("{} · {}", row.label(), row.count), 9.0, theme.text_muted);
        });
}

/// An open no-item row: its events as dots on one line.
fn loose_row(parent: &mut ChildSpawnerCommands, theme: &BevyTheme, row: &LooseRow, width: f32) {
    parent
        .spawn((
            TraceLooseRow {
                kind: row.kind,
                count: row.count,
            },
            boxed(0.0, row.y as f32, width, 0.0),
        ))
        .with_children(|line| {
            rect(line, boxed(0.0, -0.5, width, 1.0), theme.divider);
            for dot in &row.dots {
                dot_node(line, theme, dot, dot.x as f32 - dot_side(dot) / 2.0, -dot_side(dot) / 2.0);
            }
        });
}

/// The rail canvas: its trunk, fork lanes, row dots and row bands.
fn rail_canvas(parent: &mut ChildSpawnerCommands, theme: &BevyTheme, canvas: &RailCanvas) {
    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Px(canvas.height as f32),
            ..default()
        })
        .with_children(|area| {
            rail_trunk(area, theme, canvas);
            for fork in &canvas.forks {
                rail_fork(area, theme, fork);
            }
            for row in &canvas.rows {
                rail_dot(area, theme, row);
                rail_row(area, theme, canvas.gutter as f32, row);
            }
        });
}

/// The item line down column zero, faded once the item has landed.
fn rail_trunk(parent: &mut ChildSpawnerCommands, theme: &BevyTheme, canvas: &RailCanvas) {
    let tint = if canvas.done {
        tone_color(theme, canvas.tone).with_alpha(DONE_ALPHA)
    } else {
        tone_color(theme, canvas.tone)
    };
    let height = (canvas.trunk_y1 - canvas.trunk_y0).max(0) as f32;
    rect(
        parent,
        boxed(canvas.trunk_x as f32 - 1.0, canvas.trunk_y0 as f32, 2.0, height),
        tint,
    );
}

/// One fork lane's vertical line beside the trunk.
fn rail_fork(parent: &mut ChildSpawnerCommands, theme: &BevyTheme, fork: &RailFork) {
    let tint = if fork.dead {
        theme.text_muted.with_alpha(DEAD_ALPHA)
    } else {
        theme.divider
    };
    let height = (fork.y1 - fork.y0).max(0) as f32;
    rect(
        parent,
        boxed(fork.x as f32, fork.y0 as f32, 1.0, height),
        tint,
    );
}

/// One row's dot in the gutter; the selected dot takes the text-coloured ring.
fn rail_dot(parent: &mut ChildSpawnerCommands, theme: &BevyTheme, row: &RailRow) {
    let (fill, ring, border) = if row.selected {
        (tone_color(theme, row.tone), theme.text, 2.0)
    } else if row.tone == Tone::Hollow {
        (theme.background, theme.text_muted, 1.5)
    } else {
        (tone_color(theme, row.tone), tone_color(theme, row.tone), 1.5)
    };
    parent.spawn((
        TraceRailDot {
            id: row.row_id.clone(),
            selected: row.selected,
        },
        Node {
            border: UiRect::all(Val::Px(border)),
            ..boxed(row.x as f32 - 3.5, row.y as f32 - 3.5, 7.0, 7.0)
        },
        BorderRadius::MAX,
        BackgroundColor(fill),
        BorderColor(ring),
    ));
}

/// One row band: its stamp, chip and caption, framed when selected.
fn rail_row(parent: &mut ChildSpawnerCommands, theme: &BevyTheme, gutter: f32, row: &RailRow) {
    let framed = row.selected || row.open;
    let background = if framed {
        theme.primary.with_alpha(0.14)
    } else {
        Color::NONE
    };
    let border = if framed {
        UiRect::all(Val::Px(1.0))
    } else {
        UiRect::ZERO
    };
    let mut band = parent.spawn((
        TraceRailRow {
            id: row.row_id.clone(),
            marker: row.lane_marker,
        },
        Node {
            align_items: AlignItems::Center,
            column_gap: Val::Px(8.0),
            padding: UiRect::left(Val::Px(4.0)),
            border,
            left: px(gutter),
            right: Val::Px(0.0),
            top: px(row.y as f32 - ROW_PX as f32 / 2.0),
            height: Val::Px(ROW_PX as f32),
            position_type: PositionType::Absolute,
            ..default()
        },
        BackgroundColor(background),
        BorderColor(theme.primary),
    ));
    if row.selected {
        band.insert(TraceSelected);
    }
    band.with_children(|line| {
        if row.selected {
            rect(
                line,
                boxed(0.0, 0.0, SELECTION_BAR_PX, ROW_PX as f32),
                theme.primary,
            );
        }
        label_text(line, row.stamp.clone(), 10.0, theme.text_muted);
        if let Some(chip) = &row.chip {
            label_text(line, chip.clone(), 10.0, theme.primary);
        }
        let tint = if row.lane_marker {
            theme.text_muted
        } else {
            theme.text
        };
        label_text(line, row.label.clone(), 11.0, tint);
    });
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;
    use panel_kit_core::widgets::trace::rails::{vertical, RailOptions, VerticalGraph};
    use panel_kit_core::widgets::trace::swimlane::{Options, Range};
    use panel_kit_core::widgets::trace::{
        Edge, EdgeKind, Event, EventKind, ItemRef, Lane, LaneKind, SessionTree,
    };

    const SEC: i64 = 1_000_000;
    const HOUR: i64 = 3_600 * SEC;
    const WINDOW: i64 = 8 * 24 * HOUR;
    const WIDTH: u32 = 1400;

    // -- systems under test ---------------------------------------------------

    #[derive(Resource)]
    struct SwimlaneFixture {
        graph: Graph,
        selected: Option<String>,
    }

    #[derive(Resource)]
    struct RailsFixture {
        view: RailView,
    }

    fn draw_swimlane(mut commands: Commands, fixture: Res<SwimlaneFixture>) {
        let theme = BevyTheme::default();
        commands
            .spawn(Node::default())
            .with_children(move |parent| {
                swimlane(parent, &theme, &fixture.graph, fixture.selected.as_deref());
            });
    }

    fn draw_rails(mut commands: Commands, fixture: Res<RailsFixture>) {
        let theme = BevyTheme::default();
        commands
            .spawn(Node::default())
            .with_children(move |parent| rails(parent, &theme, &fixture.view));
    }

    fn swimlane_app(graph: &Graph, selected: Option<&str>) -> App {
        let mut app = App::new();
        app.insert_resource(SwimlaneFixture {
            graph: graph.clone(),
            selected: selected.map(str::to_string),
        });
        app.add_systems(Startup, draw_swimlane);
        app.update();
        app
    }

    fn rails_app(view: RailView) -> App {
        let mut app = App::new();
        app.insert_resource(RailsFixture { view });
        app.add_systems(Startup, draw_rails);
        app.update();
        app
    }

    // -- swimlane fixtures ----------------------------------------------------

    fn uid() -> String {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        format!("t:{}", NEXT.fetch_add(1, Ordering::Relaxed))
    }

    fn at(hours: f64) -> i64 {
        (hours * HOUR as f64).round() as i64
    }

    fn ev(item: &str, hour: f64) -> Event {
        Event::new(uid(), at(hour), EventKind::Note)
            .agent("worker")
            .item(item)
    }

    fn loose_ev(kind: EventKind, hour: f64) -> Event {
        Event::new(uid(), at(hour), kind).agent("worker")
    }

    fn layout(events: &[Event]) -> Graph {
        Graph::layout(events, &Options::new(WIDTH, Range::new(0, WINDOW)))
    }

    fn layout_with(events: &[Event], tree: &SessionTree) -> Graph {
        let options = Options::new(WIDTH, Range::new(0, WINDOW)).tree(tree);
        Graph::layout(events, &options)
    }

    fn fork_lane(item: &str, from: f64, to: f64, label: &str) -> Lane {
        Lane {
            id: format!("{item}/collab/{label}"),
            item_id: item.to_owned(),
            kind: LaneKind::Collab,
            label: label.to_owned(),
            from: at(from),
            to: at(to),
            chosen: None,
            parent: None,
            agent: Some(label.to_owned()),
        }
    }

    fn edge(kind: EdgeKind, lane: &Lane, item: &str, hour: f64) -> Edge {
        Edge {
            kind,
            lane: lane.id.clone(),
            item_id: item.to_owned(),
            at: at(hour),
        }
    }

    // -- rails fixtures -------------------------------------------------------

    fn re(id: &str, hour: f64, agent: &str) -> Event {
        Event::new(id, at(hour), EventKind::Note)
            .agent(agent)
            .item("imp-16")
            .summary(format!("{id} said"))
    }

    fn lane_of(
        id: &str,
        from: f64,
        to: f64,
        agent: Option<&str>,
        kind: LaneKind,
        label: &str,
    ) -> Lane {
        Lane {
            id: id.to_owned(),
            item_id: "imp-16".to_owned(),
            kind,
            label: label.to_owned(),
            from: at(from),
            to: at(to),
            chosen: None,
            parent: None,
            agent: agent.map(str::to_owned),
        }
    }

    fn collab(from: f64, to: f64, agent: &str) -> Lane {
        lane_of(
            &format!("imp-16/collab/{agent}"),
            from,
            to,
            Some(agent),
            LaneKind::Collab,
            agent,
        )
    }

    fn tree(lanes: Vec<Lane>, edges: Vec<Edge>) -> SessionTree {
        SessionTree {
            agent: Some("clara".to_owned()),
            lanes,
            edges,
        }
    }

    fn rail_graph(events: &[Event], tree: &SessionTree) -> VerticalGraph {
        vertical(
            events,
            &ItemRef {
                id: "imp-16".to_owned(),
                status: Some("claimed".to_owned()),
            },
            tree,
        )
    }

    fn view_selecting(graph: &VerticalGraph, selected: Option<&str>) -> RailView {
        graph.view(&RailOptions {
            on_select: true,
            selected: selected.map(str::to_owned),
            ..RailOptions::default()
        })
    }

    // -- queries --------------------------------------------------------------

    fn count<T: Component>(app: &mut App) -> usize {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<T>>();
        query.iter(world).count()
    }

    fn root_node(app: &mut App) -> Node {
        let world = app.world_mut();
        let mut query = world.query_filtered::<&Node, With<TraceSwimlane>>();
        query.single(world).expect("swimlane root").clone()
    }

    fn texts(app: &mut App) -> Vec<String> {
        let world = app.world_mut();
        let mut query = world.query::<&Text>();
        query.iter(world).map(|text| text.0.clone()).collect()
    }

    fn texts_under(app: &App, entity: Entity) -> Vec<String> {
        let mut found = Vec::new();
        let mut pending = vec![entity];
        while let Some(current) = pending.pop() {
            if let Some(text) = app.world().get::<Text>(current) {
                found.push(text.0.clone());
            }
            if let Some(children) = app.world().get::<Children>(current) {
                for child in children.iter() {
                    pending.push(child);
                }
            }
        }
        found
    }

    fn children_of(app: &App, entity: Entity) -> Vec<Entity> {
        app.world()
            .get::<Children>(entity)
            .map(|children| children.iter().collect())
            .unwrap_or_default()
    }

    fn lane_entity(app: &mut App, id: &str) -> Entity {
        let world = app.world_mut();
        let mut query = world.query::<(Entity, &TraceLane)>();
        query
            .iter(world)
            .find(|(_, lane)| lane.id == id)
            .map(|(entity, _)| entity)
            .expect("item lane")
    }

    /// Item lanes top to bottom.
    fn lanes(app: &mut App) -> Vec<(TraceLane, Node)> {
        let world = app.world_mut();
        let mut query = world.query::<(&TraceLane, &Node)>();
        let mut found: Vec<(TraceLane, Node)> = query
            .iter(world)
            .map(|(lane, node)| (lane.clone(), node.clone()))
            .collect();
        found.sort_by_key(|(lane, _)| lane.lane);
        found
    }

    fn lane_ranks(graph: &Graph) -> Vec<(String, usize)> {
        let mut app = swimlane_app(graph, None);
        lanes(&mut app)
            .into_iter()
            .map(|(lane, _)| (lane.id, lane.lane))
            .collect()
    }

    fn rank_of(ranks: &[(String, usize)], id: &str) -> usize {
        ranks
            .iter()
            .find(|(lane, _)| lane == id)
            .map(|(_, rank)| *rank)
            .expect("lane rank")
    }

    fn span_of(app: &mut App, id: &str) -> (TraceSpan, Node, Color) {
        let lane = lane_entity(app, id);
        for child in children_of(app, lane) {
            let world = app.world();
            if let Some(span) = world.get::<TraceSpan>(child) {
                return (
                    *span,
                    world.get::<Node>(child).expect("span node").clone(),
                    world.get::<BackgroundColor>(child).expect("span tint").0,
                );
            }
        }
        panic!("span of {id}");
    }

    fn lane_labels(app: &mut App, id: &str) -> Vec<(TraceLaneLabel, Node)> {
        let lane = lane_entity(app, id);
        children_of(app, lane)
            .into_iter()
            .filter_map(|child| {
                let world = app.world();
                Some((
                    world.get::<TraceLaneLabel>(child)?.clone(),
                    world.get::<Node>(child)?.clone(),
                ))
            })
            .collect()
    }

    fn dots(app: &mut App, item: &str) -> Vec<(TraceDot, Node, Color)> {
        let lane = lane_entity(app, item);
        children_of(app, lane)
            .into_iter()
            .filter_map(|child| {
                let world = app.world();
                Some((
                    world.get::<TraceDot>(child)?.clone(),
                    world.get::<Node>(child)?.clone(),
                    world.get::<BackgroundColor>(child)?.0,
                ))
            })
            .collect()
    }

    fn fork_entity(app: &mut App, id: &str) -> Entity {
        let world = app.world_mut();
        let mut query = world.query::<(Entity, &TraceForkLane)>();
        query
            .iter(world)
            .find(|(_, lane)| lane.id == id)
            .map(|(entity, _)| entity)
            .expect("fork lane")
    }

    /// The picked band(s), in spawn order.
    fn selection_bands(app: &mut App) -> Vec<(Entity, Node, Color)> {
        let world = app.world_mut();
        let mut query =
            world.query_filtered::<(Entity, &Node, &BackgroundColor), With<TraceSelected>>();
        query
            .iter(world)
            .map(|(entity, node, tint)| (entity, node.clone(), tint.0))
            .collect()
    }

    fn strip_rows(app: &mut App) -> Vec<TraceStripRow> {
        let world = app.world_mut();
        let mut query = world.query::<&TraceStripRow>();
        query.iter(world).cloned().collect()
    }

    /// Rail rows top to bottom, with their entities.
    fn rail_rows(app: &mut App) -> Vec<(Entity, TraceRailRow, Node)> {
        let world = app.world_mut();
        let mut query = world.query::<(Entity, &TraceRailRow, &Node)>();
        let mut found: Vec<(Entity, TraceRailRow, Node)> = query
            .iter(world)
            .map(|(entity, row, node)| (entity, row.clone(), node.clone()))
            .collect();
        found.sort_by(|a, b| match (a.2.top, b.2.top) {
            (Val::Px(a), Val::Px(b)) => a.partial_cmp(&b).expect("row tops"),
            _ => std::cmp::Ordering::Equal,
        });
        found
    }

    fn selected_rail_rows(app: &mut App) -> Vec<TraceRailRow> {
        let world = app.world_mut();
        let mut query = world.query_filtered::<&TraceRailRow, With<TraceSelected>>();
        query.iter(world).cloned().collect()
    }

    // -- swimlane: lanes ------------------------------------------------------

    // Ported from `swimlane/tests/graph.rs::items_never_open_together_share_a_row_and_no_more_rows_are_used_than_ever_overlap`.
    #[test]
    fn items_never_open_together_share_a_lane() {
        let graph = layout(&[
            ev("imp-1", 0.0),
            ev("imp-1", 20.0),
            ev("imp-2", 10.0),
            ev("imp-2", 100.0),
            ev("imp-3", 60.0),
            ev("imp-3", 80.0),
        ]);
        assert_eq!(graph.lanes, 2, "model precondition");

        let mut app = swimlane_app(&graph, None);
        let lanes = lanes(&mut app);
        assert_eq!(lanes.len(), 3);
        let rank = |id: &str| {
            lanes
                .iter()
                .find(|(lane, _)| lane.id == id)
                .expect("lane")
                .0
                .lane
        };
        let top = |id: &str| match lanes
            .iter()
            .find(|(lane, _)| lane.id == id)
            .expect("lane")
            .1
            .top
        {
            Val::Px(top) => top,
            other => panic!("lane top is {other:?}"),
        };
        assert_eq!(rank("imp-1"), rank("imp-3"));
        assert_ne!(rank("imp-1"), rank("imp-2"));
        assert_eq!(top("imp-1"), top("imp-3"), "a shared lane is one row");

        let mut rows: Vec<(usize, f32)> = lanes
            .iter()
            .map(|(lane, node)| {
                let top = match node.top {
                    Val::Px(top) => top,
                    other => panic!("lane top is {other:?}"),
                };
                (lane.lane, top)
            })
            .collect();
        rows.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)));
        rows.dedup_by_key(|(lane, _)| *lane);
        assert_eq!(rows.len(), 2, "two packed rows");
        assert!(rows[0].1 < rows[1].1, "row 0 sits above row 1: {rows:?}");
    }

    // Ported from `swimlane/tests/graph.rs::a_branch_that_starts_under_the_previous_ones_tip_label_takes_another_row`.
    #[test]
    fn a_branch_starting_under_the_previous_tip_label_takes_another_lane() {
        let graph = layout(&[
            ev("imp-1", 0.0),
            ev("imp-1", 10.0),
            ev("imp-2", 10.0),
            ev("imp-2", 40.0),
        ]);

        let ranks = lane_ranks(&graph);
        assert_ne!(rank_of(&ranks, "imp-1"), rank_of(&ranks, "imp-2"));
    }

    // Ported from `swimlane/tests/graph.rs::a_branch_that_starts_after_the_label_has_ended_reuses_the_row`.
    #[test]
    fn a_branch_starting_after_the_label_reuses_the_lane() {
        let graph = layout(&[
            ev("imp-1", 0.0),
            ev("imp-1", 10.0),
            ev("imp-2", 60.0),
            ev("imp-2", 90.0),
        ]);
        assert_eq!(graph.lanes, 1, "model precondition");

        let ranks = lane_ranks(&graph);
        assert_eq!(rank_of(&ranks, "imp-1"), rank_of(&ranks, "imp-2"));
    }

    // -- swimlane: spans and labels ------------------------------------------

    #[test]
    fn the_span_maps_the_items_start_and_end_to_left_and_width() {
        let graph = layout(&[ev("imp-9", 10.0), ev("imp-9", 30.0)]);
        let item = graph.items[0].clone();
        let mut app = swimlane_app(&graph, None);

        let lane = lanes(&mut app)
            .into_iter()
            .find(|(lane, _)| lane.id == "imp-9")
            .expect("lane");
        assert_eq!(lane.1.top, Val::Px(item.y as f32));

        let (span, node, _) = span_of(&mut app, "imp-9");
        assert_eq!(node.left, Val::Px(item.x0 as f32));
        assert_eq!(node.width, Val::Px((item.x1 - item.x0) as f32));
        assert_eq!(node.top, Val::Px(-1.0), "line centred on the row");
        assert_eq!(node.height, Val::Px(2.0));
        assert_eq!((span.x0, span.x1), (item.x0 as f32, item.x1 as f32));
        assert!(item.x0 < item.x1, "two events span the axis");
    }

    // Ported from `swimlane/tests/graph.rs::both_ends_of_a_segment_are_labelled_a_single_dot_is_labelled_once`.
    #[test]
    fn both_ends_of_a_segment_are_labelled_a_single_dot_is_labelled_once() {
        let graph = layout(&[ev("imp-9", 10.0), ev("imp-9", 30.0), ev("imp-3", 50.0)]);
        let spanned = graph
            .items
            .iter()
            .find(|item| item.id == "imp-9")
            .expect("item")
            .clone();
        let mut app = swimlane_app(&graph, None);

        let labels = lane_labels(&mut app, "imp-9");
        assert_eq!(labels.len(), 2);
        let start = labels.iter().find(|(label, _)| label.start).expect("start");
        let tip = labels.iter().find(|(label, _)| !label.start).expect("tip");
        assert_eq!(
            start.1.left,
            Val::Px((spanned.start_label_x.expect("start label") - label_px("imp-9")) as f32)
        );
        assert_eq!(tip.1.left, Val::Px(spanned.label_x as f32));

        let single = lane_labels(&mut app, "imp-3");
        assert_eq!(single.len(), 1);
        assert!(!single[0].0.start);
    }

    // -- swimlane: empty and loose -------------------------------------------

    // Ported from `swimlane/tests/graph.rs::an_empty_window_lays_out_an_empty_graph`,
    // `a_window_with_nothing_in_it_says_so_instead_of_drawing_an_empty_frame` and
    // `the_empty_graph_is_one_axis_row_tall`.
    #[test]
    fn an_empty_window_draws_only_the_empty_state() {
        let graph = layout(&[]);
        assert!(graph.is_empty());
        assert_eq!((graph.width, graph.height), (WIDTH, 32), "model precondition");

        let mut app = swimlane_app(&graph, None);
        let root = root_node(&mut app);
        assert_eq!(root.width, Val::Px(WIDTH as f32));
        assert_eq!(root.height, Val::Px(graph.height as f32));
        assert_eq!(count::<TraceLane>(&mut app), 0);
        assert_eq!(count::<TraceSpan>(&mut app), 0);
        assert_eq!(count::<TraceDot>(&mut app), 0);
        assert!(texts(&mut app).contains(&EMPTY_TEXT.to_owned()));
    }

    // Ported from `swimlane/tests/graph.rs::events_with_no_work_item_collect_on_their_own_row_not_as_a_branch`.
    #[test]
    fn events_with_no_work_item_collect_on_a_strip_row_not_a_lane() {
        let graph = layout(&[loose_ev(EventKind::Commit, 5.0), ev("imp-1", 5.0)]);
        assert_eq!(graph.items.len(), 1, "model precondition");
        assert_eq!(graph.strip.len(), 1, "model precondition");

        let mut app = swimlane_app(&graph, None);
        assert_eq!(count::<TraceLane>(&mut app), 1);
        let strips = strip_rows(&mut app);
        assert_eq!(strips.len(), 1);
        assert_eq!((strips[0].kind, strips[0].count), (EventKind::Commit, 1));
    }

    // -- swimlane: dots and tones --------------------------------------------

    // Ported from `swimlane/tests/graph.rs::events_closer_than_a_dot_merge_into_one_keeping_every_title_and_the_severest_tone`.
    #[test]
    fn events_closer_than_a_dot_merge_into_one_burst_glyph() {
        let graph = layout(&[
            Event {
                kind: EventKind::ItemStatus,
                ..ev("imp-1", 0.0).status("blocked").summary("failed")
            },
            ev("imp-1", 0.0).summary("retrying"),
            ev("imp-1", 90.0),
        ]);
        assert_eq!(graph.items[0].dots.len(), 2, "model precondition");

        let mut app = swimlane_app(&graph, None);
        let dots = dots(&mut app, "imp-1");
        assert_eq!(dots.len(), 2);
        let burst = dots
            .iter()
            .find(|(dot, _, _)| dot.count == 2)
            .expect("burst");
        assert_eq!(burst.0.tone, Tone::Red);
        assert_eq!(burst.1.width, Val::Px(CLUSTER_PX));
        assert_eq!(burst.2, BevyTheme::default().error);
    }

    // Ported from `swimlane/tests/graph.rs::a_status_colours_the_line_and_only_done_lands`.
    #[test]
    fn a_status_colours_the_span_and_only_done_lands() {
        let theme = BevyTheme::default();
        let cases = [
            ("blocked", theme.error, false),
            ("done", theme.success, true),
            ("review", theme.warning, false),
        ];
        for (status, tint, done) in cases {
            let graph = layout(&[ev("imp-1", 0.0).status(status)]);
            let mut app = swimlane_app(&graph, None);
            let (_, _, colour) = span_of(&mut app, "imp-1");
            let expected = if done {
                tint.with_alpha(DONE_ALPHA)
            } else {
                tint
            };
            assert_eq!(colour, expected, "status {status}");
        }
    }

    // -- swimlane: selection --------------------------------------------------

    // Ported from `rails/tests.rs::rows_are_buttons_only_when_selectable_exactly_the_selected_one_is_marked`
    // and `swimlane/tests/graph.rs::the_hit_rectangle_runs_label_to_label`.
    #[test]
    fn the_picked_item_gets_a_tinted_band_over_its_hit_rect() {
        let graph = layout(&[ev("imp-1", 0.0), ev("imp-3", 40.0)]);
        let item = graph
            .items
            .iter()
            .find(|item| item.id == "imp-3")
            .expect("item")
            .clone();
        let mut app = swimlane_app(&graph, Some("imp-3"));

        let bands = selection_bands(&mut app);
        assert_eq!(bands.len(), 1);
        let (entity, node, tint) = &bands[0];
        let hit = item.hit_rect();
        assert_eq!(node.left, Val::Px(hit.x as f32));
        assert_eq!(node.top, Val::Px((hit.y - item.y) as f32));
        assert_eq!(node.width, Val::Px(hit.w as f32));
        assert_eq!(node.height, Val::Px(hit.h as f32));
        assert_eq!(*tint, BevyTheme::default().primary.with_alpha(0.14));
        let parent = app
            .world()
            .get::<ChildOf>(*entity)
            .expect("band parent")
            .parent();
        assert_eq!(app.world().get::<TraceLane>(parent).expect("lane").id, "imp-3");
    }

    #[test]
    fn an_unknown_or_absent_selection_highlights_nothing() {
        let graph = layout(&[ev("imp-1", 0.0)]);
        for selected in [None, Some("nope")] {
            let mut app = swimlane_app(&graph, selected);
            assert_eq!(selection_bands(&mut app).len(), 0, "selected {selected:?}");
        }
    }

    // -- swimlane: forks ------------------------------------------------------

    // Ported from `swimlane/tests/graph.rs::a_fork_lane_takes_the_row_under_its_item_and_its_elbows_join_the_line`.
    #[test]
    fn a_fork_lane_takes_the_row_under_its_item_and_carries_its_label() {
        let lane = fork_lane("imp-1", 10.0, 40.0, "omp");
        let tree = tree(
            vec![lane.clone()],
            vec![
                edge(EdgeKind::Fork, &lane, "imp-1", 10.0),
                edge(EdgeKind::Merge, &lane, "imp-1", 40.0),
            ],
        );
        let graph = layout_with(
            &[ev("imp-1", 0.0), ev("imp-1", 40.0), ev("imp-1", 90.0)],
            &tree,
        );
        assert_eq!((graph.lanes, graph.rows), (1, 2), "model precondition");

        let item = graph.items[0].clone();
        let fork = item.forks[0].clone();
        let mut app = swimlane_app(&graph, None);
        let entity = fork_entity(&mut app, &fork.id);
        let container = app.world().get::<Node>(entity).expect("fork").clone();
        assert_eq!(container.top, Val::Px((fork.y - item.y) as f32));

        let line = children_of(&app, entity)
            .into_iter()
            .find(|child| app.world().get::<TraceForkLine>(*child).is_some())
            .expect("fork line");
        let node = app.world().get::<Node>(line).expect("line node").clone();
        assert_eq!(node.left, Val::Px(fork.x0 as f32));
        assert_eq!(node.width, Val::Px((fork.x1 - fork.x0) as f32));
        assert!(texts_under(&app, entity).contains(&fork.label));
    }

    // -- rails: rows, labels and selection ------------------------------------

    // Ported from `rails/tests.rs::one_row_per_event_in_time_order_other_agents_sit_in_their_lane_column`.
    #[test]
    fn rail_rows_keep_event_order_their_stamp_and_their_caption() {
        let events = [re("e1", 0.0, "clara"), re("e2", 10.0, "clara"), re("e3", 30.0, "clara")];
        let graph = rail_graph(&events, &tree(vec![], vec![]));
        let gutter = graph.gutter;
        let view = graph.view(&RailOptions::default());
        let expected = view.canvas.as_ref().expect("rows").rows.clone();

        let mut app = rails_app(view);
        let rows = rail_rows(&mut app);
        let ids: Vec<&str> = rows.iter().map(|(_, row, _)| row.id.as_str()).collect();
        assert_eq!(ids, ["e1", "e2", "e3"]);
        for (row, (_, _, node)) in expected.iter().zip(rows.iter()) {
            assert_eq!(node.left, Val::Px(gutter as f32));
            assert_eq!(node.top, Val::Px(row.y as f32 - ROW_PX as f32 / 2.0));
        }

        let captions = texts(&mut app);
        for id in ["e1", "e2", "e3"] {
            assert!(captions.contains(&format!("{id} said")), "caption of {id}");
        }
        assert!(captions.contains(&expected[0].stamp.clone()));
    }

    // Ported from `rails/tests.rs::first_row_in_a_lane_names_the_lane_only_the_first`.
    #[test]
    fn the_first_row_in_a_lane_names_the_lane_only_the_first() {
        let events = [
            re("e1", 0.0, "clara"),
            re("e2", 10.0, "omp"),
            re("e3", 20.0, "omp"),
            re("e4", 30.0, "clara"),
        ];
        let graph = rail_graph(&events, &tree(vec![collab(10.0, 20.0, "omp")], vec![]));
        let mut app = rails_app(graph.view(&RailOptions::default()));
        let rows = rail_rows(&mut app);
        let row = |id: &str| {
            rows.clone()
                .into_iter()
                .find(|(_, row, _)| row.id == id)
                .expect("row")
        };
        assert!(texts_under(&app, row("e2").0).contains(&"[OMP]".to_owned()));
        assert!(!texts_under(&app, row("e3").0).contains(&"[OMP]".to_owned()));
        assert!(!texts_under(&app, row("e1").0).contains(&"[OMP]".to_owned()));
        assert!(!texts_under(&app, row("e4").0).contains(&"[OMP]".to_owned()));
    }

    // Ported from `rails/tests.rs::rows_are_buttons_only_when_selectable_exactly_the_selected_one_is_marked`.
    #[test]
    fn exactly_the_selected_row_is_highlighted() {
        let events = [re("e1", 0.0, "clara"), re("e2", 10.0, "clara")];
        let graph = rail_graph(&events, &tree(vec![], vec![]));

        let plain = graph.view(&RailOptions {
            dom_id: "st".to_owned(),
            heading: false,
            ..RailOptions::default()
        });
        assert_eq!(plain.heading, None);
        let mut app = rails_app(plain);
        assert_eq!(count::<TraceRailRow>(&mut app), 2);
        assert_eq!(selected_rail_rows(&mut app).len(), 0);

        let mut app = rails_app(view_selecting(&graph, Some("e2")));
        let selected = selected_rail_rows(&mut app);
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].id, "e2");
    }

    // Ported from `rails/tests.rs::a_marker_row_is_a_button_once_interactive_but_is_never_selected`.
    #[test]
    fn a_lane_marker_is_never_highlighted() {
        let tree = tree(
            vec![lane_of("b", 10.0, 20.0, None, LaneKind::Branch, "x")],
            vec![],
        );
        let graph = rail_graph(&[re("e1", 0.0, "clara")], &tree);
        let mut app = rails_app(view_selecting(&graph, Some("lane:b")));

        let marker = rail_rows(&mut app)
            .into_iter()
            .find(|(_, row, _)| row.marker)
            .expect("marker row");
        assert_eq!(marker.1.id, "lane:b");
        assert_eq!(selected_rail_rows(&mut app).len(), 0);
    }

    // Ported from `rails/tests.rs::an_item_with_an_owner_and_no_events_is_still_a_graph_with_no_rows`.
    #[test]
    fn an_empty_rail_says_nothing_recorded_yet() {
        let graph = vertical(
            &[],
            &ItemRef {
                id: "imp-16".to_owned(),
                status: Some("open".to_owned()),
            },
            &tree(vec![], vec![]),
        );
        let view = graph.view(&RailOptions::default());
        assert_eq!(view.empty, Some(EMPTY_TEXT));
        assert!(view.canvas.is_none());
        assert_eq!(view.heading.as_ref().expect("heading").meta, None);

        let mut app = rails_app(view);
        assert_eq!(count::<TraceRailRow>(&mut app), 0);
        let found = texts(&mut app);
        assert!(found.contains(&"session tree".to_owned()));
        assert!(found.contains(&EMPTY_TEXT.to_owned()));
    }

    // Ported from `rails/tests.rs::select_node_toggles_ignores_unknown_ids_and_switches`.
    #[test]
    fn select_node_toggles_ignores_unknown_ids_and_switches() {
        let events = [re("e1", 0.0, "clara"), re("e2", 10.0, "clara")];
        let tree = tree(
            vec![lane_of("b", 5.0, 6.0, None, LaneKind::Branch, "x")],
            vec![],
        );
        let graph = rail_graph(&events, &tree);
        let cases: [(Option<&str>, &str, Option<&str>); 6] = [
            (None, "e1", Some("e1")),
            (Some("e1"), "e1", None),
            (Some("e1"), "e2", Some("e2")),
            (Some("e1"), "nope", Some("e1")),
            (None, "nope", None),
            (None, "lane:b", None),
        ];
        for (selected, id, expected) in cases {
            let next = graph.select_node(selected, id);
            assert_eq!(next.as_deref(), expected, "select {id} over {selected:?}");

            let mut app = rails_app(view_selecting(&graph, next.as_deref()));
            let highlighted: Vec<String> = selected_rail_rows(&mut app)
                .into_iter()
                .map(|row| row.id)
                .collect();
            let want: Vec<String> = expected.map(str::to_owned).into_iter().collect();
            assert_eq!(highlighted, want);
        }
    }
}
