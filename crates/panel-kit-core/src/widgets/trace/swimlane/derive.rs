//! Fork lanes derived from the events of one work item.
//!
//! A host with no session tree of its own still holds the facts one is made
//! of: the branches commits and pull requests name, handoffs, the agents
//! working alongside the owner, the runs that failed and the run that retried
//! them. [`derive_forks`] reads them off an item's events; the layout calls it
//! for every item the explicit tree has no lanes for.
//!
//! The lanes are a function of the multiset of events: the order events
//! arrive in, ties in time included, never changes them.
//!
//! - Branch: one per branch name other than the trunk, from its first to its
//!   last event. `Some(true)` with a merge elbow when a `pr_merged` names it,
//!   `Some(false)` when the item moved on and landed without it.
//! - Handoff: one per agent a `handoff` event names as the new holder, from
//!   the first handoff to that agent's last event.
//! - Collab: one per other named agent, from its first to its last event.
//! - Path: one per run whose last event is `run_failed` or `run_cancelled`
//!   (dead), and the next run that ends in `run_finished` (kept, parented to
//!   the failed one). A run whose events carry a branch hangs off its lane.

use super::super::model::{Edge, EdgeKind, Event, EventKind, Lane, LaneKind, Micros};
use super::group::FastMap;
use super::metrics::truncated_chars;

/// Most lanes one item keeps; the least recent collab lanes go first.
pub const FORK_CAP: usize = 8;

/// Branch names that are the item's own line, not a fork.
const TRUNKS: [&str; 2] = ["main", "master"];
/// Rows a [`Table`] scans linearly before it hashes its keys.
const LINEAR_ROWS: usize = 16;
/// How a run's last event reads: `FAILED` and `FINISHED` end it.
const FAILED: u8 = 2;
const FINISHED: u8 = 1;

/// The lanes and elbows of one item, read off its `events` in any order.
///
/// Lane ids are `<item>/<kind>/<label>`, unique within the item; at most
/// [`FORK_CAP`] lanes come back, in drawing order.
pub fn derive_forks(item_id: &str, events: &[&Event]) -> (Vec<Lane>, Vec<Edge>) {
    derive_with(
        item_id,
        events.iter().copied(),
        None,
        &mut Scratch::default(),
    )
}

/// [`derive_forks`] over any run of events, with the caller's current
/// `status` winning over the events' own when it names whether the item landed.
pub(super) fn derive_with<'e>(
    item_id: &str,
    events: impl Iterator<Item = &'e Event>,
    status: Option<&str>,
    scratch: &mut Scratch<'e>,
) -> (Vec<Lane>, Vec<Edge>) {
    let plan = scratch.plan(events, status);
    let id_of = |kind: LaneKind, label: &str| format!("{item_id}/{}/{label}", kind.as_str());
    let mut lanes = Vec::with_capacity(plan.len());
    let mut edges = Vec::with_capacity(plan.len() * 2);
    for lane in plan {
        let id = id_of(lane.kind, lane.label);
        edges.push(Edge {
            kind: EdgeKind::Fork,
            lane: id.clone(),
            item_id: item_id.to_owned(),
            at: lane.from,
        });
        if let Some(at) = lane.merge {
            edges.push(Edge {
                kind: EdgeKind::Merge,
                lane: id.clone(),
                item_id: item_id.to_owned(),
                at,
            });
        }
        lanes.push(Lane {
            id,
            item_id: item_id.to_owned(),
            kind: lane.kind,
            label: lane.label.to_owned(),
            from: lane.from,
            to: lane.to,
            chosen: lane.chosen,
            parent: match lane.parent {
                Parent::Item => None,
                Parent::Branch(name) => Some(id_of(LaneKind::Branch, name)),
                Parent::Run(run) => Some(id_of(LaneKind::Path, run)),
            },
            agent: lane.agent.map(str::to_owned),
        });
    }
    (lanes, edges)
}

