//! LANES mode: the window's events as a sequence diagram.
//!
//! One lifeline per agent, time flowing down, one row per interaction at a
//! fixed pitch so a painter windows by `y = row * ROW_PITCH`. Two agents meet
//! on an edge: a handoff (`detail.from` to the agent), a baton pass (the
//! next event of an item belongs to someone else) or a review (reviewer to
//! the item's holder). A global action no item owns (a branch head, a commit,
//! a merge) is a broadcast across every lifeline. Plain activity is a dot on
//! one lifeline, drawn only while `activity` is on. A day separator is a row
//! of its own, so the pitch never varies.
//!
//! Every agent that interacted has a lane. [`Sequence::show`] draws a chosen
//! few and folds the rest into one narrow `+N` lane at the right, where the
//! edges to a folded agent end; [`Sequence::columns`] sizes the columns.

use std::collections::HashMap;

use serde::Serialize;

use super::super::model::{is_named, Event, EventKind, Micros};
use super::super::query::LanePick;
use super::super::time;

/// Height of one row in CSS pixels.
pub const ROW_PITCH: f64 = 22.0;
/// Lanes up to which the default draws every agent; above it, the busiest
/// [`BUSIEST_LANES`].
pub const LANE_CAP: usize = 16;
/// Lanes the default draws when there are more than [`LANE_CAP`].
pub const BUSIEST_LANES: usize = 8;
/// Width of the stamp gutter in CSS pixels.
pub const GUTTER_W: f64 = 56.0;
/// Least width of an agent's column.
pub const LANE_MIN_W: f64 = 112.0;
/// Width of the folded `+N` column.
pub const FOLD_W: f64 = 56.0;
/// Below this width a column drops its count and keeps its name.
pub const COUNT_MIN_W: f64 = 72.0;
/// Events up to which plain activity is drawn unless the host says otherwise.
pub const ACTIVITY_AUTO_MAX: usize = 5_000;

const MICROS_PER_DAY: i64 = 86_400_000_000;

/// How two lifelines met.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    /// A `handoff` event: `detail.from` to the event's agent.
    Handoff,
    /// Consecutive events of one item changed agent: previous to new holder.
    Baton,
    /// A `pr_reviewed` event: the reviewer to the item's holder.
    Review,
}

impl EdgeKind {
    /// Snake-case name, e.g. `handoff`.
    pub fn as_str(self) -> &'static str {
        match self {
            EdgeKind::Handoff => "handoff",
            EdgeKind::Baton => "baton",
            EdgeKind::Review => "review",
        }
    }
}

/// One row of the diagram. `event` indexes the slice [`sequence`] read, `*_lane`
/// are [`LaneHead::x_index`] values.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SeqRow {
    /// A UTC day starts here; `at` is its midnight.
    Day {
        /// Midnight, microseconds.
        at: Micros,
    },
    /// Two lifelines met; the arrowhead is at `to_lane`.
    Edge {
        /// When, microseconds.
        at: Micros,
        /// Lifeline the arrow leaves.
        from_lane: u32,
        /// Lifeline the arrow reaches.
        to_lane: u32,
        /// The event, by index.
        event: u32,
        /// Why the lifelines met.
        kind: EdgeKind,
    },
    /// A global action: a bar across every lifeline.
    Broadcast {
        /// When, microseconds.
        at: Micros,
        /// The event, by index.
        event: u32,
    },
    /// Plain activity on one lifeline.
    Dot {
        /// When, microseconds.
        at: Micros,
        /// The lifeline.
        lane: u32,
        /// The event, by index.
        event: u32,
    },
}

impl SeqRow {
    /// The row's instant: the event's, or a day's midnight.
    pub fn at(&self) -> Micros {
        match *self {
            SeqRow::Day { at }
            | SeqRow::Edge { at, .. }
            | SeqRow::Broadcast { at, .. }
            | SeqRow::Dot { at, .. } => at,
        }
    }

    /// Index of the row's event; a day has none.
    pub fn event(&self) -> Option<u32> {
        match *self {
            SeqRow::Day { .. } => None,
            SeqRow::Edge { event, .. }
            | SeqRow::Broadcast { event, .. }
            | SeqRow::Dot { event, .. } => Some(event),
        }
    }

