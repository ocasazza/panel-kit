//! Vertical rail of one work item: rows top to bottom, a trunk, forked lanes.
//!
//! Entry points:
//! - [`vertical`]: events + [`SessionTree`] to a [`VerticalGraph`] (rows, forks,
//!   elbow paths, gutter, stamp format) with all geometry resolved in pixels.
//! - [`VerticalGraph::view`]: a [`RailView`] with stamps, chips, header text and
//!   the selection state resolved, ready for a painter to emit DOM.
//! - [`VerticalGraph::find_dot`] / [`VerticalGraph::select_node`]: selection by event id.
//! - [`header_text`]: `N events · M lanes`.
//! - [`timeline`]: the flat per-actor lane list (`TimelineLanes`) and its builder.
//! - [`panels`]: session-pane and orchestrator-trace models.

use std::collections::HashMap;
use std::collections::HashSet;

use serde::Serialize;
use serde::Serializer;

use super::model::EdgeKind;
use super::model::Event;
use super::model::EventKind;
use super::model::ItemRef;
use super::model::Lane;
use super::model::LaneKind;
use super::model::Micros;
use super::model::SessionTree;
use super::query::tone_for_status;
use super::query::Tone;
use super::time;

pub mod builder;
pub mod panels;
pub mod timeline;
pub mod view;

#[cfg(test)]
mod tests;

pub use view::Hit;
pub use view::RailCanvas;
pub use view::RailHeading;
pub use view::RailOptions;
pub use view::RailRow;
pub use view::RailView;

/// Row height in px.
pub const ROW_PX: i32 = 22;
/// Column pitch in px.
pub const COL_PX: i32 = 14;
/// Left padding of the trunk in px.
pub const PAD_PX: i32 = 7;

/// X of column `col`; column 0 is the item line.
pub const fn col_x(col: u32) -> i32 {
    PAD_PX + col as i32 * COL_PX
}

/// Time format shared by every row of one rail.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Stamp {
    /// `HH:MM`: all rows fall on one UTC date.
    Time,
    /// `MM-DD HH:MM`: rows span several UTC dates.
    DayTime,
}

impl Stamp {
    /// Format `at` in this stamp format.
    pub fn format(self, at: Micros) -> String {
        match self {
            Stamp::Time => time::format_time(at),
            Stamp::DayTime => time::format_day_time(at),
        }
    }
}

/// What a row stands for: an event kind or a lane-start marker.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowKind {
    /// Row of a recorded event.
    Event(EventKind),
    /// Marker drawn for a lane that has no event of its own.
    Lane,
}

impl RowKind {
    /// Snake-case name (`data-kind`); `lane` for a marker.
    pub fn as_str(self) -> &'static str {
        match self {
            RowKind::Event(kind) => kind.as_str(),
            RowKind::Lane => "lane",
        }
    }
}

impl Serialize for RowKind {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

/// One row of the rail.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Row {
    /// Event id, or `lane:<lane id>` for a marker.
    pub id: String,
    /// Instant, microseconds.
    pub at: Micros,
    /// Event kind or lane marker.
    pub kind: RowKind,
    /// Acting agent.
    pub agent: Option<String>,
    /// Dot colour class.
    pub tone: Tone,
    /// Tooltip.
    pub title: String,
    /// One-line caption.
    pub label: String,
    /// Raw event summary; `None` for markers.
    pub detail: Option<String>,
    /// Events the dot stands for; always 1 on a rail.
    pub count: u32,
    /// 0 is the item line, 1+ a fork-lane column.
    pub col: u32,
    /// `false` only for lane markers.
    pub selectable: bool,
    /// Row lies in a lane whose `chosen` is `Some(false)`.
    pub dead: bool,
    /// Lane label iff this is the first row in that lane's column.
    pub lane: Option<String>,
    /// Dot x in px.
    pub x: i32,
    /// Dot y in px (row centre).
    pub y: i32,
}

impl Row {
    /// Actor chip text for this row: `[LABEL]`, only on a lane's first row.
    pub fn chip(&self) -> Option<String> {
        self.lane.as_deref().map(chip_text)
    }
}

/// `[LABEL]`: uppercase, bracketed.
pub fn chip_text(label: &str) -> String {
    format!("[{}]", label.to_uppercase())
}

