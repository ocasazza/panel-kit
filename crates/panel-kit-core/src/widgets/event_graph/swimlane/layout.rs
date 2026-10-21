//! The layout: events, window and width in; rows, dots, forks, strip and axis out.
//!
//! Every work item is a branch from its first event to its last, a dot per
//! event and its id at the tip, the way a ref sits at the tip of a git branch.
//! Items that are never open together share a row, rows run newest first, and
//! an item with fork lanes keeps the rows under its own for them.
//!
//! Pure: the same events, window and width always lay out the same graph.

use std::collections::HashMap;

use serde::Deserialize;
use serde::Serialize;

use super::super::model::{Edge, EdgeKind, Event, EventKind, Lane, LaneKind, Micros, SessionTree};
use super::super::query::{tone_for_kind, tone_for_status, Tone};
use super::dot::{cluster, Dot};
use super::metrics::{
    clearance, label_px, label_px_chars, longest_label_reserve, right_reserve, truncate_label,
    truncated_chars, AXIS, FORK_NODE_PX, LABEL_GAP, PAD_LEFT, PAD_RIGHT, ROW, START_LABEL_PX,
};
use super::num::{fmt1, round_half_up};
use super::pack::FirstFit;
use super::strip::{strip, LooseOpen, LooseRow, StripRow};
use super::warp::{Gap, Params, Tick, Warp};

const MICROS_PER_SEC: i64 = 1_000_000;

/// The window the axis covers: its two ends become zero-weight points of the
/// warp, so idle time before the first event shows as a hatched gap.
///
/// Fed from the query layer's resolved window; the clock is never read here.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Range {
    /// Window start, microseconds.
    pub since: Micros,
    /// Window end, microseconds: the window's `until`, else the clock's now.
    pub now: Micros,
}

impl Range {
    /// A window from `since` to `now`, both in microseconds.
    pub fn new(since: Micros, now: Micros) -> Self {
        Range { since, now }
    }

    /// The window of a resolved query interval, all in epoch seconds: `until`
    /// when the window is closed, else `now`.
    pub fn from_secs(since: i64, until: Option<i64>, now: i64) -> Self {
        Range {
            since: since.saturating_mul(MICROS_PER_SEC),
            now: until.unwrap_or(now).saturating_mul(MICROS_PER_SEC),
        }
    }
}

/// The width to lay the graph out at, from a measured container: its client
/// width less its horizontal padding, floored. `0` when that is not positive,
/// which a host reports as nothing so a hidden container keeps the width the
/// graph already has.
pub fn content_width(client_width: f64, padding_left: f64, padding_right: f64) -> u32 {
    let width = (client_width - padding_left - padding_right).floor();
    if width > 0.0 {
        width as u32
    } else {
        0
    }
}

/// What [`Graph::layout`] reads besides the events.
#[derive(Clone, Copy, Debug)]
pub struct Options<'a> {
    /// Container content width in px.
    pub width: u32,
    /// The window the axis covers.
    pub range: Range,
    /// Item rows drawn: the newest `limit`; `None` draws every item.
    pub limit: Option<usize>,
    /// Further events with no item, besides item-less ones among the events.
    pub loose: &'a [Event],
    /// Which no-item strip rows are expanded to dots.
    pub loose_open: LooseOpen<'a>,
    /// Fork lanes and edges drawn under their items; `None` draws none.
    pub tree: Option<&'a SessionTree>,
    /// Current status per item id, winning over the events' own.
    pub statuses: Option<&'a HashMap<String, String>>,
}

impl<'a> Options<'a> {
    /// Options for `width` px over `range`: no cap, no extra loose events, all
    /// strip rows closed, no forks.
    pub fn new(width: u32, range: Range) -> Self {
        Options {
            width,
            range,
            limit: None,
            loose: &[],
            loose_open: LooseOpen::None,
            tree: None,
            statuses: None,
        }
    }

    /// Caps the item rows drawn at the newest `limit`.
    pub fn limit(mut self, limit: Option<usize>) -> Self {
        self.limit = limit;
        self
    }

    /// Adds events with no item.
    pub fn loose(mut self, loose: &'a [Event]) -> Self {
        self.loose = loose;
        self
    }