    /// Whether the row starts or ends on lifeline `lane`; a broadcast spans
    /// every lifeline and so belongs to none.
    pub fn touches(&self, lane: u32) -> bool {
        match *self {
            SeqRow::Edge {
                from_lane, to_lane, ..
            } => from_lane == lane || to_lane == lane,
            SeqRow::Dot { lane: at, .. } => at == lane,
            SeqRow::Day { .. } | SeqRow::Broadcast { .. } => false,
        }
    }
}

/// A lifeline's header.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct LaneHead {
    /// The agent, or `+N` for the folded lifeline.
    pub agent: String,
    /// Column of the lifeline, from the left.
    pub x_index: usize,
    /// Interactions on it: each end of an edge, each dot.
    pub count: usize,
    /// First interaction, microseconds.
    pub first: Micros,
    /// Last interaction, microseconds.
    pub last: Micros,
    /// The agents behind a `+N` lifeline, sorted; empty on any other.
    pub folded: Vec<String>,
}

/// One agent of the window, for the lane menu.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RosterLane {
    /// The agent.
    pub agent: String,
    /// Interactions on its lane.
    pub count: usize,
    /// Whether it stands as a lane of its own.
    pub shown: bool,
}

/// What to draw.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Options {
    /// Draw plain activity; `None` draws it up to [`ACTIVITY_AUTO_MAX`] events.
    pub activity: Option<bool>,
}

impl Options {
    /// Draws plain activity on (`Some(true)`), off, or by event count (`None`).
    pub fn activity(mut self, activity: Option<bool>) -> Self {
        self.activity = activity;
        self
    }
}

/// The diagram of a set of events.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Sequence {
    /// Lifelines left to right, in order of first interaction; a folded
    /// lifeline last. [`sequence`] returns every agent's.
    pub lanes: Vec<LaneHead>,
    /// Every agent of the window, busiest first, and whether it has a lane.
    pub roster: Vec<RosterLane>,
    /// Rows top to bottom, day separators included.
    pub rows: Vec<SeqRow>,
    /// Events the window and filters kept.
    pub read: usize,
    /// Rows that are not day separators.
    pub drawn: usize,
    /// Events with an agent and no interaction: dots, drawn or not.
    pub quiet: usize,
    /// Events with no agent that no row draws.
    pub hidden: usize,
    /// Whether plain activity is drawn.
    pub activity: bool,
}

/// Agent of an event, when it names someone.
fn named(agent: Option<&str>) -> Option<&str> {
    agent.filter(|agent| is_named(Some(agent)))
}

struct Stat {
    count: usize,
    first: Micros,
    last: Micros,
}

enum Raw {
    Edge { from: u32, to: u32, kind: EdgeKind },
    Broadcast,
    Dot { agent: u32 },
}

struct RawRow {
    at: Micros,
    event: u32,
    what: Raw,
}

/// Agents by first sight, and what they took part in.
#[derive(Default)]
struct Agents<'a> {
    ids: HashMap<&'a str, u32>,
    names: Vec<&'a str>,
    stats: Vec<Stat>,
}

impl<'a> Agents<'a> {
    fn id(&mut self, name: &'a str) -> u32 {
        if let Some(&id) = self.ids.get(name) {
            return id;
        }
        let id = self.names.len() as u32;
        self.ids.insert(name, id);
        self.names.push(name);
        self.stats.push(Stat {
            count: 0,
            first: 0,
            last: 0,
        });
        id
    }

    fn touch(&mut self, id: u32, at: Micros) {
        let stat = &mut self.stats[id as usize];
        if stat.count == 0 {
            stat.first = at;
        }
        stat.count += 1;
        stat.last = at;
    }
}