/// Where a planned lane forks from.
#[derive(Clone, Copy, Debug)]
enum Parent<'e> {
    Item,
    Branch(&'e str),
    Run(&'e str),
}

/// A lane before its strings are built; it borrows from the item's events.
#[derive(Clone, Copy, Debug)]
pub(super) struct Planned<'e> {
    pub kind: LaneKind,
    /// Branch name, agent or run id.
    pub label: &'e str,
    pub from: Micros,
    pub to: Micros,
    pub chosen: Option<bool>,
    /// Instant of the merge elbow.
    pub merge: Option<Micros>,
    pub agent: Option<&'e str>,
    parent: Parent<'e>,
    /// Where the lane was found, the last tie-break of every order.
    found: u16,
}

struct AgentRow {
    first: Micros,
    last: Micros,
    /// First handoff that named the agent the new holder.
    handoff: Option<Micros>,
}

struct BranchRow<'e> {
    first: Micros,
    last: Micros,
    merged: Option<Micros>,
    /// Earliest named agent on the branch, ties to the smaller name.
    agent: Option<(Micros, &'e str)>,
}

struct RunRow<'e> {
    first: Micros,
    started: Option<Micros>,
    /// Newest event by time, source rank and `seq`, then how it reads.
    end: (Micros, u8, u64, u8),
    branch: Option<(Micros, &'e str)>,
    agent: Option<(Micros, &'e str)>,
}

/// Rows keyed by short strings: scanned while few, hashed once many.
struct Table<'e, V> {
    rows: Vec<(&'e str, V)>,
    index: FastMap<&'e str, u32>,
    hashed: bool,
}

impl<V> Default for Table<'_, V> {
    fn default() -> Self {
        Table {
            rows: Vec::new(),
            index: FastMap::default(),
            hashed: false,
        }
    }
}

impl<'e, V> Table<'e, V> {
    fn clear(&mut self) {
        self.rows.clear();
        if self.hashed {
            self.index.clear();
            self.hashed = false;
        }
    }

    fn entry(&mut self, key: &'e str, new: impl FnOnce() -> V) -> &mut V {
        let found = if self.hashed {
            self.index.get(key).map(|&at| at as usize)
        } else {
            self.rows.iter().position(|(name, _)| *name == key)
        };
        let at = match found {
            Some(at) => at,
            None => {
                let at = self.rows.len();
                self.rows.push((key, new()));
                if self.hashed {
                    self.index.insert(key, at as u32);
                } else if self.rows.len() > LINEAR_ROWS {
                    self.hashed = true;
                    self.index.clear();
                    self.index.extend(
                        self.rows
                            .iter()
                            .enumerate()
                            .map(|(at, (name, _))| (*name, at as u32)),
                    );
                }
                at
            }
        };
        &mut self.rows[at].1
    }
}

/// Buffers one derivation after another reuses, so a layout over a million
/// events allocates for the busiest item only.
#[derive(Default)]
pub(super) struct Scratch<'e> {
    branches: Table<'e, BranchRow<'e>>,
    agents: Table<'e, AgentRow>,
    runs: Table<'e, RunRow<'e>>,
    lanes: Vec<Planned<'e>>,
}

/// The name an optional field carries: present and not blank.
fn named(field: &Option<String>) -> Option<&str> {
    field.as_deref().filter(|name| !name.trim().is_empty())
}

fn drawn_rank(kind: LaneKind) -> u8 {
    match kind {
        LaneKind::Branch => 0,
        LaneKind::Path => 1,
        LaneKind::Handoff => 2,
        LaneKind::Collab => 3,
    }
}

/// What goes first when an item has too many lanes.
fn drop_rank(kind: LaneKind) -> u8 {
    match kind {
        LaneKind::Collab => 0,
        LaneKind::Handoff => 1,
        LaneKind::Branch => 2,
        LaneKind::Path => 3,
    }
}