    /// Expands strip rows: `true` all, `false` none, or a list of kinds.
    pub fn loose_open(mut self, open: impl Into<LooseOpen<'a>>) -> Self {
        self.loose_open = open.into();
        self
    }

    /// Draws fork lanes and edges from `tree`.
    pub fn tree(mut self, tree: &'a SessionTree) -> Self {
        self.tree = Some(tree);
        self
    }

    /// Takes item statuses from the caller, by item id.
    pub fn statuses(mut self, statuses: &'a HashMap<String, String>) -> Self {
        self.statuses = Some(statuses);
        self
    }
}

/// A rectangle in graph px.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Rect {
    /// Left edge.
    pub x: f64,
    /// Top edge.
    pub y: f64,
    /// Width.
    pub w: f64,
    /// Height.
    pub h: f64,
}

/// A fork lane under its item: a line, and a tip node past its end.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Fork {
    /// The lane's id.
    pub id: String,
    /// What the lane represents.
    pub kind: LaneKind,
    /// Caption: the label cut to 15 characters and `…` when over 16.
    pub label: String,
    /// Tooltip: the kind and the full label, `collab omp`.
    pub title: String,
    /// `Some(true)` survived, `Some(false)` is dead, `None` undecided.
    pub chosen: Option<bool>,
    /// Whether the lane lost (`chosen == Some(false)`): drawn at 35% opacity.
    pub dead: bool,
    /// Tone of the tip node, by lane kind.
    pub tone: Tone,
    /// Centre line of the lane.
    pub y: f64,
    /// Left end of the lane's line.
    pub x0: f64,
    /// Right end of the lane's line, never left of `x0`.
    pub x1: f64,
}

impl Fork {
    /// Left of the tip node's box: its 7px dot is centred on the line's end.
    pub fn node_left(&self) -> f64 {
        round_half_up(self.x1 - 3.5, 1)
    }
}

/// A one-pixel elbow joining a fork lane to its item's line.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct EdgePath {
    /// Fork leaves the item's line; merge climbs back to it.
    pub kind: EdgeKind,
    /// Whether the lane lost.
    pub dead: bool,
    /// SVG path, numbers to one decimal: `M12.0 7.0V21.0H3.0`.
    pub d: String,
}

/// One work item drawn as a branch.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Item {
    /// The item id.
    pub id: String,
    /// Newest title.
    pub title: Option<String>,
    /// Status the line is coloured by.
    pub status: Option<String>,
    /// Tone of `status`.
    pub tone: Tone,
    /// Whether the item landed: status `done` or a merged pull request.
    pub done: bool,
    /// Row rank, 0 at the top; newest row first.
    pub lane: usize,
    /// Centre line of the item's row.
    pub y: f64,
    /// Left end of the line: the first dot.
    pub x0: f64,
    /// Right end of the line: the last dot.
    pub x1: f64,
    /// Where the tip label starts.
    pub label_x: f64,
    /// Where the start label ends, when the line is 120px or longer.
    pub start_label_x: Option<f64>,
    /// Instant of the newest event.
    pub last: Micros,
    /// The item's events, bursts merged, left to right.
    pub dots: Vec<Dot>,
    /// Fork lanes in the rows under this one.
    pub forks: Vec<Fork>,
    /// Elbows joining the forks to the line.
    pub edges: Vec<EdgePath>,
}

impl Item {
    /// Left end of what is drawn: the start label's left end, else the line's.
    pub fn left_edge(&self) -> f64 {
        match self.start_label_x {
            Some(at) => at - label_px(&self.id),
            None => self.x0,
        }
    }

    /// The transparent hit rectangle over the item: label to label, 12px tall.
    pub fn hit_rect(&self) -> Rect {
        let left_edge = self.left_edge();
        Rect {
            x: left_edge - 4.0,
            y: self.y - 6.0,
            w: self.label_x - left_edge + label_px(&self.id) + 8.0,
            h: 12.0,
        }
    }

    /// Tooltip: id, title and status, empty parts dropped, joined with ` · `.
    pub fn tooltip(&self) -> String {
        let parts = [
            Some(self.id.as_str()),
            self.title.as_deref(),
            self.status.as_deref(),
        ];
        parts
            .into_iter()
            .flatten()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(" · ")
    }
}