/// Builds the diagram of the events `keep` accepts. `events` may be any
/// superset; rows index into it.
pub fn sequence(events: &[Event], keep: impl Fn(&Event) -> bool, options: &Options) -> Sequence {
    let mut order: Vec<(Micros, u8, u64, u32)> = Vec::new();
    for (index, event) in events.iter().enumerate() {
        if keep(event) {
            let (at, rank, seq) = event.order_key();
            order.push((at, rank, seq, index as u32));
        }
    }
    order.sort_unstable();
    let activity = options.activity.unwrap_or(order.len() <= ACTIVITY_AUTO_MAX);

    let mut agents = Agents::default();
    let mut holder: HashMap<&str, u32> = HashMap::new();
    let mut raw: Vec<RawRow> = Vec::new();
    let (mut quiet, mut hidden) = (0usize, 0usize);
    for &(at, _, _, index) in &order {
        let event = &events[index as usize];
        let item = event.item_id.as_deref();
        let to = named(event.agent.as_deref()).map(|name| agents.id(name));
        let mut edge: Option<(u32, u32, EdgeKind)> = None;
        if let Some(to) = to {
            if event.kind == EventKind::Handoff {
                if let Some(from) = named(event.detail.from.as_deref()) {
                    let from = agents.id(from);
                    if from != to {
                        edge = Some((from, to, EdgeKind::Handoff));
                    }
                }
            }
            if let (None, Some(item)) = (edge, item) {
                match (event.kind, holder.get(item).copied()) {
                    (EventKind::PrReviewed, Some(held)) if held != to => {
                        edge = Some((to, held, EdgeKind::Review));
                    }
                    (EventKind::PrReviewed, _) => {}
                    (_, Some(prev)) if prev != to => edge = Some((prev, to, EdgeKind::Baton)),
                    _ => {}
                }
            }
            if let Some(item) = item.filter(|_| event.kind != EventKind::PrReviewed) {
                holder.insert(item, to);
            }
        }
        let what = if let Some((from, to, kind)) = edge {
            agents.touch(from, at);
            agents.touch(to, at);
            Some(Raw::Edge { from, to, kind })
        } else if item.is_none()
            && matches!(
                event.kind,
                EventKind::BranchHead | EventKind::Commit | EventKind::PrMerged
            )
        {
            Some(Raw::Broadcast)
        } else if let Some(agent) = to {
            quiet += 1;
            activity.then(|| {
                agents.touch(agent, at);
                Raw::Dot { agent }
            })
        } else {
            hidden += 1;
            None
        };
        if let Some(what) = what {
            raw.push(RawRow {
                at,
                event: index,
                what,
            });
        }
    }

    let (lanes, lane_of) = lanes(&agents);
    let body: Vec<SeqRow> = raw
        .iter()
        .map(|row| match row.what {
            Raw::Edge { from, to, kind } => SeqRow::Edge {
                at: row.at,
                from_lane: lane_of[from as usize],
                to_lane: lane_of[to as usize],
                event: row.event,
                kind,
            },
            Raw::Broadcast => SeqRow::Broadcast {
                at: row.at,
                event: row.event,
            },
            Raw::Dot { agent } => SeqRow::Dot {
                at: row.at,
                lane: lane_of[agent as usize],
                event: row.event,
            },
        })
        .collect();
    let drawn = body.len();
    let mut seq = Sequence {
        lanes,
        roster: Vec::new(),
        rows: with_days(body),
        read: order.len(),
        drawn,
        quiet,
        hidden,
        activity,
    };
    seq.roster = seq.roster_of(|_| true);
    seq
}

/// The lifelines of `agents`, one each in order of first interaction, and
/// each agent's column. Agents with no interaction have none.
fn lanes(agents: &Agents<'_>) -> (Vec<LaneHead>, Vec<u32>) {
    let (names, stats) = (&agents.names, &agents.stats);
    let mut ids: Vec<u32> = (0..names.len() as u32)
        .filter(|&id| stats[id as usize].count > 0)
        .collect();
    ids.sort_by(|&a, &b| {
        let (a, b) = (a as usize, b as usize);
        stats[a]
            .first
            .cmp(&stats[b].first)
            .then(names[a].cmp(names[b]))
    });
    let mut lane_of = vec![u32::MAX; names.len()];
    let mut lanes: Vec<LaneHead> = Vec::with_capacity(ids.len());
    for (x_index, &id) in ids.iter().enumerate() {
        lane_of[id as usize] = x_index as u32;
        let stat = &stats[id as usize];
        lanes.push(LaneHead {
            agent: names[id as usize].to_string(),
            x_index,
            count: stat.count,
            first: stat.first,
            last: stat.last,
            folded: Vec::new(),
        });
    }
    (lanes, lane_of)
}

/// `body` with a day separator before the first row of each UTC day.
fn with_days(body: Vec<SeqRow>) -> Vec<SeqRow> {
    let mut rows = Vec::with_capacity(body.len() + 1);
    let mut day = None;
    for row in body {
        let today = row.at().div_euclid(MICROS_PER_DAY);
        if day != Some(today) {
            rows.push(SeqRow::Day {
                at: today * MICROS_PER_DAY,
            });
            day = Some(today);
        }
        rows.push(row);
    }
    rows
}

