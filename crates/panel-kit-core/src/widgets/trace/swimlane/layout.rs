//! The layout: events, window and width in; rows, dots, forks, strip and axis out.
//!
//! Every work item is a branch from its first event to its last, a dot per
//! event and its id at the tip, the way a ref sits at the tip of a git branch.
//! Items that are never open together share a row, rows run newest first, and
//! an item with fork lanes keeps the rows under its own for them.
//!
//! Pure: the same events, window and width always lay out the same graph.

use std::borrow::Cow;
use std::collections::HashMap;

use serde::Deserialize;
use serde::Serialize;

use super::super::model::{Edge, EdgeKind, Event, EventKind, Lane, LaneKind, Micros, SessionTree};
use super::super::query::{tone_for_kind, tone_for_status, Tone};
use super::dot::{cluster, Dot};
use super::group::{group, FastMap, Groups};
use super::metrics::{
    clearance, label_px, label_px_chars, longest_label_reserve, right_reserve, truncate_label,
    truncated_chars, AXIS, DOT_MERGE_PX, FORK_NODE_PX, LABEL_GAP, PAD_LEFT, PAD_RIGHT, ROW,
    START_LABEL_PX,
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
    /// Leave the items unbuilt: [`Graph::layout_slice`] lays out their rows
    /// and [`Graph::item`] builds one when a painter needs it.
    pub lazy: bool,
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
            lazy: false,
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

    /// Builds items on demand rather than all at once; see [`Options::lazy`].
    pub fn lazy(mut self, lazy: bool) -> Self {
        self.lazy = lazy;
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

/// Where one item sits: its row in graph px, and the item that draws there.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct RowSpan {
    /// Centre line of the item's row.
    pub y: f64,
    /// Height of the row space the item keeps: its row and its fork rows.
    pub height: f64,
    /// The item, as the index [`Graph::item`] takes: its place among `items`.
    pub item: u32,
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
    /// Items drawn, ordered by where their row space begins. Empty for a
    /// lazy layout, which builds them through [`Graph::item`].
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
    #[serde(skip)]
    spans: Vec<RowSpan>,
    /// The tallest row space of `spans`: how far above a band an item may
    /// start and still reach into it.
    #[serde(skip)]
    reach: f64,
    #[serde(skip)]
    lazy: Option<Lazy>,
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

/// An item as the row packing sees it: how far it reaches, how many fork
/// rows it keeps and when it was last touched.
#[derive(Clone, Copy)]
struct Slot {
    left: f64,
    right: f64,
    last: Micros,
    lane: usize,
    forks: u32,
    /// Where the item came from: its index among the items picked.
    pick: u32,
}

/// What the packing reads of an item's dots: the pixel and size of the first
/// and last one, bursts merged.
struct Ends {
    x0: f64,
    first: u32,
    x1: f64,
    last: u32,
}

/// Where an item's labels and extent fall, from the ends of its line.
struct Reach {
    label_x: f64,
    start_label_x: Option<f64>,
    left: f64,
    right: f64,
}

/// An item's run of events in a lazy graph, and its row.
#[derive(Clone, Debug, PartialEq)]
struct Row {
    start: u32,
    len: u32,
    lane: u32,
    y: f64,
}

/// What a lazy graph keeps to build an item's dots later: the axis, the
/// events of each item as indices into the slice it was laid out from, and
/// the fork lanes and statuses that apply.
#[derive(Clone, Debug, PartialEq)]
struct Lazy {
    events: usize,
    warp: Warp,
    order: Vec<u32>,
    rows: Vec<Row>,
    lanes: Vec<Lane>,
    edges: Vec<Edge>,
    forks_of: FastMap<String, Vec<u32>>,
    edges_of: FastMap<String, Vec<u32>>,
    statuses: FastMap<u32, String>,
}

impl Lazy {
    fn item(&self, events: &[Event], at: usize) -> Item {
        assert_eq!(
            events.len(),
            self.events,
            "a lazy graph builds its items from the slice it was laid out from"
        );
        let row = &self.rows[at];
        let from = row.start as usize;
        let run: Vec<&Event> = self.order[from..from + row.len as usize]
            .iter()
            .map(|&i| &events[i as usize])
            .collect();
        let id = run
            .first()
            .and_then(|event| event.item_id.as_deref())
            .unwrap_or("");
        let pick = |table: &FastMap<String, Vec<u32>>| -> Vec<u32> {
            table.get(id).cloned().unwrap_or_default()
        };
        let lanes: Vec<&Lane> = pick(&self.forks_of)
            .iter()
            .map(|&i| &self.lanes[i as usize])
            .collect();
        let edges: Vec<&Edge> = pick(&self.edges_of)
            .iter()
            .map(|&i| &self.edges[i as usize])
            .collect();
        let branch = Branch {
            id,
            run: &run,
            last: run.last().map_or(0, |event| event.at),
        };
        let status = self.statuses.get(&(at as u32)).map(String::as_str);
        let mut item = place(&branch, &self.warp, &lanes, status)
            .map(|placed| placed.item)
            .expect("a run holds at least one event");
        item.lane = row.lane as usize;
        item.y = row.y;
        item.forks = forks(&lanes, item.y, &self.warp);
        item.edges = edge_paths(&edges, &item.forks, item.y, &self.warp);
        item
    }
}

impl Graph {
    /// Lays out `events` across `opts.width` px over `opts.range`.
    ///
    /// Events with an item become branches; the rest, with `opts.loose`, the
    /// no-item strip. The axis is [`Warp`]ed over every event and both window
    /// ends, so a burst gets room and an idle week 24px. Every item drawn is
    /// built; `opts.lazy` is for [`Graph::layout_slice`].
    pub fn layout<'a>(events: impl IntoIterator<Item = &'a Event>, opts: &Options<'_>) -> Graph {
        let refs: Vec<&Event> = events.into_iter().collect();
        let kept: Vec<u32> = (0..refs.len() as u32).collect();
        build(&|i| refs[i as usize], &kept, opts, None)
    }

    /// [`Graph::layout`] over the events of `events` that `keep` accepts. With
    /// `opts.lazy` no item is built: [`Graph::item`] builds one from the same
    /// `events` when a painter asks for its row.
    pub fn layout_slice(
        events: &[Event],
        keep: impl Fn(&Event) -> bool,
        opts: &Options<'_>,
    ) -> Graph {
        let kept: Vec<u32> = events
            .iter()
            .enumerate()
            .filter(|(_, event)| keep(event))
            .map(|(i, _)| i as u32)
            .collect();
        build(
            &|i| &events[i as usize],
            &kept,
            opts,
            opts.lazy.then_some(events.len()),
        )
    }

    /// Items drawn: `items.len()` for a built graph, the rows of a lazy one.
    pub fn item_count(&self) -> usize {
        self.spans.len()
    }

    /// The rows of every item drawn, by y and then x.
    pub fn row_spans(&self) -> &[RowSpan] {
        &self.spans
    }

    /// The rows of [`Graph::row_spans`] that meet `y0..=y1`, graph px, as a
    /// range of indices: every item whose row space touches the band, plus
    /// at most the few rows above it that a tall fork stack could reach.
    pub fn span(&self, y0: f64, y1: f64) -> std::ops::Range<usize> {
        let top = |span: &RowSpan| span.y - ROW / 2.0;
        let lo = self
            .spans
            .partition_point(|span| top(span) + self.reach <= y0);
        let hi = self.spans.partition_point(|span| top(span) < y1);
        lo..hi.max(lo)
    }

    /// The item at [`RowSpan::item`]. `events` is the slice the graph was
    /// laid out from; a built graph reads its own items and ignores it.
    pub fn item(&self, events: &[Event], item: u32) -> Cow<'_, Item> {
        match &self.lazy {
            None => Cow::Borrowed(&self.items[item as usize]),
            Some(lazy) => Cow::Owned(lazy.item(events, item as usize)),
        }
    }

    /// `{items} items in {lanes} rows`: the SVG's accessible name.
    pub fn aria_label(&self) -> String {
        format!("{} items in {} rows", self.item_count(), self.lanes)
    }

    /// Whether nothing at all is drawn: no items, no loose rows, no strip.
    pub fn is_empty(&self) -> bool {
        self.item_count() == 0 && self.loose.is_empty() && self.strip.is_empty()
    }
}