/// The laid-out graph: everything a painter needs, geometry resolved.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Graph {
    /// Width the graph was laid out at.
    pub width: u32,
    /// Total height: the axis line plus its label band.
    pub height: i64,
    /// Packed row ranks; not pixel rows.
    pub lanes: usize,
    /// Pixel rows of 14px, fork rows included.
    pub rows: usize,
    /// Items before the cap.
    pub total_items: usize,
    /// Items the cap left out.
    pub hidden: usize,
    /// Items drawn, ordered by where their row space begins.
    pub items: Vec<Item>,
    /// One strip row per kind of item-less event, open or not.
    pub strip: Vec<StripRow>,
    /// The open strip rows.
    pub loose: Vec<LooseRow>,
    /// Centre line of a strip row, `Some(7.0)` when there is a strip.
    pub loose_y: Option<f64>,
    /// Grid lines.
    pub ticks: Vec<Tick>,
    /// Hatched idle stretches.
    pub gaps: Vec<Gap>,
    /// Y of the axis line.
    pub axis_y: f64,
}

/// An item's events, found but not yet drawn.
struct Branch<'a> {
    id: &'a str,
    /// Sorted by time, ties in input order.
    run: &'a [&'a Event],
    last: Micros,
}

/// An item with the extent its row needs, before rows are assigned.
struct Placed {
    item: Item,
    left: f64,
    right: f64,
}

impl Graph {
    /// Lays out `events` across `opts.width` px over `opts.range`.
    ///
    /// Events with an item become branches; the rest, with `opts.loose`, the
    /// no-item strip. The axis is [`Warp`]ed over every event and both window
    /// ends, so a burst gets room and an idle week 24px.
    pub fn layout<'a>(events: impl IntoIterator<Item = &'a Event>, opts: &Options<'_>) -> Graph {
        let no_tree = SessionTree::default();
        let tree = opts.tree.unwrap_or(&no_tree);

        let mut with_item: Vec<&Event> = Vec::new();
        let mut loose: Vec<&Event> = Vec::new();
        for event in events {
            if event.item_id.is_some() {
                with_item.push(event);
            } else {
                loose.push(event);
            }
        }
        loose.extend(opts.loose);

        let params = Params {
            since: Some(opts.range.since),
            now: Some(opts.range.now),
            left: PAD_LEFT + longest_label_reserve(),
            right: PAD_RIGHT + right_reserve(&tree.lanes),
            ..Params::default()
        };
        let warp = Warp::scale(
            with_item.iter().chain(&loose).map(|event| event.at),
            f64::from(opts.width),
            &params,
        );

        let (strip_rows, loose_rows) = strip(&loose, |event| warp.x(event.at), opts.loose_open);
        let top = strip_rows.len() as f64 * ROW;

        with_item.sort_by(|a, b| a.item_id.cmp(&b.item_id).then(a.at.cmp(&b.at)));
        let mut branches: Vec<Branch<'_>> = with_item
            .chunk_by(|a, b| a.item_id == b.item_id)
            .filter_map(|run| {
                let last = run.last()?;
                Some(Branch {
                    id: last.item_id.as_deref()?,
                    run,
                    last: last.at,
                })
            })
            .collect();
        let total_items = branches.len();
        branches.sort_by(|a, b| b.last.cmp(&a.last).then_with(|| b.id.cmp(a.id)));
        if let Some(limit) = opts.limit {
            branches.truncate(limit);
        }

        let mut forks_of: HashMap<&str, Vec<&Lane>> = HashMap::new();
        for lane in &tree.lanes {
            forks_of
                .entry(lane.item_id.as_str())
                .or_default()
                .push(lane);
        }
        let mut edges_of: HashMap<&str, Vec<&Edge>> = HashMap::new();
        for edge in &tree.edges {
            edges_of
                .entry(edge.item_id.as_str())
                .or_default()
                .push(edge);
        }

        let mut placed: Vec<Placed> = branches
            .iter()
            .filter_map(|branch| {
                let lanes = forks_of.get(branch.id).map_or(&[][..], Vec::as_slice);
                place(branch, &warp, lanes, opts.statuses)
            })
            .collect();
        let hidden = total_items - placed.len();