/// The columns of a diagram in a scroller of a given width.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Columns {
    /// Lifelines of agents, the folded one not counted.
    pub lanes: usize,
    /// Width of an agent's column.
    pub lane_w: f64,
    /// Width of the folded column; zero when nothing is folded.
    pub fold_w: f64,
    /// Width of every column together, gutter excluded.
    pub width: f64,
    /// Whether the columns are wider than the room, so the body scrolls
    /// sideways.
    pub scrolls: bool,
}

impl Columns {
    /// The x of lifeline `lane`, gutter excluded; the folded lane is the last.
    pub fn x(&self, lane: usize) -> f64 {
        if lane < self.lanes {
            (lane as f64 + 0.5) * self.lane_w
        } else {
            self.lanes as f64 * self.lane_w + self.fold_w / 2.0
        }
    }

    /// The width of lifeline `lane`'s column.
    pub fn span(&self, lane: usize) -> f64 {
        if lane < self.lanes {
            self.lane_w
        } else {
            self.fold_w
        }
    }
}

/// Whether a column `width` px wide keeps its count: below [`COUNT_MIN_W`]
/// the count goes before the name does.
pub fn shows_count(width: f64) -> bool {
    width >= COUNT_MIN_W
}

impl Sequence {
    /// Height of every row together, in CSS pixels.
    pub fn height(&self) -> f64 {
        self.rows.len() as f64 * ROW_PITCH
    }

    /// Whether no row draws anything.
    pub fn is_empty(&self) -> bool {
        self.drawn == 0
    }

    /// Whether the last lifeline holds folded agents.
    pub fn folds(&self) -> bool {
        self.lanes
            .last()
            .is_some_and(|lane| !lane.folded.is_empty())
    }

    /// The columns of the lifelines in a scroller `available` px wide, gutter
    /// included: each at least [`LANE_MIN_W`], growing to share the room
    /// the folded column leaves; wider than the room, the body scrolls.
    pub fn columns(&self, available: f64) -> Columns {
        let fold_w = if self.folds() { FOLD_W } else { 0.0 };
        let lanes = self.lanes.len() - usize::from(self.folds());
        let room = (available - GUTTER_W - fold_w).max(0.0);
        let lane_w = if lanes == 0 {
            LANE_MIN_W
        } else {
            (room / lanes as f64).floor().max(LANE_MIN_W)
        };
        let width = lanes as f64 * lane_w + fold_w;
        Columns {
            lanes,
            lane_w,
            fold_w,
            width,
            scrolls: width > available - GUTTER_W,
        }
    }

    /// Lane indices, busiest first, then earliest, then by name.
    fn busiest(&self) -> Vec<usize> {
        let mut ranked: Vec<usize> = (0..self.lanes.len()).collect();
        ranked.sort_by(|&a, &b| {
            let (a, b) = (&self.lanes[a], &self.lanes[b]);
            b.count
                .cmp(&a.count)
                .then(a.first.cmp(&b.first))
                .then(a.agent.cmp(&b.agent))
        });
        ranked
    }

    /// The roster, busiest first, with `drawn` saying who has a lane.
    fn roster_of(&self, drawn: impl Fn(usize) -> bool) -> Vec<RosterLane> {
        self.busiest()
            .into_iter()
            .map(|at| RosterLane {
                agent: self.lanes[at].agent.clone(),
                count: self.lanes[at].count,
                shown: drawn(at),
            })
            .collect()
    }