/// A lane drawn as a vertical line beside the item line.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Fork {
    /// Lane id.
    pub id: String,
    /// Lane kind.
    pub kind: LaneKind,
    /// Lane label.
    pub label: String,
    /// `<kind> <label>`.
    pub title: String,
    /// Agent owning the lane.
    pub agent: Option<String>,
    /// `Some(true)` survived, `Some(false)` dead, `None` undecided.
    pub chosen: Option<bool>,
    /// `chosen == Some(false)`.
    pub dead: bool,
    /// No merge edge: the line runs to the bottom of the rail.
    pub open: bool,
    /// Id of the lane it forked from.
    pub parent: Option<String>,
    /// Column, 1+.
    pub col: u32,
    /// Line x in px.
    pub x: i32,
    /// Top of the line in px.
    pub y0: i32,
    /// Bottom of the line in px; never above `y0`.
    pub y1: i32,
}

/// Elbow path between a parent rail and a lane rail.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct EdgePath {
    /// Fork or merge.
    pub kind: EdgeKind,
    /// Belongs to a dead lane.
    pub dead: bool,
    /// SVG path, e.g. `M7.0 11.0H21.0`.
    pub d: String,
}

/// The item's line with everything drawn on it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RailItem {
    /// Item id.
    pub id: String,
    /// Title of the newest event that carries one.
    pub title: Option<String>,
    /// Item status, else the last event's status.
    pub status: Option<String>,
    /// Colour of the item line.
    pub tone: Tone,
    /// Status is `done` or a PR merged.
    pub done: bool,
    /// Instant of the newest event.
    pub last: Option<Micros>,
    /// Owner of the trunk.
    pub agent: Option<String>,
    /// Trunk x in px.
    pub x: i32,
    /// Centre of the first row; `None` without rows.
    pub y0: Option<i32>,
    /// Centre of the last row; `None` without rows.
    pub y1: Option<i32>,
    /// Rows, top to bottom.
    pub rows: Vec<Row>,
    /// One per lane, in order of first instant.
    pub forks: Vec<Fork>,
    /// Fork and merge elbows.
    pub edges: Vec<EdgePath>,
}

/// Resolved vertical rail.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct VerticalGraph {
    /// Rows times [`ROW_PX`].
    pub height: i32,
    /// Width reserved left of the rows.
    pub gutter: i32,
    /// Time format shared by all rows.
    pub stamp: Stamp,
    /// The one item drawn.
    pub item: RailItem,
}

impl VerticalGraph {
    /// First selectable row with this id.
    pub fn find_dot(&self, id: &str) -> Option<&Row> {
        self.item
            .rows
            .iter()
            .find(|row| row.selectable && row.id == id)
    }

    /// Selection after clicking `id`: unknown ids are ignored, the selected
    /// id clears, any other id becomes selected.
    pub fn select_node(&self, selected: Option<&str>, id: &str) -> Option<String> {
        match self.find_dot(id) {
            None => selected.map(str::to_owned),
            Some(_) if selected == Some(id) => None,
            Some(row) => Some(row.id.clone()),
        }
    }

    /// Header meta text: selectable rows and lanes, e.g. `7 events · 6 lanes`.
    pub fn header(&self) -> String {
        let events = self.item.rows.iter().filter(|row| row.selectable).count();
        header_text(events, self.item.forks.len())
    }
}

/// `1 event` / `3 events`.
pub fn plural(count: usize, noun: &str) -> String {
    if count == 1 {
        format!("1 {noun}")
    } else {
        format!("{count} {noun}s")
    }
}

/// `N events · M lanes`; the lanes part is omitted when there are none.
pub fn header_text(events: usize, lanes: usize) -> String {
    if lanes > 0 {
        format!("{} · {}", plural(events, "event"), plural(lanes, "lane"))
    } else {
        plural(events, "event")
    }
}

/// Replace every run of ASCII whitespace by one space and trim.
pub(crate) fn one_line(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for word in text
        .split(|c: char| matches!(c, ' ' | '\t'..='\r'))
        .filter(|word| !word.is_empty())
    {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(word);
    }
    out
}

struct Placed<'a> {
    lane: &'a Lane,
    col: u32,
    open: bool,
}

impl Placed<'_> {
    fn covers(&self, col: u32, at: Micros) -> bool {
        self.col == col && self.lane.from <= at && at <= self.lane.to
    }
}

