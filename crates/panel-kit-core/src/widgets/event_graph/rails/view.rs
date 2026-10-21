//! Render model of the rail: what a painter emits, with every decision made.

use serde::Serialize;

use super::super::query::Tone;
use super::super::time;
use super::chip_text;
use super::EdgePath;
use super::Fork;
use super::Row;
use super::RowKind;
use super::VerticalGraph;

/// Caption shown instead of rows when there are none.
pub const EMPTY_TEXT: &str = "nothing recorded yet";

/// Presentation options of one rail.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RailOptions {
    /// Heading text and accessible label.
    pub title: String,
    /// Draw the heading.
    pub heading: bool,
    /// Element id prefix; row ids are `<dom_id>-row-<row id>`.
    pub dom_id: String,
    /// Selected event id.
    pub selected: Option<String>,
    /// Rows accept clicks; without it every row is static.
    pub on_select: bool,
    /// Draw a 1px rule under each row.
    pub separators: bool,
}

impl Default for RailOptions {
    fn default() -> Self {
        RailOptions {
            title: "session tree".to_owned(),
            heading: true,
            dom_id: "activity-graph".to_owned(),
            selected: None,
            on_select: false,
            separators: false,
        }
    }
}

/// How a row responds to the pointer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Hit {
    /// Clickable.
    Button,
    /// Inert: lane markers, or a rail without `on_select`.
    Static,
}

/// Heading above the rail.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RailHeading {
    /// Heading text.
    pub title: String,
    /// `N events · M lanes`; `None` when the rail has no rows.
    pub meta: Option<String>,
}

/// One row, resolved for painting.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RailRow {
    /// Element id, `<dom_id>-row-<row id>`.
    pub dom_id: String,
    /// Event id or `lane:<lane id>`.
    pub row_id: String,
    /// Event kind or lane marker.
    pub kind: RowKind,
    /// Row is a lane-start marker.
    pub lane_marker: bool,
    /// Row lies in a dead lane.
    pub dead: bool,
    /// Pointer behaviour.
    pub hit: Hit,
    /// Marked as the selected row; only a button can be.
    pub selected: bool,
    /// Dot colour.
    pub tone: Tone,
    /// Dot x in px.
    pub x: i32,
    /// Dot y in px.
    pub y: i32,
    /// Tooltip.
    pub title: String,
    /// Time text in the rail's stamp format.
    pub stamp: String,
    /// ISO-8601 instant for the `<time>` element.
    pub iso: String,
    /// `[LANE]` chip, on a lane's first row only.
    pub chip: Option<String>,
    /// One-line caption.
    pub label: String,
    /// 1px rule under the row.
    pub separator: bool,
}

/// Drawing area: rails plus rows.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RailCanvas {
    /// Item-line colour.
    pub tone: Tone,
    /// Item is done: trunk drawn faded.
    pub done: bool,
    /// Width of the rails area in px.
    pub gutter: i32,
    /// Height of the rails area in px.
    pub height: i32,
    /// Trunk x.
    pub trunk_x: i32,
    /// Trunk top.
    pub trunk_y0: i32,
    /// Trunk bottom.
    pub trunk_y1: i32,
    /// Lane lines.
    pub forks: Vec<Fork>,
    /// Elbow paths.
    pub edges: Vec<EdgePath>,
    /// Rows, top to bottom; dots are drawn from these.
    pub rows: Vec<RailRow>,
}

/// The whole rail.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RailView {
    /// Element id prefix.
    pub dom_id: String,
    /// Accessible label.
    pub title: String,
    /// Heading, unless disabled.
    pub heading: Option<RailHeading>,
    /// [`EMPTY_TEXT`] when there are no rows.
    pub empty: Option<&'static str>,
    /// Rails and rows; `None` when there are no rows.
    pub canvas: Option<RailCanvas>,
}

fn rail_row(graph: &VerticalGraph, row: &Row, opts: &RailOptions) -> RailRow {
    let hit = if row.selectable && opts.on_select {
        Hit::Button
    } else {
        Hit::Static
    };
    RailRow {
        dom_id: format!("{}-row-{}", opts.dom_id, row.id),
        row_id: row.id.clone(),
        kind: row.kind,
        lane_marker: row.kind == RowKind::Lane,
        dead: row.dead,
        hit,
        selected: hit == Hit::Button && opts.selected.as_deref() == Some(row.id.as_str()),
        tone: row.tone,
        x: row.x,
        y: row.y,
        title: row.title.clone(),
        stamp: graph.stamp.format(row.at),
        iso: time::format_iso8601(row.at),
        chip: row.lane.as_deref().map(chip_text),
        label: row.label.clone(),
        separator: opts.separators,
    }
}

impl VerticalGraph {
    /// Resolve the rail for painting under `opts`.
    pub fn view(&self, opts: &RailOptions) -> RailView {
        let item = &self.item;
        let canvas = match (item.y0, item.y1) {
            (Some(trunk_y0), Some(trunk_y1)) => Some(RailCanvas {
                tone: item.tone,
                done: item.done,
                gutter: self.gutter,
                height: self.height,
                trunk_x: item.x,
                trunk_y0,
                trunk_y1,
                forks: item.forks.clone(),
                edges: item.edges.clone(),
                rows: item
                    .rows
                    .iter()
                    .map(|row| rail_row(self, row, opts))
                    .collect(),
            }),
            _ => None,
        };
        RailView {
            dom_id: opts.dom_id.clone(),
            title: opts.title.clone(),
            heading: opts.heading.then(|| RailHeading {
                title: opts.title.clone(),
                meta: canvas.as_ref().map(|_| self.header()),
            }),
            empty: canvas.is_none().then_some(EMPTY_TEXT),
            canvas,
        }
    }
}