/// Groups `kept`, picks the items to draw, finds their extents, packs them
/// into rows and builds the strip and the axis. `lazy` is the length of the
/// slice `get` reads when the items are to be built on demand.
fn build<'e>(
    get: &impl Fn(u32) -> &'e Event,
    kept: &[u32],
    opts: &Options<'_>,
    lazy: Option<usize>,
) -> Graph {
    let no_tree = SessionTree::default();
    let tree = opts.tree.unwrap_or(&no_tree);

    let mut with_item: Vec<u32> = Vec::with_capacity(kept.len());
    let mut loose: Vec<&Event> = Vec::new();
    for &i in kept {
        let event = get(i);
        if event.item_id.is_some() {
            with_item.push(i);
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
    // Input order, so a host's oldest-first events reach the warp sorted.
    let warp = Warp::scale(
        kept.iter()
            .map(|&i| get(i).at)
            .chain(opts.loose.iter().map(|event| event.at)),
        f64::from(opts.width),
        &params,
    );

    let (strip_rows, loose_rows) = strip(&loose, |event| warp.x(event.at), opts.loose_open);
    let top = strip_rows.len() as f64 * ROW;

    let groups = group(get, &with_item);
    let total_items = groups.items();
    let picked = pick(&groups, get, opts.limit);
    let (order, runs) = runs(get, &with_item, &groups, &picked);
    drop(with_item);

    let forks_of = by_item(&tree.lanes, |lane| lane.item_id.as_str());
    let edges_of = by_item(&tree.edges, |edge| edge.item_id.as_str());
    let lanes_of = |id: &str| forks_of.get(id).map_or(&[][..], Vec::as_slice);
    let edges_at = |id: &str| edges_of.get(id).map_or(&[][..], Vec::as_slice);
    let id_of = |item: u32| {
        get(groups.first[item as usize])
            .item_id
            .as_deref()
            .unwrap_or("")
    };
    let statuses_of = |id: &str| {
        opts.statuses
            .and_then(|by_id| by_id.get(id))
            .map(String::as_str)
    };
    let run_of = |p: usize| &order[runs[p].0 as usize..(runs[p].0 + runs[p].1) as usize];

    // Built graphs measure each item by drawing it; lazy ones from its ends.
    let mut placed: Vec<Placed> = Vec::new();
    let mut slots: Vec<Slot> = Vec::with_capacity(picked.len());
    let mut statuses: FastMap<u32, String> = FastMap::default();
    if lazy.is_none() {
        for (p, &item) in picked.iter().enumerate() {
            let run: Vec<&Event> = run_of(p).iter().map(|&i| get(i)).collect();
            let id = id_of(item);
            let branch = Branch {
                id,
                run: &run,
                last: groups.last[item as usize],
            };
            if let Some(one) = place(&branch, &warp, lanes_of(id), statuses_of(id)) {
                placed.push(one);
            }
        }
        placed.sort_by(|a, b| {
            a.left
                .total_cmp(&b.left)
                .then_with(|| a.item.id.cmp(&b.item.id))
        });
        slots.extend(placed.iter().enumerate().map(|(p, one)| Slot {
            left: one.left,
            right: one.right,
            last: one.item.last,
            lane: 0,
            forks: lanes_of(&one.item.id).len() as u32,
            pick: p as u32,
        }));
    } else {
        let mut scratch: Vec<(f64, Micros)> = Vec::new();
        for (p, &item) in picked.iter().enumerate() {
            let id = id_of(item);
            let ends = ends_of(run_of(p), get, &warp, &mut scratch);
            let reach = reach(&ends, id, lanes_of(id), &warp);
            slots.push(Slot {
                left: reach.left,
                right: reach.right,
                last: groups.last[item as usize],
                lane: 0,
                forks: lanes_of(id).len() as u32,
                pick: p as u32,
            });
        }
        slots.sort_by(|a, b| {
            a.left
                .total_cmp(&b.left)
                .then_with(|| id_of(picked[a.pick as usize]).cmp(id_of(picked[b.pick as usize])))
        });
    }
    let hidden = total_items - slots.len();

    let lanes = pack(&mut slots);
    newest_first(&mut slots, lanes);
    let (offsets, rows) = row_offsets(&slots, lanes);
    let y_of = |slot: &Slot| top + offsets[slot.lane] as f64 * ROW + ROW / 2.0;
    let spans = row_spans(&slots, lanes, &y_of);
    let reach = spans.iter().map(|span| span.height).fold(0.0, f64::max);

    let mut items: Vec<Item> = Vec::new();
    let mut lazy_rows: Option<Lazy> = None;
    match lazy {
        None => {
            for (one, slot) in placed.iter_mut().zip(&slots) {
                let item = &mut one.item;
                item.lane = slot.lane;
                item.y = y_of(slot);
                item.forks = forks(lanes_of(&item.id), item.y, &warp);
                item.edges = edge_paths(edges_at(&item.id), &item.forks, item.y, &warp);
            }
            items = placed.into_iter().map(|one| one.item).collect();
        }
        Some(events) => {
            let mut table: Vec<Row> = Vec::with_capacity(slots.len());
            for (at, slot) in slots.iter().enumerate() {
                let p = slot.pick as usize;
                table.push(Row {
                    start: runs[p].0,
                    len: runs[p].1,
                    lane: slot.lane as u32,
                    y: y_of(slot),
                });
                if let Some(status) = statuses_of(id_of(picked[p])) {
                    statuses.insert(at as u32, status.to_owned());
                }
            }
            lazy_rows = Some(Lazy {
                events,
                warp: warp.clone(),
                order,
                rows: table,
                lanes: tree.lanes.clone(),
                edges: tree.edges.clone(),
                forks_of: indices(&tree.lanes, |lane| lane.item_id.as_str()),
                edges_of: indices(&tree.edges, |edge| edge.item_id.as_str()),
                statuses,
            });
        }
    }

    let axis_y = top + rows as f64 * ROW + 4.0;
    Graph {
        width: opts.width,
        height: (axis_y + AXIS).round() as i64,
        lanes,
        rows,
        total_items,
        hidden,
        items,
        spans,
        reach,
        lazy: lazy_rows,
        loose_y: (!strip_rows.is_empty()).then_some(ROW / 2.0),
        strip: strip_rows,
        loose: loose_rows,
        ticks: warp.ticks,
        gaps: warp.gaps,
        axis_y,
    }
}

/// The items to draw: all, or the `limit` touched most recently, ties by
/// the greater id first.
fn pick<'e>(groups: &Groups, get: &impl Fn(u32) -> &'e Event, limit: Option<usize>) -> Vec<u32> {
    let mut items: Vec<u32> = (0..groups.items() as u32).collect();
    let Some(limit) = limit.filter(|limit| *limit < items.len()) else {
        return items;
    };
    if limit == 0 {
        items.clear();
        return items;
    }
    let id = |item: u32| {
        get(groups.first[item as usize])
            .item_id
            .as_deref()
            .unwrap_or("")
    };
    items.select_nth_unstable_by(limit - 1, |a, b| {
        groups.last[*b as usize]
            .cmp(&groups.last[*a as usize])
            .then_with(|| id(*b).cmp(id(*a)))
    });
    items.truncate(limit);
    items
}

/// The events of the `picked` items, item by item and each by time (ties in
/// input order), as indices; and each item's start and length in them.
fn runs<'e>(
    get: &impl Fn(u32) -> &'e Event,
    with_item: &[u32],
    groups: &Groups,
    picked: &[u32],
) -> (Vec<u32>, Vec<(u32, u32)>) {
    let mut slot_of = vec![u32::MAX; groups.items()];
    let mut runs: Vec<(u32, u32)> = Vec::with_capacity(picked.len());
    let mut total = 0u32;
    for (p, &item) in picked.iter().enumerate() {
        slot_of[item as usize] = p as u32;
        let len = groups.count[item as usize];
        runs.push((total, len));
        total += len;
    }
    let mut order = vec![0u32; total as usize];
    let mut filled: Vec<u32> = runs.iter().map(|run| run.0).collect();
    for (&index, &item) in with_item.iter().zip(&groups.item_of) {
        let p = slot_of[item as usize];
        if p != u32::MAX {
            order[filled[p as usize] as usize] = index;
            filled[p as usize] += 1;
        }
    }
    for &(start, len) in &runs {
        let run = &mut order[start as usize..(start + len) as usize];
        if !run.is_sorted_by_key(|&i| get(i).at) {
            run.sort_by_key(|&i| get(i).at);
        }
    }
    (order, runs)
}