        placed.sort_by(|a, b| {
            a.left
                .total_cmp(&b.left)
                .then_with(|| a.item.id.cmp(&b.item.id))
        });
        let lanes = pack(&mut placed);
        newest_first(&mut placed, lanes);
        let rows = stack(&mut placed, lanes, top, &warp, &forks_of, &edges_of);

        let axis_y = top + rows as f64 * ROW + 4.0;
        Graph {
            width: opts.width,
            height: (axis_y + AXIS).round() as i64,
            lanes,
            rows,
            total_items,
            hidden,
            items: placed.into_iter().map(|p| p.item).collect(),
            loose_y: (!strip_rows.is_empty()).then_some(ROW / 2.0),
            strip: strip_rows,
            loose: loose_rows,
            ticks: warp.ticks,
            gaps: warp.gaps,
            axis_y,
        }
    }

    /// `{items} items in {lanes} rows`: the SVG's accessible name.
    pub fn aria_label(&self) -> String {
        format!("{} items in {} rows", self.items.len(), self.lanes)
    }

    /// Whether nothing at all is drawn: no items, no loose rows, no strip.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty() && self.loose.is_empty() && self.strip.is_empty()
    }
}

/// What an item is, read off its events.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Summary {
    /// Newest status the events (or the caller) name.
    pub status: Option<String>,
    /// Newest title, scanning from the newest event; an empty title counts.
    pub title: Option<String>,
    /// Tone of `status`; hollow when it is absent or unknown.
    pub tone: Tone,
    /// Whether the item landed: status `done` or any `pr_merged` event.
    pub done: bool,
    /// Instant of the newest event.
    pub last: Micros,
}

/// Facts of one item from its `events`, sorted by time. `status_override`
/// is the caller's current status and wins; else the newest event that
/// carries a status names it. `None` for no events.
pub fn summarize(events: &[&Event], status_override: Option<&str>) -> Option<Summary> {
    let latest = events.last()?;
    let status = status_override.or_else(|| {
        events
            .iter()
            .rev()
            .find_map(|event| event.detail.status.as_deref())
    });
    Some(Summary {
        status: status.map(str::to_string),
        title: events
            .iter()
            .rev()
            .find_map(|event| event.detail.title.as_deref())
            .map(str::to_string),
        tone: tone_for_status(status),
        done: status == Some("done") || events.iter().any(|e| e.kind == EventKind::PrMerged),
        last: latest.at,
    })
}

fn place(
    branch: &Branch<'_>,
    warp: &Warp,
    lanes: &[&Lane],
    statuses: Option<&HashMap<String, String>>,
) -> Option<Placed> {
    let dots = cluster(branch.run.iter().map(|event| (*event, warp.x(event.at))));
    let (first, last_dot) = (dots.first()?, dots.last()?);
    let (x0, x1) = (first.x, last_dot.x);
    let label_x = x1 + last_dot.clearance();
    let start_label_x = (x1 - x0 >= START_LABEL_PX).then(|| x0 - first.clearance());
    let override_status = statuses
        .and_then(|by_id| by_id.get(branch.id))
        .map(String::as_str);
    let summary = summarize(branch.run, override_status)?;

    let tip = label_x + label_px(branch.id);
    let right = lanes.iter().fold(tip, |reach, lane| {
        let end = warp.x(lane.to).max(warp.x(lane.from));
        reach.max(end + FORK_NODE_PX + label_px_chars(truncated_chars(&lane.label)))
    });
    let left = match start_label_x {
        None => x0 - clearance(first.count),
        Some(at) => at - label_px(branch.id) - LABEL_GAP,
    };

    Some(Placed {
        item: Item {
            id: branch.id.to_string(),
            title: summary.title,
            status: summary.status,
            tone: summary.tone,
            done: summary.done,
            lane: 0,
            y: 0.0,
            x0,
            x1,
            label_x,
            start_label_x,
            last: branch.last,
            dots,
            forks: Vec::new(),
            edges: Vec::new(),
        },
        left,
        right,
    })
}