/// Characters of the longest caption `event` could give a derived lane: its
/// branch, its agent, and the run of a run-ending event. What the layout
/// reserves right-hand room for before it has grouped anything.
pub(super) fn caption_chars(event: &Event) -> usize {
    let name = |field: &Option<String>| field.as_deref().map_or(0, truncated_chars);
    let run = if matches!(
        event.kind,
        EventKind::RunFailed | EventKind::RunCancelled | EventKind::RunFinished
    ) {
        name(&event.run_id)
    } else {
        0
    };
    name(&event.branch).max(name(&event.agent)).max(run)
}

impl<'e> Scratch<'e> {
    /// The lanes of one item's `events`, in drawing order. `status` is the
    /// caller's current status for the item, which wins over the events'.
    pub(super) fn plan(
        &mut self,
        events: impl Iterator<Item = &'e Event>,
        status: Option<&str>,
    ) -> &[Planned<'e>] {
        let Scratch {
            branches,
            agents,
            runs,
            lanes,
        } = self;
        branches.clear();
        agents.clear();
        runs.clear();
        lanes.clear();

        let mut last_at = Micros::MIN;
        let mut merged_any = false;
        let mut newest_status: Option<(Micros, &str)> = None;
        let mut first_agent: Option<(Micros, &str)> = None;
        for event in events {
            let at = event.at;
            last_at = last_at.max(at);
            merged_any |= event.kind == EventKind::PrMerged;
            if let Some(text) = event.detail.status.as_deref() {
                if newest_status.is_none_or(|newest| (at, text) > newest) {
                    newest_status = Some((at, text));
                }
            }
            let agent = named(&event.agent);
            if let Some(name) = agent {
                if first_agent.is_none_or(|first| (at, name) < first) {
                    first_agent = Some((at, name));
                }
                let row = agents.entry(name, || AgentRow {
                    first: at,
                    last: at,
                    handoff: None,
                });
                row.first = row.first.min(at);
                row.last = row.last.max(at);
                if event.kind == EventKind::Handoff && event.detail.from.as_deref() != Some(name) {
                    row.handoff = Some(row.handoff.map_or(at, |first| first.min(at)));
                }
            }
            let branch = named(&event.branch).filter(|name| !TRUNKS.contains(name));
            if let Some(name) = branch {
                let row = branches.entry(name, || BranchRow {
                    first: at,
                    last: at,
                    merged: None,
                    agent: None,
                });
                row.first = row.first.min(at);
                row.last = row.last.max(at);
                if event.kind == EventKind::PrMerged {
                    row.merged = Some(row.merged.map_or(at, |merged| merged.min(at)));
                }
                earliest(&mut row.agent, at, agent);
            }
            if let Some(name) = named(&event.run_id) {
                let tag = match event.kind {
                    EventKind::RunFailed | EventKind::RunCancelled => FAILED,
                    EventKind::RunFinished => FINISHED,
                    _ => 0,
                };
                let end = (at, event.source.rank(), event.seq, tag);
                let row = runs.entry(name, || RunRow {
                    first: at,
                    started: None,
                    end,
                    branch: None,
                    agent: None,
                });
                row.first = row.first.min(at);
                row.end = row.end.max(end);
                if event.kind == EventKind::RunStarted {
                    row.started = Some(row.started.map_or(at, |started| started.min(at)));
                }
                earliest(&mut row.branch, at, branch);
                earliest(&mut row.agent, at, agent);
            }
        }
        if branches.rows.is_empty() && agents.rows.is_empty() && runs.rows.is_empty() {
            return lanes;
        }

        let status = status.or(newest_status.map(|(_, text)| text));
        let landed = merged_any || matches!(status, Some("done" | "closed"));
        let mut found = 0u16;
        let mut push = |lanes: &mut Vec<Planned<'e>>, mut lane: Planned<'e>| {
            lane.found = found;
            found = found.saturating_add(1);
            lanes.push(lane);
        };

        branches.rows.sort_unstable_by(|a, b| a.0.cmp(b.0));
        for (name, row) in &branches.rows {
            let chosen = if row.merged.is_some() {
                Some(true)
            } else if landed && last_at > row.last {
                Some(false)
            } else {
                None
            };
            push(
                lanes,
                Planned {
                    kind: LaneKind::Branch,
                    label: name,
                    from: row.first,
                    to: row.last,
                    chosen,
                    merge: row.merged,
                    agent: row.agent.map(|(_, agent)| agent),
                    parent: Parent::Item,
                    found: 0,
                },
            );
        }

        agents.rows.sort_unstable_by(|a, b| a.0.cmp(b.0));
        for (name, row) in &agents.rows {
            let lane = match row.handoff {
                Some(at) => Planned {
                    kind: LaneKind::Handoff,
                    label: name,
                    from: at,
                    to: row.last,
                    chosen: None,
                    merge: None,
                    agent: Some(name),
                    parent: Parent::Item,
                    found: 0,
                },
                None if first_agent.is_some_and(|(_, first)| first == *name) => continue,
                None => Planned {
                    kind: LaneKind::Collab,
                    label: name,
                    from: row.first,
                    to: row.last,
                    chosen: None,
                    merge: (last_at > row.last).then_some(row.last),
                    agent: Some(name),
                    parent: Parent::Item,
                    found: 0,
                },
            };
            push(lanes, lane);
        }

        runs.rows
            .sort_unstable_by_key(|(name, row)| (row.started.unwrap_or(row.first), *name));
        let mut failed: Option<&str> = None;
        for (name, row) in &runs.rows {
            let from = row.started.unwrap_or(row.first);
            let (to, tag) = (row.end.0, row.end.3);
            let agent = row.agent.map(|(_, agent)| agent);
            match tag {
                FAILED => {
                    failed = Some(name);
                    push(
                        lanes,
                        Planned {
                            kind: LaneKind::Path,
                            label: name,
                            from,
                            to,
                            chosen: Some(false),
                            merge: None,
                            agent,
                            parent: row.branch.map_or(Parent::Item, |(_, name)| Parent::Branch(name)),
                            found: 0,
                        },
                    );
                }
                FINISHED => {
                    if let Some(retried) = failed.take() {
                        push(
                            lanes,
                            Planned {
                                kind: LaneKind::Path,
                                label: name,
                                from,
                                to,
                                chosen: Some(true),
                                merge: Some(to),
                                agent,
                                parent: Parent::Run(retried),
                                found: 0,
                            },
                        );
                    }
                }
                _ => {}
            }
        }

        if lanes.len() > FORK_CAP {
            lanes.sort_unstable_by_key(|lane| {
                (drop_rank(lane.kind), lane.to, lane.from, lane.label, lane.found)
            });
            lanes.drain(..lanes.len() - FORK_CAP);
        }
        lanes.sort_unstable_by_key(|lane| (lane.from, drawn_rank(lane.kind), lane.found));
        // A parent the cap removed is the item line again.
        for at in 0..lanes.len() {
            let parent = match lanes[at].parent {
                Parent::Item => continue,
                Parent::Branch(name) => (LaneKind::Branch, name),
                Parent::Run(name) => (LaneKind::Path, name),
            };
            if !lanes
                .iter()
                .any(|lane| lane.kind == parent.0 && lane.label == parent.1)
            {
                lanes[at].parent = Parent::Item;
            }
        }
        lanes
    }
}

/// Keeps the earliest `(at, name)` seen, ties to the smaller name.
fn earliest<'e>(slot: &mut Option<(Micros, &'e str)>, at: Micros, name: Option<&'e str>) {
    if let Some(name) = name {
        if slot.is_none_or(|current| (at, name) < current) {
            *slot = Some((at, name));
        }
    }
}