/// Pack lanes leftwards into the first column free over their span.
fn place_lanes<'a>(lanes: &'a [Lane], merges: &HashMap<&str, Micros>) -> Vec<Placed<'a>> {
    let mut sorted: Vec<&Lane> = lanes.iter().collect();
    sorted.sort_by(|a, b| a.from.cmp(&b.from).then_with(|| a.id.cmp(&b.id)));
    let mut columns: Vec<Vec<(Micros, Micros)>> = Vec::new();
    let mut placed = Vec::with_capacity(sorted.len());
    for lane in sorted {
        let merge = merges.get(lane.id.as_str()).copied();
        let span = (lane.from, merge.unwrap_or(lane.to));
        let free = columns
            .iter()
            .position(|spans| !spans.iter().any(|&(f, t)| span.0 <= t && span.1 >= f));
        let slot = free.unwrap_or_else(|| {
            columns.push(Vec::new());
            columns.len() - 1
        });
        columns[slot].push(span);
        placed.push(Placed {
            lane,
            col: slot as u32 + 1,
            open: merge.is_none(),
        });
    }
    placed
}

fn row_title(event: &Event, kind_label: &str) -> String {
    let at = time::format_tooltip(event.at);
    let mut parts: Vec<&str> = vec![&at, kind_label];
    parts.extend(event.agent.as_deref());
    parts.extend(event.summary.as_deref());
    parts.retain(|part| !part.is_empty());
    parts.join(" · ")
}