/// What `rows` say of each item id, as a map from id to rows.
fn by_item<T>(rows: &[T], id: impl Fn(&T) -> &str) -> FastMap<&str, Vec<&T>> {
    let mut by_id: FastMap<&str, Vec<&T>> = FastMap::default();
    for row in rows {
        by_id.entry(id(row)).or_default().push(row);
    }
    by_id
}

/// [`by_item`] as indices into `rows`, owning its keys.
fn indices<T>(rows: &[T], id: impl Fn(&T) -> &str) -> FastMap<String, Vec<u32>> {
    let mut by_id: FastMap<String, Vec<u32>> = FastMap::default();
    for (at, row) in rows.iter().enumerate() {
        by_id.entry(id(row).to_owned()).or_default().push(at as u32);
    }
    by_id
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

/// The ends of an item's line from the pixels of its events, without
/// drawing the dots: the same fold [`cluster`] runs, keeping the ends.
fn ends_of<'e>(
    run: &[u32],
    get: &impl Fn(u32) -> &'e Event,
    warp: &Warp,
    scratch: &mut Vec<(f64, Micros)>,
) -> Ends {
    scratch.clear();
    scratch.extend(run.iter().map(|&i| {
        let event = get(i);
        (warp.x(event.at), event.at)
    }));
    let order =
        |a: &(f64, Micros), b: &(f64, Micros)| a.0.total_cmp(&b.0).then_with(|| a.1.cmp(&b.1));
    if !scratch.is_sorted_by(|a, b| order(a, b).is_le()) {
        scratch.sort_by(order);
    }
    let mut ends = Ends {
        x0: 0.0,
        first: 0,
        x1: 0.0,
        last: 0,
    };
    let mut dots = 0usize;
    for &(x, _) in scratch.iter() {
        if dots > 0 && x - ends.x1 < DOT_MERGE_PX {
            ends.last += 1;
            continue;
        }
        match dots {
            0 => ends.x0 = x,
            1 => ends.first = ends.last,
            _ => {}
        }
        dots += 1;
        ends.x1 = x;
        ends.last = 1;
    }
    if dots == 1 {
        ends.first = ends.last;
    }
    ends
}