    /// The diagram with the lanes `pick` draws standing and the rest folded
    /// into one `+N` lane at the right; an edge to a folded agent ends on it.
    /// Call it on what [`sequence`] returned, whose lanes are every agent's.
    pub fn show(&self, pick: &LanePick) -> Sequence {
        let mut drawn = vec![false; self.lanes.len()];
        match pick {
            LanePick::All => drawn.fill(true),
            LanePick::Auto if self.lanes.len() <= LANE_CAP => drawn.fill(true),
            LanePick::Auto => {
                for at in self.busiest().into_iter().take(BUSIEST_LANES) {
                    drawn[at] = true;
                }
            }
            LanePick::Only(names) => {
                for (at, lane) in self.lanes.iter().enumerate() {
                    drawn[at] = names.contains(&lane.agent);
                }
            }
        }

        let mut lane_of = vec![0u32; self.lanes.len()];
        let mut lanes: Vec<LaneHead> = Vec::with_capacity(self.lanes.len());
        for (at, lane) in self.lanes.iter().enumerate().filter(|(at, _)| drawn[*at]) {
            lane_of[at] = lanes.len() as u32;
            lanes.push(LaneHead {
                x_index: lanes.len(),
                ..lane.clone()
            });
        }
        let left: Vec<usize> = (0..self.lanes.len()).filter(|&at| !drawn[at]).collect();
        if !left.is_empty() {
            let x_index = lanes.len();
            let mut fold = LaneHead {
                agent: format!("+{}", left.len()),
                x_index,
                count: 0,
                first: Micros::MAX,
                last: Micros::MIN,
                folded: Vec::with_capacity(left.len()),
            };
            for at in left {
                lane_of[at] = x_index as u32;
                let lane = &self.lanes[at];
                fold.count += lane.count;
                fold.first = fold.first.min(lane.first);
                fold.last = fold.last.max(lane.last);
                fold.folded.push(lane.agent.clone());
            }
            fold.folded.sort();
            lanes.push(fold);
        }
        let rows = self
            .rows
            .iter()
            .map(|row| match *row {
                SeqRow::Edge {
                    at,
                    from_lane,
                    to_lane,
                    event,
                    kind,
                } => SeqRow::Edge {
                    at,
                    from_lane: lane_of[from_lane as usize],
                    to_lane: lane_of[to_lane as usize],
                    event,
                    kind,
                },
                SeqRow::Dot { at, lane, event } => SeqRow::Dot {
                    at,
                    lane: lane_of[lane as usize],
                    event,
                },
                other => other,
            })
            .collect();
        Sequence {
            lanes,
            roster: self.roster_of(|at| drawn[at]),
            rows,
            read: self.read,
            drawn: self.drawn,
            quiet: self.quiet,
            hidden: self.hidden,
            activity: self.activity,
        }
    }

    /// The rows that start or end on lifeline `lane`, with day separators
    /// redrawn for what is left; broadcasts belong to no lifeline.
    pub fn touching(&self, lane: usize) -> Sequence {
        let body: Vec<SeqRow> = self
            .rows
            .iter()
            .filter(|row| row.touches(lane as u32))
            .copied()
            .collect();
        Sequence {
            lanes: self.lanes.clone(),
            roster: self.roster.clone(),
            drawn: body.len(),
            rows: with_days(body),
            read: self.read,
            quiet: self.quiet,
            hidden: self.hidden,
            activity: self.activity,
        }
    }
}

/// `MM-DD` of the UTC day of `at`: a day separator's label.
pub fn day_label(at: Micros) -> String {
    let c = time::civil(at);
    format!("{:02}-{:02}", c.month, c.day)
}

/// `HH:MM` of `at` in UTC: a row's stamp in the gutter.
pub fn clock_label(at: Micros) -> String {
    let c = time::civil(at);
    format!("{:02}:{:02}", c.hour, c.minute)
}

/// A DOM-safe id: each run of characters outside `A-Za-z0-9_-` becomes one `-`.
pub fn slug(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_run = false;
    for c in text.chars() {
        if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
            out.push(c);
            in_run = false;
        } else if !in_run {
            out.push('-');
            in_run = true;
        }
    }
    out
}

/// A row's caption: the kind, where the event points (a pull request, a
/// commit, an item) and the first line of its summary. A branch head is named
/// by its branch, since it has no subject.
pub fn label(event: &Event) -> String {
    if let (EventKind::BranchHead, Some(branch)) = (event.kind, event.branch.as_deref()) {
        let sha: String = event
            .commit
            .as_deref()
            .unwrap_or("")
            .chars()
            .take(8)
            .collect();
        return format!("branch {branch} @ {sha}");
    }
    let link = match (event.item_id.as_deref(), event.commit.as_deref(), event.pr) {
        (_, _, Some(pr)) => Some(format!("#{pr}")),
        (_, Some(commit), None) => Some(commit.chars().take(12).collect::<String>()),
        (Some(item), None, None) => Some(item.to_string()),
        (None, None, None) => None,
    };
    let first_line = event
        .summary
        .as_deref()
        .map(|summary| summary.split('\n').next().unwrap_or("").trim());
    let kind = event.kind.label();
    [Some(kind.as_str()), link.as_deref(), first_line]
        .into_iter()
        .flatten()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Tooltip: kind, agent, item, branch and time, joined with ` · `.
pub fn detail(event: &Event) -> String {
    let at = time::format_iso8601(event.at);
    [
        Some(event.kind.as_str()),
        event.agent.as_deref(),
        event.item_id.as_deref(),
        event.branch.as_deref(),
        Some(at.as_str()),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" · ")
}