fn event_row(event: &Event, placed: &[Placed<'_>], owner: Option<&str>) -> Row {
    let kind_label = event.kind.label();
    let label = match event.summary.as_deref().map(one_line) {
        Some(line) if !line.is_empty() => line,
        _ => kind_label.clone(),
    };
    let col = match event.agent.as_deref() {
        Some(agent) if Some(agent) != owner => placed
            .iter()
            .find(|p| {
                p.lane.agent.as_deref() == Some(agent)
                    && p.lane.from <= event.at
                    && event.at <= p.lane.to
            })
            .map_or(0, |p| p.col),
        _ => 0,
    };
    Row {
        id: event.id.clone(),
        at: event.at,
        kind: RowKind::Event(event.kind),
        agent: event.agent.clone(),
        tone: event.tone(),
        title: row_title(event, &kind_label),
        label,
        detail: event.summary.clone(),
        count: 1,
        col,
        selectable: true,
        dead: false,
        lane: None,
        x: 0,
        y: 0,
    }
}

fn marker_row(p: &Placed<'_>) -> Row {
    let text = format!("{} {}", p.lane.kind.as_str(), p.lane.label);
    Row {
        id: format!("lane:{}", p.lane.id),
        at: p.lane.from,
        kind: RowKind::Lane,
        agent: p.lane.agent.clone(),
        tone: Tone::Hollow,
        title: text.clone(),
        label: text,
        detail: None,
        count: 1,
        col: p.col,
        selectable: false,
        dead: false,
        lane: None,
        x: 0,
        y: 0,
    }
}

/// Mark dead rows and name each lane once, on its first non-marker row.
fn decorate(rows: &mut [Row], placed: &[Placed<'_>]) {
    let mut named: HashSet<&str> = HashSet::new();
    for row in rows.iter_mut() {
        let lane = if row.col > 0 {
            placed.iter().find(|p| p.covers(row.col, row.at))
        } else {
            None
        };
        row.dead = lane.is_some_and(|p| p.lane.chosen == Some(false));
        let Some(p) = lane else { continue };
        if !named.insert(p.lane.id.as_str()) {
            continue;
        }
        if row.kind != RowKind::Lane {
            row.lane = Some(p.lane.label.clone());
        }
    }
}

/// Y of the first row at or after `at`, else of the last row; 0 without rows.
fn row_y(rows: &[Row], at: Micros) -> i32 {
    let i = rows.partition_point(|row| row.at < at);
    rows.get(i).or_else(|| rows.last()).map_or(0, |row| row.y)
}

fn xy(x: i32, y: i32) -> String {
    format!("{x}.0 {y}.0")
}

fn edge_paths(forks: &[Fork]) -> Vec<EdgePath> {
    let mut out = Vec::with_capacity(forks.len() * 2);
    for fork in forks {
        let from_x = fork
            .parent
            .as_deref()
            .and_then(|parent| forks.iter().find(|f| f.id == parent))
            .map_or(col_x(0), |f| f.x);
        out.push(EdgePath {
            kind: EdgeKind::Fork,
            dead: fork.dead,
            d: format!("M{}H{}.0", xy(from_x, fork.y0), fork.x),
        });
        if !fork.open {
            out.push(EdgePath {
                kind: EdgeKind::Merge,
                dead: fork.dead,
                d: format!("M{}H{}.0", xy(fork.x, fork.y1), from_x),
            });
        }
    }
    out
}

/// Lay out one item's events and session tree as a vertical rail.
pub fn vertical(events: &[Event], item: &ItemRef, tree: &SessionTree) -> VerticalGraph {
    let owner = tree.agent.as_deref();
    let mut evs: Vec<&Event> = events
        .iter()
        .filter(|e| e.item_id.as_deref() == Some(item.id.as_str()))
        .collect();
    evs.sort_by(|a, b| a.at.cmp(&b.at).then_with(|| a.id.cmp(&b.id)));

    let mut forks_at: HashMap<&str, Micros> = HashMap::new();
    let mut merges: HashMap<&str, Micros> = HashMap::new();
    for edge in &tree.edges {
        let map = match edge.kind {
            EdgeKind::Fork => &mut forks_at,
            EdgeKind::Merge => &mut merges,
        };
        map.insert(edge.lane.as_str(), edge.at);
    }
    let placed = place_lanes(&tree.lanes, &merges);

    let mut rows: Vec<Row> = evs.iter().map(|e| event_row(e, &placed, owner)).collect();
    let markers: Vec<Row> = placed
        .iter()
        .filter(|p| !rows.iter().any(|row| p.covers(row.col, row.at)))
        .map(marker_row)
        .collect();
    rows.extend(markers);
    rows.sort_by(|a, b| a.at.cmp(&b.at).then_with(|| a.id.cmp(&b.id)));
    decorate(&mut rows, &placed);
    for (index, row) in rows.iter_mut().enumerate() {
        row.x = col_x(row.col);
        row.y = index as i32 * ROW_PX + ROW_PX / 2;
    }

    let height = rows.len() as i32 * ROW_PX;
    let bottom = (height - 4).max(0);
    let forks: Vec<Fork> = placed
        .iter()
        .map(|p| {
            let lane = p.lane;
            let y0 = row_y(
                &rows,
                forks_at.get(lane.id.as_str()).copied().unwrap_or(lane.from),
            );
            let y1 = if p.open {
                bottom
            } else {
                merges
                    .get(lane.id.as_str())
                    .map_or(bottom, |&at| row_y(&rows, at))
            };
            Fork {
                id: lane.id.clone(),
                kind: lane.kind,
                label: lane.label.clone(),
                title: format!("{} {}", lane.kind.as_str(), lane.label),
                agent: lane.agent.clone(),
                chosen: lane.chosen,
                dead: lane.chosen == Some(false),
                open: p.open,
                parent: lane.parent.clone(),
                col: p.col,
                x: col_x(p.col),
                y0,
                y1: y1.max(y0),
            }
        })
        .collect();

    let first_day = rows.first().map(|row| time::day_index(row.at));
    let stamp = if rows
        .iter()
        .any(|row| Some(time::day_index(row.at)) != first_day)
    {
        Stamp::DayTime
    } else {
        Stamp::Time
    };
    let max_col = placed.iter().map(|p| p.col).max().unwrap_or(0);

    let latest = evs.last();
    let status = item
        .status
        .clone()
        .or_else(|| latest.and_then(|e| e.detail.status.clone()));
    let title = evs.iter().rev().find_map(|e| e.detail.title.clone());
    let tone = tone_for_status(status.as_deref());
    let done =
        status.as_deref() == Some("done") || evs.iter().any(|e| e.kind == EventKind::PrMerged);
    let edges = edge_paths(&forks);

    VerticalGraph {
        height,
        gutter: col_x(max_col) + PAD_PX,
        stamp,
        item: RailItem {
            id: item.id.clone(),
            title,
            status,
            tone,
            done,
            last: latest.map(|e| e.at),
            agent: tree.agent.clone(),
            x: col_x(0),
            y0: rows.first().map(|row| row.y),
            y1: rows.last().map(|row| row.y),
            rows,
            forks,
            edges,
        },
    }
}