/// First fit over items sorted by where they begin. An item's interval ends
/// where its reach ends, so neither a tip label nor a fork's tip node can sit
/// under the next item in its row. Returns the rows used.
fn pack(placed: &mut [Placed]) -> usize {
    let mut fit = FirstFit::new(placed.len());
    for p in placed.iter_mut() {
        p.item.lane = fit.place(p.left, p.right + LABEL_GAP);
    }
    fit.lanes()
}

/// Rows run from the one touched most recently down, so the top of the graph
/// is where things are happening now. Equal times keep the lower row first.
fn newest_first(placed: &mut [Placed], lanes: usize) {
    let mut newest = vec![Micros::MIN; lanes];
    for p in placed.iter() {
        newest[p.item.lane] = newest[p.item.lane].max(p.item.last);
    }
    let mut order: Vec<usize> = (0..lanes).collect();
    order.sort_by(|a, b| newest[*b].cmp(&newest[*a]));
    let mut rank = vec![0; lanes];
    for (r, lane) in order.into_iter().enumerate() {
        rank[lane] = r;
    }
    for p in placed.iter_mut() {
        p.item.lane = rank[p.item.lane];
    }
}

/// Gives every item its y. An item with forks keeps the rows under its own for
/// its child lanes, so a row is as tall as the most forks any item in it has.
/// Returns the pixel rows used.
fn stack(
    placed: &mut [Placed],
    lanes: usize,
    top: f64,
    warp: &Warp,
    forks_of: &HashMap<&str, Vec<&Lane>>,
    edges_of: &HashMap<&str, Vec<&Edge>>,
) -> usize {
    let fork_lanes = |id: &str| forks_of.get(id).map_or(&[][..], Vec::as_slice);

    let mut extras = vec![0usize; lanes];
    for p in placed.iter() {
        let slot = &mut extras[p.item.lane];
        *slot = (*slot).max(fork_lanes(&p.item.id).len());
    }
    let mut offsets = Vec::with_capacity(lanes);
    let mut rows = 0usize;
    for extra in &extras {
        offsets.push(rows);
        rows += 1 + extra;
    }

    for p in placed.iter_mut() {
        let item = &mut p.item;
        item.y = top + offsets[item.lane] as f64 * ROW + ROW / 2.0;
        item.forks = forks(fork_lanes(&item.id), item.y, warp);
        let edges = edges_of
            .get(item.id.as_str())
            .map_or(&[][..], Vec::as_slice);
        item.edges = edge_paths(edges, &item.forks, item.y, warp);
    }
    rows
}

fn forks(lanes: &[&Lane], item_y: f64, warp: &Warp) -> Vec<Fork> {
    lanes
        .iter()
        .enumerate()
        .map(|(i, lane)| {
            let x0 = warp.x(lane.from);
            Fork {
                id: lane.id.clone(),
                kind: lane.kind,
                label: truncate_label(&lane.label).into_owned(),
                title: format!("{} {}", lane.kind.as_str(), lane.label),
                chosen: lane.chosen,
                dead: lane.chosen == Some(false),
                tone: tone_for_kind(lane.kind.as_str()),
                y: item_y + (i + 1) as f64 * ROW,
                x0,
                x1: warp.x(lane.to).max(x0),
            }
        })
        .collect()
}

/// One-pixel elbows: a fork drops from the item's line to its lane; a merge
/// runs along the lane's row to where it rejoins and climbs back.
fn edge_paths(edges: &[&Edge], forks: &[Fork], item_y: f64, warp: &Warp) -> Vec<EdgePath> {
    edges
        .iter()
        .filter_map(|edge| {
            // Of two forks sharing an id the later one is the lane.
            let fork = forks.iter().rev().find(|fork| fork.id == edge.lane)?;
            let xe = warp.x(edge.at);
            let d = match edge.kind {
                EdgeKind::Fork => {
                    format!(
                        "M{} {}V{}H{}",
                        fmt1(xe),
                        fmt1(item_y),
                        fmt1(fork.y),
                        fmt1(fork.x0)
                    )
                }
                EdgeKind::Merge => {
                    format!(
                        "M{} {}H{}V{}",
                        fmt1(fork.x1),
                        fmt1(fork.y),
                        fmt1(xe),
                        fmt1(item_y)
                    )
                }
            };
            Some(EdgePath {
                kind: edge.kind,
                dead: fork.dead,
                d,
            })
        })
        .collect()
}