/// Labels and extent of an item from the ends of its line: the tip label
/// past the last dot, the start label before the first when the line is long
/// enough, and the fork lanes' reach.
fn reach(ends: &Ends, id: &str, lanes: &[&Lane], warp: &Warp) -> Reach {
    let label_x = ends.x1 + clearance(ends.last);
    let start_label_x =
        (ends.x1 - ends.x0 >= START_LABEL_PX).then(|| ends.x0 - clearance(ends.first));
    let tip = label_x + label_px(id);
    let right = lanes.iter().fold(tip, |reach, lane| {
        let end = warp.x(lane.to).max(warp.x(lane.from));
        reach.max(end + FORK_NODE_PX + label_px_chars(truncated_chars(&lane.label)))
    });
    let left = match start_label_x {
        None => ends.x0 - clearance(ends.first),
        Some(at) => at - label_px(id) - LABEL_GAP,
    };
    Reach {
        label_x,
        start_label_x,
        left,
        right,
    }
}

fn place(
    branch: &Branch<'_>,
    warp: &Warp,
    lanes: &[&Lane],
    override_status: Option<&str>,
) -> Option<Placed> {
    let dots = cluster(branch.run.iter().map(|event| (*event, warp.x(event.at))));
    let (first, last_dot) = (dots.first()?, dots.last()?);
    let (x0, x1) = (first.x, last_dot.x);
    let summary = summarize(branch.run, override_status)?;
    let ends = Ends {
        x0,
        first: first.count,
        x1,
        last: last_dot.count,
    };
    let Reach {
        label_x,
        start_label_x,
        left,
        right,
    } = reach(&ends, branch.id, lanes, warp);

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
fn pack(slots: &mut [Slot]) -> usize {
    let mut fit = FirstFit::new(slots.len());
    for slot in slots.iter_mut() {
        slot.lane = fit.place(slot.left, slot.right + LABEL_GAP);
    }
    fit.lanes()
}

/// Rows run from the one touched most recently down, so the top of the graph
/// is where things are happening now. Equal times keep the lower row first.
fn newest_first(slots: &mut [Slot], lanes: usize) {
    let mut newest = vec![Micros::MIN; lanes];
    for slot in slots.iter() {
        newest[slot.lane] = newest[slot.lane].max(slot.last);
    }
    let mut order: Vec<usize> = (0..lanes).collect();
    order.sort_by(|a, b| newest[*b].cmp(&newest[*a]));
    let mut rank = vec![0; lanes];
    for (r, lane) in order.into_iter().enumerate() {
        rank[lane] = r;
    }
    for slot in slots.iter_mut() {
        slot.lane = rank[slot.lane];
    }
}

/// Where each row rank starts, in pixel rows, and the pixel rows used. An
/// item with forks keeps the rows under its own for its child lanes, so a
/// row is as tall as the most forks any item in it has.
fn row_offsets(slots: &[Slot], lanes: usize) -> (Vec<usize>, usize) {
    let mut extras = vec![0usize; lanes];
    for slot in slots {
        let extra = &mut extras[slot.lane];
        *extra = (*extra).max(slot.forks as usize);
    }
    let mut offsets = Vec::with_capacity(lanes);
    let mut rows = 0usize;
    for extra in &extras {
        offsets.push(rows);
        rows += 1 + extra;
    }
    (offsets, rows)
}

/// One [`RowSpan`] per slot, by row and then by position in `slots`, which
/// is the order of x: a stable counting sort on the row rank.
fn row_spans(slots: &[Slot], lanes: usize, y_of: &impl Fn(&Slot) -> f64) -> Vec<RowSpan> {
    let mut next = vec![0usize; lanes + 1];
    for slot in slots {
        next[slot.lane + 1] += 1;
    }
    for lane in 0..lanes {
        next[lane + 1] += next[lane];
    }
    let blank = RowSpan {
        y: 0.0,
        height: 0.0,
        item: 0,
    };
    let mut spans = vec![blank; slots.len()];
    for (at, slot) in slots.iter().enumerate() {
        let to = next[slot.lane];
        next[slot.lane] += 1;
        spans[to] = RowSpan {
            y: y_of(slot),
            height: (1 + slot.forks) as f64 * ROW,
            item: at as u32,
        };
    }
    spans
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

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::widgets::trace::model::LaneKind;
    use crate::widgets::trace::sample::{self, DEMO_NOW};

    const SINCE: Micros = 1_000_000_000_000_000;
    const DAY: i64 = 86_400;
    const KINDS: [EventKind; 4] = [
        EventKind::Note,
        EventKind::Commit,
        EventKind::RunFailed,
        EventKind::BranchHead,
    ];

    /// The graph of `events` built and lazy, which must agree on everything
    /// but where the items live; each lazy item must equal the built one.
    fn assert_lazy_equals_built(events: &[Event], make: impl Fn(bool) -> Graph) {
        let (built, lazy) = (make(false), make(true));
        assert!(lazy.items.is_empty(), "a lazy graph builds no items");
        assert_eq!(lazy.item_count(), built.items.len());
        assert_eq!(built.row_spans().len(), built.items.len());
        for (at, want) in built.items.iter().enumerate() {
            assert_eq!(&*lazy.item(events, at as u32), want, "item {at}");
            assert_eq!(&*built.item(events, at as u32), want, "built item {at}");
        }
        let bare = |g: &Graph| Graph {
            items: Vec::new(),
            lazy: None,
            ..g.clone()
        };
        assert_eq!(bare(&built), bare(&lazy));
    }

    fn sample_options<'a>(a: &'a sample::Activity, lazy: bool) -> Options<'a> {
        Options::new(1400, Range::from_secs(DEMO_NOW - 30 * DAY, None, DEMO_NOW))
            .tree(&a.tree)
            .statuses(&a.statuses)
            .lazy(lazy)
    }

    #[test]
    fn a_lazy_sample_equals_the_built_one_item_for_item() {
        let a = sample::activity();
        assert_lazy_equals_built(&a.events, |lazy| {
            Graph::layout_slice(&a.events, |_| true, &sample_options(&a, lazy))
        });
    }

    #[test]
    fn a_lazy_capped_sample_with_open_strip_rows_equals_the_built_one() {
        let a = sample::activity();
        assert_lazy_equals_built(&a.events, |lazy| {
            let opts = sample_options(&a, lazy)
                .limit(Some(20))
                .loose_open(LooseOpen::All);
            Graph::layout_slice(&a.events, |e| e.kind != EventKind::Heartbeat, &opts)
        });
    }

    #[test]
    fn rows_by_y_find_every_item_in_a_band() {
        let a = sample::activity_sized(30_000);
        let g = Graph::layout_slice(&a.events, |_| true, &sample_options(&a, true));
        let spans = g.row_spans();
        assert_eq!(spans.len(), g.total_items);
        assert!(spans.windows(2).all(|w| w[0].y <= w[1].y), "sorted by y");
        let mut seen: Vec<u32> = spans.iter().map(|s| s.item).collect();
        seen.sort_unstable();
        assert!(seen.iter().enumerate().all(|(at, item)| at as u32 == *item));

        let tallest = spans.iter().map(|s| s.height).fold(0.0, f64::max);
        for (y0, y1) in [
            (0.0, 40.0),
            (100.0, 100.0),
            (500.0, 900.0),
            (g.axis_y - 30.0, g.axis_y),
        ] {
            let band = g.span(y0, y1);
            for (at, s) in spans.iter().enumerate() {
                let top = s.y - ROW / 2.0;
                if top < y1 && top + s.height > y0 {
                    assert!(band.contains(&at), "row {at} at {top} missed by {y0}..{y1}");
                }
            }
            for s in &spans[band] {
                let top = s.y - ROW / 2.0;
                assert!(
                    top < y1 && top + tallest > y0,
                    "{top} is outside {y0}..{y1}"
                );
            }
        }
        assert_eq!(g.span(f64::MIN, f64::MAX), 0..spans.len());
        // A band just inside the bottom of a tall fork stack still finds it.
        let tall: Vec<(usize, &RowSpan)> = spans
            .iter()
            .enumerate()
            .filter(|(_, s)| s.height > ROW)
            .collect();
        assert!(!tall.is_empty(), "the sample has items with forks");
        for (at, s) in tall {
            let y = s.y - ROW / 2.0 + s.height - 1.0;
            assert!(
                g.span(y, y + 1.0).contains(&at),
                "row {at} lost its fork rows"
            );
        }
        assert!(g.span(1e9, 2e9).is_empty());
    }

    #[test]
    #[should_panic(expected = "slice it was laid out from")]
    fn a_lazy_graph_refuses_a_slice_it_was_not_laid_out_from() {
        let a = sample::activity();
        let g = Graph::layout_slice(&a.events, |_| true, &sample_options(&a, true));
        let _ = g.item(&a.events[1..], 0);
    }

    fn tree() -> SessionTree {
        let lane = |item: &str, label: &str, from: i64, to: i64, chosen| Lane {
            id: format!("{item}/collab/{label}"),
            item_id: item.to_owned(),
            kind: LaneKind::Collab,
            label: label.to_owned(),
            from: SINCE + from * 1_000_000,
            to: SINCE + to * 1_000_000,
            chosen,
            parent: None,
            agent: None,
        };
        let lanes = vec![
            lane("imp-0", "omp", 100, 900, None),
            lane("imp-0", "clara", 200, 1_000, Some(false)),
            lane("imp-3", "worker", 40_000, 80_000, Some(true)),
        ];
        let edges = lanes
            .iter()
            .map(|l| Edge {
                kind: EdgeKind::Fork,
                lane: l.id.clone(),
                item_id: l.item_id.clone(),
                at: l.from,
            })
            .collect();
        SessionTree {
            agent: None,
            lanes,
            edges,
        }
    }

    proptest! {
        #[test]
        fn lazy_layouts_equal_built_ones_on_shuffled_bursty_events(
            rows in proptest::collection::vec((0u8..7, 0usize..3, 0i64..900, 0usize..4), 0..90),
            width in 200u32..2_000,
            limit in proptest::option::of(0usize..6),
            open in any::<bool>(),
        ) {
            let anchors = [0, 36_000, 144_000];
            let events: Vec<Event> = rows
                .iter()
                .enumerate()
                .map(|(n, &(item, anchor, jitter, kind))| {
                    let e = Event::new(format!("e{n}"), SINCE + (anchors[anchor] + jitter) * 1_000_000, KINDS[kind]);
                    if item < 5 { e.item(format!("imp-{item}")) } else { e }
                })
                .collect();
            let tree = tree();
            let statuses: HashMap<String, String> =
                [("imp-1".to_owned(), "done".to_owned())].into_iter().collect();
            assert_lazy_equals_built(&events, |lazy| {
                let opts = Options::new(width, Range::new(SINCE - 3_600_000_000, SINCE + 200_000_000_000))
                    .limit(limit)
                    .loose_open(open)
                    .tree(&tree)
                    .statuses(&statuses)
                    .lazy(lazy);
                Graph::layout_slice(&events, |_| true, &opts)
            });
        }
    }
}
