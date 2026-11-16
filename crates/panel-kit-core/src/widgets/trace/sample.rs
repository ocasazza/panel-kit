//! Deterministic synthetic activity for the event-graph demo and tests.
//!
//! No clock, no `rand`: instants hang off [`DEMO_NOW`] and every pseudo-random
//! choice comes from [`Lcg`]. Each builder returns the inputs the core layouts
//! need and the same value on every call.
//!
//! - [`session_tree`]: 7 events and 6 lanes of `imp-16` for [`rails::vertical`](super::rails::vertical).
//! - [`item_timeline`]: 50 events and 4 lanes of `imp-42`, crossing three dates.
//! - [`activity`]: the 30-day window for the swimlane graph: 2 351 items (20 hero
//!   items drawn, 2 331 filler), 1 223 loose events, a 2-day idle gap, bursts
//!   and fork lanes ending in agent and host leaves. The heroes hold explicit
//!   lanes; the filler holds none and its events carry the facts a tree is
//!   derived from (branches, other agents, handoffs, failed and retried runs).
//! - [`activity_sized`]: [`activity`] scaled to any event count, a million
//!   included: the same heroes, idle days and fork lanes, filler items and
//!   loose events in the same proportions, and past [`DEFAULT_EVENTS`] twelve
//!   agents, 41 branch names and a long tail of busy items. At every size
//!   about 56% of the filler derives a lane, a fifth three or more.

use std::collections::HashMap;

use super::model::{
    Edge, EdgeKind, Event, EventKind, ItemRef, Lane, LaneKind, Micros, SessionTree, Source,
};
use super::time::days_from_civil;

/// The demo clock: 2026-10-20T12:00:00Z in epoch seconds.
pub const DEMO_NOW: i64 = 1_792_497_600;
/// Seed of every [`Lcg`] the builders start.
pub const SEED: u64 = 0x5EED_E7E7;
/// Items the swimlane sample holds: `HERO` drawn plus the filler.
pub const TOTAL_ITEMS: usize = 2351;
/// Loose `branch_head` events in [`activity`].
pub const LOOSE_BRANCH_HEADS: usize = 758;
/// Loose `commit` events in [`activity`].
pub const LOOSE_COMMITS: usize = 253;
/// Loose `pr_merged` events in [`activity`].
pub const LOOSE_PR_MERGED: usize = 212;

const SECOND: Micros = 1_000_000;
const DAY: i64 = 86_400;

/// 64-bit linear congruential generator (Knuth MMIX constants).
#[derive(Clone, Debug)]
pub struct Lcg(u64);

impl Lcg {
    /// Generator starting at `seed`.
    pub const fn new(seed: u64) -> Self {
        Lcg(seed)
    }

    /// Next 31 pseudo-random bits.
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 33
    }

    /// Uniform-ish value in `0..n`; `0` when `n` is `0`.
    pub fn below(&mut self, n: u64) -> u64 {
        if n == 0 {
            0
        } else {
            self.next_u64() % n
        }
    }

    /// One of `items`.
    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len() as u64) as usize]
    }
}

/// UTC instant in 2026 as microseconds.
fn at(month: i64, day: i64, hour: i64, minute: i64, second: i64) -> Micros {
    (days_from_civil(2026, month as u32, day as u32) * DAY + hour * 3600 + minute * 60 + second)
        * SECOND
}

fn source_of(kind: EventKind) -> Source {
    match kind {
        EventKind::Commit | EventKind::BranchHead => Source::Git,
        EventKind::PrOpened | EventKind::PrReviewed | EventKind::PrMerged => Source::Github,
        EventKind::RunStarted
        | EventKind::RunTool
        | EventKind::RunThought
        | EventKind::RunReasoning
        | EventKind::RunModel
        | EventKind::RunFinished
        | EventKind::RunFailed
        | EventKind::RunCancelled => Source::Run,
        _ => Source::Ledger,
    }
}

fn event(
    id: String,
    at: Micros,
    kind: EventKind,
    agent: Option<&str>,
    item: Option<&str>,
    summary: &str,
) -> Event {
    let mut e = Event::new(id, at, kind)
        .source(source_of(kind))
        .summary(summary);
    if let Some(agent) = agent {
        e = e.agent(agent);
    }
    if let Some(item) = item {
        e = e.item(item);
    }
    e
}

/// A lane with its fork edge, and its merge edge when `merge` is set.
#[allow(clippy::too_many_arguments)]
fn push_lane(
    tree: &mut SessionTree,
    item: &str,
    kind: LaneKind,
    label: &str,
    from: Micros,
    to: Micros,
    merge: Option<Micros>,
    chosen: Option<bool>,
) {
    let id = format!("{item}/{}/{label}", kind.as_str());
    tree.edges.push(Edge {
        kind: EdgeKind::Fork,
        lane: id.clone(),
        item_id: item.to_owned(),
        at: from,
    });
    if let Some(at) = merge {
        tree.edges.push(Edge {
            kind: EdgeKind::Merge,
            lane: id.clone(),
            item_id: item.to_owned(),
            at,
        });
    }
    tree.lanes.push(Lane {
        id,
        item_id: item.to_owned(),
        kind,
        label: label.to_owned(),
        from,
        to,
        chosen,
        parent: None,
        agent: Some(label.to_owned()),
    });
}

/// What one vertical rail needs: its events, the item and the session tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rail {
    /// Events of the item.
    pub events: Vec<Event>,
    /// The item drawn.
    pub item: ItemRef,
    /// Lanes and elbows of the item.
    pub tree: SessionTree,
}

/// `imp-16`: 7 events, 6 lanes, 8 rows (one lane has no event, so it gets a
/// marker row), five dates collapsed to `DayTime` stamps, owner `omp`.
pub fn session_tree() -> Rail {
    const ITEM: &str = "imp-16";
    let rows: [(Micros, EventKind, &str, &str, Option<&str>); 7] = [
        (
            at(10, 3, 19, 34, 0),
            EventKind::ItemCreated,
            "user",
            "created",
            None,
        ),
        (
            at(10, 4, 0, 16, 0),
            EventKind::ItemClaimed,
            "clara",
            "claimed",
            None,
        ),
        (
            at(10, 5, 3, 43, 10),
            EventKind::ItemStatus,
            "scheduler",
            "open",
            Some("open"),
        ),
        (
            at(10, 5, 3, 43, 40),
            EventKind::Note,
            "operator",
            "reviewed the ingest path after the raft step-down\nthe step-down at 03:41 left two shards without a leader; ingest retried 3 times, then dropped the batch",
            None,
        ),
        (
            at(10, 9, 8, 25, 1),
            EventKind::Note,
            "FHKDV644W6-MBP",
            "handoff FHKDV644W6-MBP picks up the ingest fix",
            None,
        ),
        (
            at(10, 9, 8, 25, 2),
            EventKind::Handoff,
            "omp",
            "handed from",
            None,
        ),
        (
            at(10, 9, 8, 25, 3),
            EventKind::ItemClaimed,
            "omp",
            "claimed",
            None,
        ),
    ];
    let mut events = Vec::new();
    let mut tree = SessionTree {
        agent: Some("omp".to_owned()),
        ..SessionTree::default()
    };
    for (i, (when, kind, agent, summary, status)) in rows.iter().enumerate() {
        let mut e = event(
            format!("{ITEM}:{}", i + 1),
            *when,
            *kind,
            Some(agent),
            Some(ITEM),
            summary,
        );
        if let Some(status) = status {
            e = e.status(*status);
        }
        e = match i {
            0 => e.title("Ingest drops rows after a raft step-down"),
            1 => e.run("run-imp-16-1").branch("fix/ingest-stepdown"),
            3 => e
                .run("run-imp-16-1")
                .branch("fix/ingest-stepdown")
                .commit("9f3c2ab"),
            4 => e.title("Handoff: ingest fix").from("clara"),
            5 => e.from("clara").branch("fix/ingest-stepdown"),
            6 => e
                .run("run-imp-16-2")
                .branch("fix/ingest-stepdown")
                .commit("9f3c2ab")
                .pr(207),
            _ => e,
        };
        events.push(e);
    }
    // Lanes of one event each; the handoff lane stays open, the orchestrator
    // lane has no event and draws as a marker row.
    for (i, &(when, _, agent, _, _)) in rows.iter().enumerate() {
        if matches!(i, 5 | 6) {
            continue;
        }
        let kind = if i == 4 {
            LaneKind::Handoff
        } else {
            LaneKind::Collab
        };
        let merge = (i != 4).then_some(when);
        push_lane(&mut tree, ITEM, kind, agent, when, when, merge, None);
    }
    let orchestrator = at(10, 9, 8, 25, 4);
    push_lane(
        &mut tree,
        ITEM,
        LaneKind::Collab,
        "orchestrator",
        orchestrator,
        orchestrator,
        None,
        None,
    );
    Rail {
        events,
        item: ItemRef {
            id: ITEM.to_owned(),
            status: Some("claimed".to_owned()),
        },
        tree,
    }
}

/// `imp-42`: 50 events, 4 collab lanes, three dates, every tone.
pub fn item_timeline() -> Rail {
    use EventKind as K;
    const ITEM: &str = "imp-42";
    const OMP: &str = "omp";
    const FHKD: &str = "FHKDV644W6-MBP";
    const SELF_FHKD: &str = "selfdev@FHKDV644W6-MBP";
    const SELF_L96: &str = "selfdev@L96N60X3L7-MBP";
    const ASKED_1: &str = "Two unplaced Raft seats need a fix direction";
    const ASKED_2: &str = "Fix two unplaced placements: embedding capacity + reasoning format";
    const ASKED_3: &str =
        "Reasoning-format validator rejects tool-call JSON: loosen or fix the emitter?";
    const ASKED_4: &str = "Merge the emitter fix before the next scheduling window?";

    // (month, day, hour, minute, second, kind, agent, summary, status)
    type Row = (
        i64,
        i64,
        i64,
        i64,
        i64,
        K,
        Option<&'static str>,
        String,
        Option<&'static str>,
    );
    let asked = |m, d, h, mi, text: &str| -> Vec<Row> {
        vec![
            (
                m,
                d,
                h,
                mi,
                0,
                K::Note,
                None,
                format!("asked: {text}"),
                None,
            ),
            (
                m,
                d,
                h,
                mi,
                5,
                K::Note,
                None,
                format!("unanswered: {text}"),
                None,
            ),
        ]
    };
    let mut rows: Vec<Row> = vec![
        (10, 3, 9, 34, 0, K::ItemCreated, None, "created".into(), None),
        (
            10, 4, 3, 41, 0, K::Note, Some(OMP),
            "SEAT SCHEDULING STAGE 1 \u{2014} investigation, read-only. No code changed.\nTwo Raft seats stay unplaced because the embedding capacity gate rejects them; the reasoning-format validator is the second blocker.".into(),
            None,
        ),
        (10, 4, 3, 52, 0, K::RunTool, Some(OMP), "tool: read placement ledger".into(), None),
        (10, 4, 4, 10, 0, K::Note, Some(OMP), "STAGE 1 complete: hand seat scheduling to a host".into(), None),
        (10, 9, 8, 2, 0, K::ItemClaimed, Some(FHKD), "claimed".into(), None),
        (10, 9, 8, 2, 5, K::ItemStatus, None, format!("assigned to {FHKD}"), Some("assigned")),
        (10, 9, 11, 20, 0, K::RunStarted, Some(FHKD), "stage 2: seat placement dry run".into(), None),
        (10, 9, 13, 45, 0, K::RunFinished, Some(FHKD), "dry run placed 5 of 7 seats".into(), None),
        (10, 10, 22, 57, 0, K::ItemClaimed, None, "claimed".into(), None),
        (10, 20, 2, 11, 0, K::ItemReleased, None, "released".into(), None),
        (10, 20, 3, 12, 0, K::ItemClaimed, Some(SELF_FHKD), "claimed".into(), None),
        (
            10, 20, 3, 12, 5, K::ItemStatus, None,
            format!("assigned to {FHKD} ({SELF_FHKD} runs it)"),
            Some("assigned"),
        ),
        (10, 20, 3, 40, 0, K::RunStarted, Some(SELF_FHKD), "stage 3: place the two unplaced seats".into(), None),
        (10, 20, 5, 5, 0, K::RunTool, Some(SELF_FHKD), "tool: placement.solve --seats 2".into(), None),
        (10, 20, 6, 30, 0, K::RunThought, Some(SELF_FHKD), "the embedding capacity gate is the blocker".into(), None),
        (10, 20, 8, 15, 0, K::Commit, Some(SELF_FHKD), "fix: lift embedding capacity gate for seat 4".into(), None),
        (10, 20, 9, 50, 0, K::RunFailed, Some(SELF_FHKD), "reasoning format check failed on seat 6".into(), None),
        (10, 20, 11, 30, 0, K::RunFinished, Some(SELF_FHKD), "placed seat 4; seat 6 still unplaced".into(), None),
        (10, 20, 17, 27, 0, K::ItemReleased, None, "released".into(), None),
        (10, 20, 17, 30, 0, K::ItemClaimed, Some(SELF_L96), "claimed".into(), None),
        (
            10, 20, 17, 30, 5, K::ItemStatus, None,
            format!("assigned to L96N60X3L7-MBP ({SELF_L96} runs it)"),
            Some("assigned"),
        ),
        (10, 20, 17, 34, 0, K::RunStarted, Some(SELF_L96), "retry seat 6 on a spare host".into(), None),
        (10, 20, 17, 39, 0, K::ItemReleased, None, "released".into(), None),
    ];
    rows.extend(asked(10, 20, 22, 2, ASKED_1));
    rows.extend(asked(10, 20, 22, 16, ASKED_2));
    rows.push((
        10,
        20,
        22,
        41,
        0,
        K::Note,
        None,
        format!("answered: {ASKED_1}"),
        None,
    ));
    rows.push((
        10,
        20,
        23,
        5,
        0,
        K::ItemStatus,
        None,
        "in progress".into(),
        Some("in_progress"),
    ));
    rows.extend(asked(10, 20, 23, 20, ASKED_3));
    rows.push((
        10,
        20,
        23,
        45,
        0,
        K::Note,
        None,
        format!("answered: {ASKED_3}"),
        None,
    ));
    rows.push((
        10,
        21,
        0,
        10,
        0,
        K::ItemStatus,
        None,
        "blocked on the emitter fix".into(),
        Some("blocked"),
    ));
    rows.push((
        10,
        21,
        0,
        55,
        0,
        K::Handoff,
        None,
        format!("handoff to {SELF_FHKD}"),
        None,
    ));
    rows.push((
        10,
        21,
        1,
        30,
        0,
        K::Commit,
        None,
        "fix: emit tool-call JSON in reasoning format".into(),
        None,
    ));
    rows.push((
        10,
        21,
        1,
        42,
        0,
        K::Commit,
        None,
        "test: seat 6 reasoning-format regression".into(),
        None,
    ));
    rows.push((
        10,
        21,
        2,
        5,
        0,
        K::ItemStatus,
        None,
        "in progress".into(),
        Some("in_progress"),
    ));
    rows.push((
        10,
        21,
        2,
        40,
        0,
        K::RunStarted,
        None,
        "stage 4: re-run placement of seats 4 and 6".into(),
        None,
    ));
    rows.push((
        10,
        21,
        3,
        15,
        0,
        K::RunTool,
        None,
        "tool: placement.verify --all".into(),
        None,
    ));
    rows.push((
        10,
        21,
        3,
        50,
        0,
        K::RunFinished,
        None,
        "all 7 seats placed".into(),
        None,
    ));
    rows.extend(asked(10, 21, 4, 5, ASKED_4));
    rows.push((
        10,
        21,
        4,
        30,
        0,
        K::Note,
        None,
        format!("answered: {ASKED_4}"),
        None,
    ));
    rows.push((
        10,
        21,
        4,
        45,
        0,
        K::PrOpened,
        None,
        "PR #214 opened: emitter fix + capacity gate".into(),
        None,
    ));
    rows.push((
        10,
        21,
        5,
        20,
        0,
        K::PrReviewed,
        None,
        "PR #214 reviewed: approved".into(),
        None,
    ));
    rows.push((
        10,
        21,
        5,
        35,
        0,
        K::PrMerged,
        None,
        "PR #214 merged".into(),
        None,
    ));
    rows.push((
        10,
        21,
        6,
        0,
        0,
        K::ItemStatus,
        None,
        "review".into(),
        Some("review"),
    ));
    rows.push((
        10,
        21,
        6,
        40,
        0,
        K::Note,
        None,
        "verified placement on both hosts".into(),
        None,
    ));
    rows.push((
        10,
        21,
        7,
        10,
        0,
        K::ItemStatus,
        None,
        "done".into(),
        Some("done"),
    ));
    rows.push((
        10,
        21,
        7,
        15,
        0,
        K::ItemReleased,
        None,
        "released".into(),
        None,
    ));

    let mut run = 0;
    let events: Vec<Event> = rows
        .iter()
        .enumerate()
        .map(|(i, (m, d, h, mi, s, kind, agent, summary, status))| {
            let mut e = event(
                format!("{ITEM}:{i}"),
                at(*m, *d, *h, *mi, *s),
                *kind,
                *agent,
                Some(ITEM),
                summary,
            );
            if let Some(status) = status {
                e = e.status(*status);
            }
            if i == 0 {
                e = e.title("Raft seat scheduling");
            }
            if *kind == K::RunStarted {
                run += 1;
            }
            e = match kind {
                K::RunStarted | K::RunTool | K::RunThought | K::RunFinished | K::RunFailed => e
                    .run(format!("run-{ITEM}-{run}"))
                    .branch("fix/seat-placement"),
                K::Commit => e
                    .branch("fix/seat-placement")
                    .commit(format!("{:07x}", 0x3a91c0 + i * 0x1d3f)),
                K::PrOpened => e
                    .pr(214)
                    .branch("fix/seat-placement")
                    .commit("b7e04d1")
                    .title("Emitter fix + capacity gate")
                    .commits(["b7e04d1", "c2f9a58"]),
                K::PrReviewed | K::PrMerged => e.pr(214).branch("fix/seat-placement"),
                K::Handoff => e.from(SELF_L96),
                _ => e,
            };
            e
        })
        .collect();

    let mut tree = SessionTree {
        agent: Some("operator".to_owned()),
        ..SessionTree::default()
    };
    let lanes = [
        (
            OMP,
            at(10, 4, 3, 41, 0),
            at(10, 4, 4, 10, 0),
            at(10, 4, 4, 10, 0),
        ),
        (
            FHKD,
            at(10, 9, 8, 2, 0),
            at(10, 9, 13, 45, 0),
            at(10, 10, 22, 57, 0),
        ),
        (
            SELF_FHKD,
            at(10, 20, 3, 12, 0),
            at(10, 20, 11, 30, 0),
            at(10, 20, 17, 27, 0),
        ),
        (
            SELF_L96,
            at(10, 20, 17, 30, 0),
            at(10, 20, 17, 34, 0),
            at(10, 20, 17, 39, 0),
        ),
    ];
    for (label, from, to, merge) in lanes {
        push_lane(
            &mut tree,
            ITEM,
            LaneKind::Collab,
            label,
            from,
            to,
            Some(merge),
            None,
        );
    }
    Rail {
        events,
        item: ItemRef {
            id: ITEM.to_owned(),
            status: Some("done".to_owned()),
        },
        tree,
    }
}

/// Everything the swimlane graph and its toolbar read for the demo window.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Activity {
    /// Item events and loose events, oldest first, `seq` ascending.
    pub events: Vec<Event>,
    /// Fork lanes of the hero items, merged over the window.
    pub tree: SessionTree,
    /// Current status of each hero item.
    pub statuses: HashMap<String, String>,
    /// The clock to resolve windows against, epoch seconds ([`DEMO_NOW`]).
    pub now: i64,
}

struct Hero {
    id: &'static str,
    title: &'static str,
    status: &'static str,
    n: usize,
    first_min: i64,
    last_min: i64,
    host: &'static str,
    bursts: usize,
    red: bool,
}

const FHKD: &str = "FHKDV644W6-MBP";
const SELF_FHKD: &str = "selfdev@FHKDV644W6-MBP";

const fn hero(
    id: &'static str,
    status: &'static str,
    n: usize,
    first_min: i64,
    last_min: i64,
    bursts: usize,
) -> Hero {
    Hero {
        id,
        title: "",
        status,
        n,
        first_min,
        last_min,
        host: FHKD,
        bursts,
        red: false,
    }
}

/// The 20 items that are drawn under the default row cap, newest first by
/// design: all end within 48 h, every filler item is older.
fn heroes() -> [Hero; 20] {
    [
        Hero {
            title: "Ask failures after provider timeouts",
            red: true,
            ..hero("imp-45", "blocked", 70, 14_400, 20, 6)
        },
        Hero {
            title: "Assistant role: the human \u{2194} agents bridge",
            red: true,
            ..hero("imp-42", "claimed", 24, 15_000, 75, 3)
        },
        Hero {
            title: "Seat scheduling stage 2",
            ..hero("imp-41", "claimed", 18, 12_000, 140, 2)
        },
        Hero {
            title: "Placement ledger compaction",
            ..hero("imp-36", "claimed", 14, 9_000, 300, 2)
        },
        Hero {
            title: "Reasoning-format validator",
            ..hero("imp-32", "claimed", 20, 10_000, 520, 1)
        },
        Hero {
            title: "Selfdev runner heartbeat",
            host: SELF_FHKD,
            ..hero("imp-2100", "done", 12, 8_000, 700, 1)
        },
        Hero {
            title: "Selfdev lease renewal",
            host: SELF_FHKD,
            ..hero("imp-2088", "done", 10, 7_000, 950, 1)
        },
        Hero {
            title: "Ingest drops rows after a raft step-down",
            ..hero("imp-31", "open", 6, 15_000, 1_300, 0)
        },
        Hero {
            title: "Fan-out of ask events",
            ..hero("imp-252", "claimed", 3, 405, 400, 0)
        },
        Hero {
            title: "Fan-out of answer events",
            ..hero("imp-257", "claimed", 3, 455, 450, 0)
        },
        Hero {
            title: "Board filter line",
            ..hero("imp-46", "review", 14, 6_000, 160, 1)
        },
        Hero {
            title: "Legend tone filter",
            ..hero("imp-47", "review", 12, 5_000, 220, 1)
        },
        Hero {
            title: "Branch head strip",
            host: "gpu-07",
            ..hero("imp-48", "in_progress", 18, 7_000, 260, 0)
        },
        Hero {
            title: "Commit cluster glyph",
            host: "gpu-07",
            ..hero("imp-49", "in_progress", 12, 4_000, 600, 0)
        },
        Hero {
            title: "Idle gap hatching",
            host: "sched-02",
            ..hero("imp-50", "in_progress", 10, 3_000, 880, 0)
        },
        Hero {
            title: "Tree toggle elbows",
            host: "sched-02",
            ..hero("imp-53", "in_progress", 8, 2_500, 1_100, 0)
        },
        Hero {
            title: "Show-all row cap",
            ..hero("imp-54", "in_progress", 7, 2_200, 1_500, 0)
        },
        Hero {
            title: "Provider retry budget",
            ..hero("imp-55", "blocked", 6, 3_500, 1_900, 0)
        },
        Hero {
            title: "Dock register",
            ..hero("imp-56", "open", 4, 5_000, 2_300, 0)
        },
        Hero {
            title: "Collab thread table",
            ..hero("imp-57", "open", 3, 4_000, 2_700, 0)
        },
    ]
}

const MID_KINDS: [EventKind; 16] = [
    EventKind::Note,
    EventKind::RunStarted,
    EventKind::RunTool,
    EventKind::RunTool,
    EventKind::RunTool,
    EventKind::RunThought,
    EventKind::RunThought,
    EventKind::RunReasoning,
    EventKind::RunModel,
    EventKind::RunFinished,
    EventKind::Commit,
    EventKind::Commit,
    EventKind::PrOpened,
    EventKind::Handoff,
    EventKind::Heartbeat,
    EventKind::ItemClaimed,
];

fn say(rng: &mut Lcg, kind: EventKind) -> String {
    let pool: &[&str] = match kind {
        EventKind::Note => &[
            "checked the ingest path",
            "step-down reproduces on the second node",
            "waiting on a scheduler seat",
            "retry-backoff holds under load",
        ],
        EventKind::RunStarted => &[
            "plan: trace the ingest path",
            "plan: bisect the capacity gate",
            "plan: replay the ledger",
        ],
        EventKind::RunTool => &[
            "tool: cargo test -p ingest",
            "tool: rg step_down",
            "tool: placement.solve --seats 2",
            "tool: git diff main",
        ],
        EventKind::RunThought => &[
            "the leader term changes before the flush",
            "the capacity gate looks off by one",
        ],
        EventKind::RunReasoning => &["compare the retry schedule with the lease timeout"],
        EventKind::RunModel => &["model: plan step"],
        EventKind::RunFinished => &["run finished: tests green", "run finished: nothing to do"],
        EventKind::RunFailed => &["run failed: provider timeout"],
        EventKind::Commit => &[
            "fix: flush before the term change",
            "refactor: split the ledger reader",
            "test: step-down regression",
        ],
        EventKind::PrOpened => &["PR opened: ingest fix"],
        EventKind::Handoff => &["handoff to a free seat"],
        EventKind::Heartbeat => &["alive"],
        EventKind::ItemClaimed => &["claimed"],
        EventKind::ItemCreated => &["created"],
        _ => &[""],
    };
    (*rng.pick(pool)).to_owned()
}

/// Events of one hero item, oldest first, ids `<item>:<index>`.
fn hero_events(rng: &mut Lcg, h: &Hero) -> Vec<Event> {
    struct Draft {
        age: i64,
        kind: EventKind,
        agent: String,
        summary: String,
        status: Option<&'static str>,
        pr: Option<u64>,
    }
    let agents = [h.host, "omp", "clara", "worker", "selfdev"];
    let (first, last) = (h.first_min * 60, h.last_min * 60);

    let mut ages: Vec<i64> = (0..h.n)
        .map(|i| match i {
            0 => first,
            i if i + 1 == h.n => last,
            _ => last + rng.below((first - last) as u64) as i64,
        })
        .collect();
    ages.sort_unstable_by(|a, b| b.cmp(a));

    let live = h.status != "open";
    let mut drafts: Vec<Draft> = Vec::new();
    for (i, &age) in ages.iter().enumerate() {
        let last_one = i + 1 == h.n;
        let (kind, status) = if h.n == 3 && h.status == "claimed" {
            (
                [
                    EventKind::ItemClaimed,
                    EventKind::RunStarted,
                    EventKind::RunTool,
                ][i],
                None,
            )
        } else if i == 0 {
            (EventKind::ItemCreated, None)
        } else if i == 1 {
            (
                if live {
                    EventKind::ItemClaimed
                } else {
                    EventKind::Note
                },
                None,
            )
        } else if last_one {
            let kind = match h.status {
                "done" => EventKind::PrMerged,
                "blocked" => EventKind::RunFailed,
                "review" => EventKind::PrReviewed,
                "in_progress" => EventKind::RunTool,
                "claimed" => EventKind::RunThought,
                _ => EventKind::Note,
            };
            (kind, None)
        } else if i + 2 == h.n && live && h.n >= 4 {
            (EventKind::ItemStatus, Some(h.status))
        } else if live {
            (*rng.pick(&MID_KINDS), None)
        } else {
            (EventKind::Note, None)
        };
        let agent = match kind {
            EventKind::ItemCreated => "user".to_owned(),
            EventKind::ItemClaimed => h.host.to_owned(),
            _ => (*rng.pick(&agents)).to_owned(),
        };
        let mut summary = say(rng, kind);
        let mut pr = None;
        let mut agent = agent;
        if let Some(status) = status {
            summary = status.replace('_', " ");
        }
        if kind == EventKind::PrMerged {
            let n = 200 + rng.below(60);
            summary = format!("PR #{n} merged");
            pr = Some(n);
        }
        if last_one && h.id == "imp-45" {
            agent = "clara".to_owned();
            summary = "err clara: ask failed 17:25:58 provider timeout".to_owned();
        }
        drafts.push(Draft {
            age,
            kind,
            agent,
            summary,
            status,
            pr,
        });
    }

    for b in 0..h.bursts {
        let len = 3 + rng.below(7) as i64;
        let base = last + 600 + rng.below((first - last - 1_200) as u64) as i64;
        for j in 0..len {
            let kind = if h.red && b < 2 && j == 1 {
                EventKind::RunFailed
            } else if j % 3 == 2 {
                EventKind::RunThought
            } else {
                EventKind::RunTool
            };
            drafts.push(Draft {
                age: base - j * 4,
                kind,
                agent: (*rng.pick(&agents)).to_owned(),
                summary: say(rng, kind),
                status: None,
                pr: None,
            });
        }
    }
    // Oldest first; the sort is stable, so ties keep their draw order.
    drafts.sort_by_key(|d| std::cmp::Reverse(d.age));

    let branch = format!("fix/{}-ingest", h.id);
    let mut run = 0u32;
    drafts
        .into_iter()
        .enumerate()
        .map(|(i, d)| {
            let when = (DEMO_NOW - d.age) * SECOND;
            let mut e = event(
                format!("{}:{i}", h.id),
                when,
                d.kind,
                Some(&d.agent),
                Some(h.id),
                &d.summary,
            );
            if i <= 1 {
                e = e.title(h.title);
            }
            if let Some(status) = d.status {
                e = e.status(status);
            }
            if let Some(pr) = d.pr {
                e = e.pr(pr);
            }
            hero_context(e, h.id, &branch, i, &mut run)
        })
        .collect()
}

/// The facts a hero event carries beyond its summary, all derived from the
/// item and the event's index so the sample stays deterministic: the run it
/// belongs to, the branch and commit of code events, a pull request number
/// on review events and a few multi-line summaries.
fn hero_context(mut e: Event, item: &str, branch: &str, index: usize, run: &mut u32) -> Event {
    let seed = item
        .bytes()
        .chain(index.to_le_bytes())
        .fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
            (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
        });
    let kind = e.kind;
    match kind {
        EventKind::RunStarted => {
            *run += 1;
            e = e.run(format!("run-{item}-{run}"));
        }
        EventKind::RunTool
        | EventKind::RunThought
        | EventKind::RunReasoning
        | EventKind::RunModel
        | EventKind::RunFinished
        | EventKind::RunFailed => {
            e = e.run(format!("run-{item}-{}", (*run).max(1)));
        }
        EventKind::Commit => {
            e = e
                .branch(branch)
                .commit(format!("{:07x}", seed & 0x0fff_ffff));
            if index.is_multiple_of(2) {
                let first = e.summary.clone().unwrap_or_default();
                e = e.summary(format!(
                    "{first}\n\nThe flush ran after the leader term moved, so rows queued in the \
                     old term were dropped.\nFlush first, then step down."
                ));
            }
        }
        EventKind::PrOpened | EventKind::PrReviewed => {
            e = e.branch(branch).pr(200 + seed % 60);
        }
        EventKind::PrMerged => {
            e = e.branch(branch);
        }
        _ => {}
    }
    if kind == EventKind::RunFailed
        && index % 2 == 1
        && e.summary.as_deref() == Some("run failed: provider timeout")
    {
        e = e.summary("run failed: provider timeout\nprovider: anthropic, 3 of 3 attempts\nlast error: 504 after 30s");
    }
    e
}

/// Lane templates: kind, label (`{id}` and `{host}` filled in), verdict.
/// Labels with a `-<digits>`/`@` tail are host leaves, the rest agent leaves.
const LANE_TEMPLATES: [(LaneKind, &str, Option<bool>); 11] = [
    (LaneKind::Collab, "{host}", None),
    (LaneKind::Collab, "clara", None),
    (LaneKind::Branch, "fix/{id}-ingest", Some(true)),
    (LaneKind::Handoff, "selfdev@FHKDV644W6-MBP", None),
    (LaneKind::Path, "retry-backoff", Some(true)),
    (LaneKind::Path, "drop-queue", Some(false)),
    (LaneKind::Collab, "omp", None),
    (LaneKind::Handoff, "gpu-07", None),
    (LaneKind::Branch, "feat/assistant-bridge", Some(true)),
    (LaneKind::Handoff, "worker", None),
    (LaneKind::Collab, "sched-02", None),
];

fn hero_lanes(rng: &mut Lcg, index: usize, h: &Hero, times: &[Micros], tree: &mut SessionTree) {
    if index % 5 == 4 || times.len() < 3 {
        return;
    }
    let count = 1 + rng.below(3) as usize;
    for j in 0..count {
        let (kind, label, chosen) = LANE_TEMPLATES[(index * 3 + j * 4) % LANE_TEMPLATES.len()];
        let label = label.replace("{id}", h.id).replace("{host}", h.host);
        let a = rng.below((times.len() - 1) as u64) as usize;
        let b = a + 1 + rng.below((times.len() - 1 - a) as u64) as usize;
        let (from, to) = (times[a], times[b]);
        let merge = (chosen != Some(false) && kind != LaneKind::Handoff).then_some(to);
        push_lane(tree, h.id, kind, &label, from, to, merge, chosen);
    }
}

/// Age in seconds drawn uniformly over `[lo, hi)` with `hole` removed.
fn age_outside(rng: &mut Lcg, lo: i64, hi: i64, hole: (i64, i64)) -> i64 {
    let width = (hole.1.min(hi) - hole.0.max(lo)).max(0);
    let r = lo + rng.below((hi - lo - width) as u64) as i64;
    if r >= hole.0 && lo < hole.1 {
        r + width
    } else {
        r
    }
}

fn hex(rng: &mut Lcg) -> String {
    format!("{:08x}", rng.next_u64() & 0xffff_ffff)
}

/// Events [`activity`] holds: the size [`activity_sized`] reproduces it at.
pub const DEFAULT_EVENTS: usize = 7423;
/// Filler-item events at [`DEFAULT_EVENTS`]; what a larger size adds past
/// them grows a heavy tail of long-lived items.
const BASE_FILLER_EVENTS: usize = 5805;
/// Events outside the heroes at [`DEFAULT_EVENTS`]: the unit the loose and
/// filler shares scale by.
const BASE_REST_EVENTS: usize =
    BASE_FILLER_EVENTS + LOOSE_BRANCH_HEADS + LOOSE_COMMITS + LOOSE_PR_MERGED;

const BASE_AGENTS: [&str; 4] = ["omp", "clara", "worker", "selfdev"];
/// Past [`DEFAULT_EVENTS`] twelve agents share the work, three of them hosts.
const SCALED_AGENTS: [&str; 12] = [
    "omp",
    "clara",
    "worker",
    "selfdev",
    "scheduler",
    "operator",
    "planner",
    "reviewer",
    "indexer",
    "gpu-07",
    "sched-02",
    FHKD,
];
const BASE_BRANCHES: [&str; 5] = [
    "main",
    "fix/imp-16-ingest",
    "feat/assistant-bridge",
    "fix/imp-45-ask",
    "feat/seat-scheduling",
];

/// The five default branches and 36 `{stem}/{topic}` names.
fn scaled_branches() -> Vec<String> {
    const STEMS: [&str; 4] = ["fix", "feat", "refactor", "chore"];
    const TOPICS: [&str; 9] = [
        "ingest",
        "seat-scheduling",
        "assistant-bridge",
        "ask",
        "ledger",
        "validator",
        "heartbeat",
        "placement",
        "retry",
    ];
    let mut names: Vec<String> = BASE_BRANCHES.iter().map(|b| (*b).to_owned()).collect();
    for stem in STEMS {
        names.extend(TOPICS.iter().map(|topic| format!("{stem}/{topic}")));
    }
    names
}

/// A non-trunk branch name for the `j`-th branch of item number `n`; the
/// first four of an item are distinct.
fn topic(branches: &[String], n: usize, j: usize) -> &str {
    &branches[1 + (n * 13 + j * 7) % (branches.len() - 1)]
}

/// One event of a filler item's shape: what it is, who acts (0 is the item's
/// first agent, 1 to 3 the agents after it) and the branch and run it names.
#[derive(Clone, Copy)]
struct Beat {
    kind: EventKind,
    who: u8,
    branch: Option<u8>,
    run: Option<u8>,
}

const fn beat(kind: EventKind, who: u8, branch: Option<u8>, run: Option<u8>) -> Beat {
    Beat {
        kind,
        who,
        branch,
        run,
    }
}

const CREATED: Beat = beat(EventKind::ItemCreated, 0, None, None);
const NOTE_B: Beat = beat(EventKind::Note, 1, None, None);
const COMMIT_0: Beat = beat(EventKind::Commit, 0, Some(0), None);
const OPENED_0: Beat = beat(EventKind::PrOpened, 0, Some(0), None);
const MERGED_0: Beat = beat(EventKind::PrMerged, 0, Some(0), None);
const FAILED_0: Beat = beat(EventKind::RunFailed, 0, None, Some(0));
const FINISHED_1: Beat = beat(EventKind::RunFinished, 0, None, Some(1));

const COMMIT_ONLY: [Beat; 1] = [COMMIT_0];
const FAILED_ONLY: [Beat; 1] = [FAILED_0];
const HANDOFF_ONLY: [Beat; 1] = [beat(EventKind::Handoff, 1, None, None)];
const WITH_COLLAB: [Beat; 2] = [CREATED, NOTE_B];
const WITH_BRANCH: [Beat; 2] = [CREATED, OPENED_0];
const RETRY: [Beat; 2] = [FAILED_0, FINISHED_1];
const BRANCHES_HANDOFF: [Beat; 2] = [COMMIT_0, beat(EventKind::Handoff, 1, Some(1), None)];
const BRANCH_COLLAB: [Beat; 3] = [CREATED, COMMIT_0, NOTE_B];
const LANDED: [Beat; 3] = [CREATED, OPENED_0, MERGED_0];
const RETRY_COLLAB: [Beat; 3] = [
    beat(EventKind::RunStarted, 0, None, Some(0)),
    FAILED_0,
    beat(EventKind::RunFinished, 1, None, Some(1)),
];
const HANDED_OVER: [Beat; 3] = [
    CREATED,
    beat(EventKind::Handoff, 1, None, None),
    beat(EventKind::Commit, 1, Some(0), None),
];
const THREE_AGENTS: [Beat; 3] = [
    COMMIT_0,
    beat(EventKind::Handoff, 1, Some(1), None),
    beat(EventKind::Note, 2, None, None),
];
const MERGED_LATE: [Beat; 4] = [
    CREATED,
    OPENED_0,
    beat(EventKind::ItemClaimed, 0, None, None),
    MERGED_0,
];
const RETRY_ON_BRANCH: [Beat; 4] = [
    CREATED,
    beat(EventKind::RunFailed, 0, Some(0), Some(0)),
    beat(EventKind::RunFinished, 1, None, Some(1)),
    MERGED_0,
];
const FULL_STACK: [Beat; 4] = [
    COMMIT_0,
    beat(EventKind::Commit, 1, Some(1), None),
    beat(EventKind::Handoff, 2, Some(2), None),
    beat(EventKind::RunFailed, 3, Some(3), Some(0)),
];
const HAND_THEN_LAND: [Beat; 4] = [
    CREATED,
    NOTE_B,
    beat(EventKind::Handoff, 2, None, None),
    MERGED_0,
];
const RUNS_RETRIED: [Beat; 4] = [
    CREATED,
    FAILED_0,
    FINISHED_1,
    beat(EventKind::RunFailed, 0, None, Some(2)),
];
const ABANDONED: [Beat; 4] = [
    CREATED,
    beat(EventKind::PrOpened, 0, Some(1), None),
    OPENED_0,
    MERGED_0,
];

/// The shape the roll `0..100` gives a filler item of `count` events, or
/// `None` for a plain one that derives no lane. About 56% of items get lanes
/// and 20% get three or more; one in seventy keeps the eight the cap allows.
fn shape(count: usize, roll: u64) -> Option<&'static [Beat]> {
    let shape: &'static [Beat] = match (count, roll) {
        (1, 65..=79) => &COMMIT_ONLY,
        (1, 80..=91) => &FAILED_ONLY,
        (1, 92..) => &HANDOFF_ONLY,
        (2, 45..=59) => &WITH_COLLAB,
        (2, 60..=74) => &WITH_BRANCH,
        (2, 75..=91) => &RETRY,
        (2, 92..) => &BRANCHES_HANDOFF,
        (3, 35..=49) => &BRANCH_COLLAB,
        (3, 50..=61) => &LANDED,
        (3, 62..=74) => &RETRY_COLLAB,
        (3, 75..=88) => &HANDED_OVER,
        (3, 89..) => &THREE_AGENTS,
        (4, 30..=41) => &MERGED_LATE,
        (4, 42..=55) => &RETRY_ON_BRANCH,
        (4, 56..=61) => &FULL_STACK,
        (4, 62..=75) => &HAND_THEN_LAND,
        (4, 76..=89) => &RUNS_RETRIED,
        (4, 90..) => &ABANDONED,
        _ => return None,
    };
    Some(shape)
}

/// Long-lived items past the fillers. Sizes follow a heavy tail: a few carry
/// thousands of events, most tens. Their own generator leaves the main
/// stream, and so [`activity`], untouched. Each has up to five branches,
/// three agents and runs that fail and are retried, so the busiest overflow
/// the eight lanes an item keeps.
fn tail_events(budget: usize, branches: &[String], out: &mut Vec<Event>) {
    let mut rng = Lcg::new(SEED ^ 0x7A11);
    let margin = 3 * 3600;
    // Ages on either side of the idle days, older than the heroes.
    let sides = [
        (2 * DAY + margin, 12 * DAY),
        (14 * DAY + margin, 30 * DAY - 1),
    ];
    let (mut left, mut t) = (budget, 0usize);
    while left > 0 {
        let size = (5 + 65_536 / (1 + rng.below(4096) as usize)).min(left);
        let (lo, hi) = sides[rng.below(2) as usize];
        let span = (size as i64 * (20 + rng.below(600) as i64)).min(hi - lo - 1);
        let oldest = lo + span + rng.below((hi - lo - span) as u64) as i64;
        let id = format!("imp-{}", 1_000_000 + t);
        let agent = *rng.pick(&SCALED_AGENTS);
        let helper = *rng.pick(&SCALED_AGENTS);
        let third = *rng.pick(&SCALED_AGENTS);
        let topics = 1 + rng.below(5) as usize;
        let (mut run, mut runs) = (None::<u32>, 0u32);
        for k in 0..size {
            let mut kind = match k {
                0 => EventKind::ItemCreated,
                _ if k == size / 2 && helper != agent => EventKind::Handoff,
                _ if k + 1 == size => EventKind::PrMerged,
                _ => *rng.pick(&MID_KINDS),
            };
            if kind == EventKind::Handoff && k != size / 2 {
                kind = EventKind::Note;
            }
            if kind == EventKind::RunFinished && rng.below(10) < 3 {
                kind = EventKind::RunFailed;
            }
            let who = if k % 7 == 5 {
                third
            } else if k >= size / 2 {
                helper
            } else {
                agent
            };
            let age = oldest - span * k as i64 / size as i64;
            let summary = say(&mut rng, kind);
            let mut e = event(
                format!("{id}:{k}"),
                (DEMO_NOW - age) * SECOND,
                kind,
                Some(who),
                Some(&id),
                &summary,
            );
            match kind {
                EventKind::Handoff => e = e.from(agent),
                EventKind::PrMerged => {
                    let pr = 1 + rng.below(250);
                    e = e
                        .pr(pr)
                        .summary(format!("PR #{pr} merged"))
                        .branch(topic(branches, t, 0));
                }
                EventKind::Commit | EventKind::PrOpened => {
                    e = e.branch(topic(branches, t, k % topics));
                }
                EventKind::RunStarted => {
                    runs += 1;
                    run = Some(runs);
                }
                _ => {}
            }
            let in_run = matches!(
                kind,
                EventKind::RunStarted
                    | EventKind::RunTool
                    | EventKind::RunThought
                    | EventKind::RunReasoning
                    | EventKind::RunModel
                    | EventKind::RunFinished
                    | EventKind::RunFailed
            );
            if let (true, Some(r)) = (in_run, run) {
                e = e.run(format!("run-{id}-{r}"));
                if matches!(kind, EventKind::RunFinished | EventKind::RunFailed) {
                    run = None;
                }
            }
            out.push(e);
        }
        left -= size;
        t += 1;
    }
}

/// Oldest first, ties by id, `seq` ascending. Sorts 16-byte keys and moves
/// each event once instead of sorting events of a few hundred bytes.
fn order_events(events: &mut [Event]) {
    let mut keys: Vec<(Micros, u32)> = events
        .iter()
        .enumerate()
        .map(|(i, e)| (e.at, i as u32))
        .collect();
    keys.sort_unstable_by(|a, b| {
        a.0.cmp(&b.0)
            .then_with(|| events[a.1 as usize].id.cmp(&events[b.1 as usize].id))
    });
    // `at[orig]` is where an event sits now, `was[slot]` which event holds a slot.
    let mut at: Vec<u32> = (0..events.len() as u32).collect();
    let mut was = at.clone();
    for (slot, &(_, want)) in keys.iter().enumerate() {
        let from = at[want as usize] as usize;
        if from != slot {
            events.swap(slot, from);
            let displaced = was[slot];
            was[from] = displaced;
            at[displaced as usize] = from as u32;
            was[slot] = want;
            at[want as usize] = slot as u32;
        }
    }
    for (seq, e) in events.iter_mut().enumerate() {
        e.seq = seq as u64;
    }
}

/// 30 days of activity ending at [`DEMO_NOW`]; see the module docs.
pub fn activity() -> Activity {
    activity_sized(DEFAULT_EVENTS)
}

/// [`activity`] with `events` events: the 20 heroes (395 events, whatever the
/// size), then the rest split in the proportions of the default sample into
/// loose branch heads, commits and merged pull requests and filler items.
///
/// At [`DEFAULT_EVENTS`] it is [`activity`] exactly; past it twelve agents
/// share the work, every third filler item changes hands, 41 branch names
/// appear and an eighth of the filler events go to a few long-lived items.
/// Below 395 the heroes alone come back. Deterministic, 30 days, idle days
/// 12 and 13 kept empty.
pub fn activity_sized(events: usize) -> Activity {
    let scaled = events > DEFAULT_EVENTS;
    let mut rng = Lcg::new(SEED);
    let mut out: Vec<Event> = Vec::with_capacity(events.max(DEFAULT_EVENTS));
    let mut tree = SessionTree::default();
    let mut statuses = HashMap::new();

    let heroes = heroes();
    for (index, h) in heroes.iter().enumerate() {
        let evs = hero_events(&mut rng, h);
        let times: Vec<Micros> = evs.iter().map(|e| e.at).collect();
        hero_lanes(&mut rng, index, h, &times, &mut tree);
        statuses.insert(h.id.to_owned(), h.status.to_owned());
        out.extend(evs);
    }

    let rest = events.saturating_sub(out.len());
    let share = |n: usize| (n as u64 * rest as u64 / BASE_REST_EVENTS as u64) as usize;
    let loose = [
        (EventKind::BranchHead, share(LOOSE_BRANCH_HEADS), "branch"),
        (EventKind::Commit, share(LOOSE_COMMITS), "commit"),
        (EventKind::PrMerged, share(LOOSE_PR_MERGED), "pr"),
    ];
    let filler = rest - loose.iter().map(|l| l.1).sum::<usize>();
    let tail = if scaled {
        filler.saturating_sub(BASE_FILLER_EVENTS) / 8
    } else {
        0
    };
    let agents: &[&str] = if scaled { &SCALED_AGENTS } else { &BASE_AGENTS };
    let branches: Vec<String> = if scaled {
        scaled_branches()
    } else {
        BASE_BRANCHES.iter().map(|b| (*b).to_owned()).collect()
    };

    // Filler items: older than 48 h, spread over days 2 to 30 around the idle
    // days 12 and 13. Up to three follow-up events step at most 50 min each
    // toward now, so a 3 h margin keeps every event of an item off the edges.
    // The random stream decides each item's size, time and kinds whatever its
    // shape; the shape comes from a stream of its own.
    let margin = 3 * 3600;
    let mut styles = Lcg::new(SEED ^ 0x57D1);
    let (mut left, mut n) = (filler - tail, 0usize);
    while left > 0 {
        let id = format!("imp-{}", 10_000 + n);
        let count = (1 + rng.below(4) as usize).min(left);
        let hole = (12 * DAY, 14 * DAY + margin);
        let mut age = age_outside(&mut rng, 2 * DAY + margin, 30 * DAY - 1, hole);
        let agent = *rng.pick(agents);
        let plan = shape(count, styles.below(100));
        let first = agents.iter().position(|a| *a == agent).unwrap_or(0);
        let mut holder = agent;
        for k in 0..count {
            let beat = plan.and_then(|beats| beats.get(k));
            let drawn = if k == 0 {
                EventKind::ItemCreated
            } else {
                *rng.pick(&[
                    EventKind::ItemClaimed,
                    EventKind::RunStarted,
                    EventKind::RunFinished,
                    EventKind::PrOpened,
                    EventKind::PrMerged,
                    EventKind::Note,
                ])
            };
            let kind = beat.map_or(drawn, |b| b.kind);
            let summary = say(&mut rng, kind);
            let who = beat.map_or(agent, |b| agents[(first + b.who as usize) % agents.len()]);
            let mut e = event(
                format!("{id}:{k}"),
                (DEMO_NOW - age) * SECOND,
                kind,
                Some(who),
                Some(&id),
                &summary,
            );
            let drawn_pr = (drawn == EventKind::PrMerged).then(|| 1 + rng.below(250));
            if kind == EventKind::PrMerged {
                let pr = drawn_pr.unwrap_or_else(|| 1 + styles.below(250));
                e = e.pr(pr).summary(format!("PR #{pr} merged"));
            }
            if let Some(b) = beat {
                if let Some(j) = b.branch {
                    e = e.branch(topic(&branches, n, j as usize));
                }
                if let Some(r) = b.run {
                    e = e.run(format!("run-{id}-{r}"));
                }
                if kind == EventKind::Handoff {
                    e = e.from(holder);
                }
            }
            holder = who;
            out.push(e);
            age -= 5 + rng.below(50) as i64 * 60;
        }
        left -= count;
        n += 1;
    }
    tail_events(tail, &branches, &mut out);

    // Loose events: no item, same span, nothing on days 12 and 13.
    for (kind, count, prefix) in loose {
        for n in 0..count {
            let age = age_outside(&mut rng, 0, 30 * DAY - 1, (12 * DAY, 14 * DAY));
            let agent = match rng.below(10) {
                0 => Some("clara"),
                1 => Some("omp"),
                _ => None,
            };
            let sha = hex(&mut rng);
            let branch = rng.pick(&branches);
            let summary = match kind {
                EventKind::BranchHead => format!("branch {branch} @ {sha}"),
                EventKind::Commit => format!("fix: step {n}"),
                _ => format!("PR #{} merged", 1 + n),
            };
            let mut e = event(
                format!("{prefix}:{n}"),
                (DEMO_NOW - age) * SECOND,
                kind,
                agent,
                None,
                &summary,
            );
            e = match kind {
                EventKind::PrMerged => e.pr(1 + n as u64),
                EventKind::Commit => e.commit(sha),
                _ => e.branch(branch).commit(sha),
            };
            out.push(e);
        }
    }

    order_events(&mut out);
    for lane in &tree.lanes {
        debug_assert!(lane.from <= lane.to);
    }
    Activity {
        events: out,
        tree,
        statuses,
        now: DEMO_NOW,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeSet, HashSet};

    use super::*;
    use crate::widgets::trace::query::{GraphState, Tone, Window};
    use crate::widgets::trace::rails::{self, Stamp};
    use crate::widgets::trace::swimlane::{derive_forks, view, Graph, Options, Range, FORK_CAP};

    fn state() -> GraphState {
        GraphState {
            window: "30d".to_owned(),
            tree: true,
            ..GraphState::default()
        }
    }

    fn drawn(activity: &Activity, state: &GraphState) -> Graph {
        view::graph(&activity.events, state, Some(&activity.tree), activity.now)
    }

    #[test]
    fn demo_now_constant() {
        assert_eq!(DEMO_NOW, days_from_civil(2026, 10, 20) * DAY + 12 * 3600);
    }

    #[test]
    fn samples_are_deterministic() {
        assert_eq!(session_tree(), session_tree());
        assert_eq!(item_timeline(), item_timeline());
        assert_eq!(activity(), activity());
    }

    #[test]
    fn session_tree_is_seven_events_six_lanes() {
        let rail = session_tree();
        assert_eq!(rail.events.len(), 7);
        assert_eq!(rail.tree.lanes.len(), 6);
        let graph = rails::vertical(&rail.events, &rail.item, &rail.tree);
        assert_eq!(graph.header(), "7 events \u{b7} 6 lanes");
        assert_eq!(graph.item.rows.len(), 8);
        assert_eq!(graph.item.forks.len(), 6);
        assert_eq!(graph.stamp, Stamp::DayTime);
        let tones: HashSet<Tone> = graph.item.rows.iter().map(|r| r.tone).collect();
        assert!(tones.contains(&Tone::Blue) && tones.contains(&Tone::Yellow));
        assert!(tones.contains(&Tone::Hollow));
        let chips: Vec<_> = graph
            .item
            .rows
            .iter()
            .filter_map(|r| r.lane.as_deref())
            .collect();
        assert!(chips.contains(&"user") && chips.contains(&"clara") && chips.contains(&"operator"));
        assert!(graph.item.rows.iter().any(|r| !r.selectable));
    }

    #[test]
    fn rail_samples_carry_facts_worth_expanding() {
        use crate::widgets::trace::facts;
        for rail in [session_tree(), item_timeline()] {
            let labels: BTreeSet<&str> = rail
                .events
                .iter()
                .flat_map(|e| facts::event_facts(e, facts::WRAP_COLS))
                .map(|f| f.label)
                .collect();
            for want in ["run", "branch", "commit", "from", "title", "text"] {
                assert!(labels.contains(want), "{want} missing in {labels:?}");
            }
            let tall = rail
                .events
                .iter()
                .filter(|e| facts::line_count(&facts::event_facts(e, facts::WRAP_COLS)) >= 8)
                .count();
            assert!(tall >= 1, "no multi-line event");
        }
        assert!(item_timeline().events.iter().any(|e| e.pr == Some(214)));
    }

    #[test]
    fn timeline_is_fifty_events_four_lanes() {
        let rail = item_timeline();
        assert_eq!(rail.events.len(), 50);
        assert_eq!(rail.tree.lanes.len(), 4);
        let graph = rails::vertical(&rail.events, &rail.item, &rail.tree);
        assert_eq!(graph.header(), "50 events \u{b7} 4 lanes");
        assert_eq!(graph.item.rows.len(), 50);
        assert_eq!(graph.stamp, Stamp::DayTime);
        let dates: BTreeSet<i64> = rail.events.iter().map(|e| e.at / (DAY * SECOND)).collect();
        assert!(dates.len() >= 3);
        assert!(graph.item.rows.windows(2).all(|w| w[0].at < w[1].at));
        let tones: HashSet<Tone> = graph.item.rows.iter().map(|r| r.tone).collect();
        assert_eq!(tones.len(), 5);
        // Every lane names its first row once; chips follow the screenshot.
        let chips: Vec<_> = graph
            .item
            .rows
            .iter()
            .filter_map(|r| r.lane.as_deref())
            .collect();
        assert_eq!(chips.len(), 4);
    }

    #[test]
    fn swimlane_matches_the_reference_counts() {
        let a = activity();
        let loose = |k: EventKind| {
            a.events
                .iter()
                .filter(|e| e.item_id.is_none() && e.kind == k)
                .count()
        };
        assert_eq!(loose(EventKind::BranchHead), 758);
        assert_eq!(loose(EventKind::Commit), 253);
        assert_eq!(loose(EventKind::PrMerged), 212);
        for id in ["imp-31", "imp-45", "imp-252", "imp-257"] {
            assert!(
                a.events.iter().any(|e| e.item_id.as_deref() == Some(id)),
                "{id}"
            );
        }
        let ids: HashSet<&str> = a.events.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids.len(), a.events.len());

        let g = drawn(&a, &state());
        assert_eq!(g.total_items, 2351);
        assert_eq!(g.items.len(), 20);
        assert_eq!(g.hidden, g.total_items - g.items.len());
        assert_eq!(g.hidden, 2331);
        let strip: Vec<_> = g.strip.iter().map(|r| (r.kind, r.count)).collect();
        assert_eq!(
            strip,
            [
                (EventKind::BranchHead, 758),
                (EventKind::Commit, 253),
                (EventKind::PrMerged, 212)
            ]
        );
        assert!(
            g.gaps.iter().any(|gap| gap.ms >= 2 * DAY * 1000),
            "{:?}",
            g.gaps
        );

        let all = drawn(
            &a,
            &GraphState {
                show_all: true,
                ..state()
            },
        );
        assert_eq!(all.item_count(), 2351);
        assert_eq!(all.hidden, 0);
    }

    #[test]
    fn swimlane_draws_bursts_forks_and_every_tone() {
        let a = activity();
        let g = drawn(&a, &state());
        let dots = g.items.iter().flat_map(|i| &i.dots);
        assert!(dots.clone().any(|d| d.count >= 2));
        assert!(dots.clone().any(|d| d.count >= 2 && d.tone == Tone::Red));
        let tones: BTreeSet<Tone> = dots.map(|d| d.tone).collect();
        assert_eq!(tones.len(), 5, "{tones:?}");
        assert!(g.items.iter().filter(|i| !i.forks.is_empty()).count() >= 12);
        let kinds: HashSet<_> = g
            .items
            .iter()
            .flat_map(|i| &i.forks)
            .map(|f| f.kind)
            .collect();
        assert_eq!(kinds.len(), 4);
        assert!(g.items.iter().flat_map(|i| &i.forks).any(|f| f.dead));
        let is_host = |l: &str| {
            l.contains('@')
                || l.rsplit_once('-')
                    .is_some_and(|(_, t)| t.chars().next().is_some_and(|c| c.is_ascii_digit()))
        };
        let labels: Vec<&str> = g
            .items
            .iter()
            .flat_map(|i| &i.forks)
            .map(|f| f.label.as_str())
            .collect();
        assert!(labels.iter().any(|l| is_host(l)) && labels.iter().any(|l| !is_host(l)));
    }

    #[test]
    fn every_event_lies_in_the_thirty_day_window() {
        let a = activity();
        let window = Window::parse("30d").unwrap().resolve(DEMO_NOW);
        assert!(a.events.iter().all(|e| window.contains(e.at / SECOND)));
        assert!(a.events.iter().all(|e| e.at <= DEMO_NOW * SECOND));
        let g = Graph::layout(
            a.events.iter(),
            &Options::new(1200, Range::from_secs(DEMO_NOW - 30 * DAY, None, DEMO_NOW))
                .tree(&a.tree)
                .statuses(&a.statuses),
        );
        assert_eq!(g.total_items, 2351);
    }

    fn item_number(e: &Event) -> Option<usize> {
        e.item_id.as_deref()?.strip_prefix("imp-")?.parse().ok()
    }

    #[test]
    fn the_default_size_is_the_default_sample() {
        assert_eq!(activity().events.len(), DEFAULT_EVENTS);
        assert_eq!(activity_sized(DEFAULT_EVENTS), activity());
    }

    #[test]
    fn sized_samples_hold_exactly_the_events_asked_for() {
        let idle = 12 * DAY..14 * DAY;
        for size in [395, 1_000, DEFAULT_EVENTS - 1, DEFAULT_EVENTS + 1, 25_000] {
            let a = activity_sized(size);
            assert_eq!(a.events.len(), size, "{size}");
            for (i, pair) in a.events.windows(2).enumerate() {
                let (x, y) = (&pair[0], &pair[1]);
                assert!((x.at, &x.id) < (y.at, &y.id), "{size}: order at {i}");
            }
            assert!(a.events.iter().enumerate().all(|(i, e)| e.seq == i as u64));
            for e in &a.events {
                let age = DEMO_NOW - e.at / SECOND;
                assert!((0..30 * DAY).contains(&age), "{size}: {} age {age}", e.id);
                assert!(
                    !idle.contains(&age),
                    "{size}: {} sits in the idle days",
                    e.id
                );
            }
        }
    }

    #[test]
    fn sized_samples_are_deterministic() {
        assert_eq!(activity_sized(30_000), activity_sized(30_000));
        assert_ne!(activity_sized(30_000), activity_sized(30_001));
    }

    #[test]
    fn sizes_below_the_heroes_come_back_as_the_heroes_alone() {
        let a = activity_sized(10);
        assert_eq!(a.events.len(), 395);
        assert!(a
            .events
            .iter()
            .all(|e| item_number(e).is_some_and(|n| n < 10_000)));
        assert_eq!(a.statuses.len(), 20);
    }

    #[test]
    fn larger_samples_keep_the_heroes_the_default_view_draws() {
        let heroes = |a: &Activity| -> Vec<Event> {
            a.events
                .iter()
                .filter(|e| item_number(e).is_some_and(|n| n < 10_000))
                .cloned()
                .map(|e| Event { seq: 0, ..e })
                .collect()
        };
        let (small, big) = (activity(), activity_sized(40_000));
        assert_eq!(heroes(&small), heroes(&big));
        assert_eq!(small.tree, big.tree);

        let ids = |a: &Activity| -> Vec<String> {
            let mut ids: Vec<String> = drawn(a, &state()).items.into_iter().map(|i| i.id).collect();
            ids.sort();
            ids
        };
        assert_eq!(ids(&small).len(), 20);
        assert_eq!(ids(&small), ids(&big));
    }

    #[test]
    fn larger_samples_look_like_a_busy_ledger() {
        use std::collections::HashMap;
        let a = activity_sized(60_000);
        let loose = a.events.iter().filter(|e| e.item_id.is_none()).count();
        assert!((9_000..12_000).contains(&loose), "{loose} loose");

        let mut per_item: HashMap<&str, usize> = HashMap::new();
        for e in &a.events {
            if let Some(item) = e.item_id.as_deref() {
                *per_item.entry(item).or_default() += 1;
            }
        }
        let items = per_item.len();
        assert!((60_000 / 5..60_000 / 2).contains(&items), "{items} items");
        let longest = per_item.values().copied().max().unwrap_or(0);
        assert!(longest >= 200, "longest item has {longest} events");
        assert!(per_item.values().filter(|n| **n <= 4).count() * 10 >= items * 8);

        let agents: BTreeSet<&str> = a.events.iter().filter_map(|e| e.agent.as_deref()).collect();
        assert!(agents.len() >= 12, "{agents:?}");
        let branches: BTreeSet<&str> = a
            .events
            .iter()
            .filter_map(|e| e.branch.as_deref())
            .collect();
        assert!(branches.len() >= 36, "{} branches", branches.len());
        let handoffs = a
            .events
            .iter()
            .filter(|e| e.kind == EventKind::Handoff && e.detail.from.is_some())
            .count();
        assert!(
            handoffs * 10 >= items,
            "{handoffs} handoffs over {items} items"
        );
    }

    /// What each item draws with the tree on: the explicit lanes the sample
    /// holds for it, else the ones its events derive.
    fn lanes_by_item(a: &Activity) -> HashMap<&str, Vec<Lane>> {
        let mut events: HashMap<&str, Vec<&Event>> = HashMap::new();
        for e in &a.events {
            if let Some(item) = e.item_id.as_deref() {
                events.entry(item).or_default().push(e);
            }
        }
        events
            .into_iter()
            .map(|(item, events)| {
                let own: Vec<Lane> = a
                    .tree
                    .lanes
                    .iter()
                    .filter(|lane| lane.item_id == item)
                    .cloned()
                    .collect();
                let lanes = if own.is_empty() {
                    derive_forks(item, &events).0
                } else {
                    own
                };
                (item, lanes)
            })
            .collect()
    }

    fn item_number_of(id: &str) -> usize {
        id.strip_prefix("imp-")
            .and_then(|n| n.parse().ok())
            .unwrap_or(0)
    }

    #[test]
    fn the_tree_is_derived_for_the_filler_at_every_size() {
        for size in [DEFAULT_EVENTS, 30_000, 100_000] {
            let a = activity_sized(size);
            let lanes = lanes_by_item(&a);
            let items = lanes.len() as f64;
            let share = |at_least: usize| {
                lanes.values().filter(|l| l.len() >= at_least).count() as f64 / items
            };
            eprintln!(
                "{size} events: {:.1}% of {} items with a lane, {:.1}% with three or more",
                100.0 * share(1),
                lanes.len(),
                100.0 * share(3)
            );
            assert!(share(1) >= 0.45, "{size}: {:.3} with a lane", share(1));
            assert!(share(3) >= 0.12, "{size}: {:.3} with three", share(3));
            assert!(
                lanes.values().any(|l| l.len() == FORK_CAP),
                "{size}: no item keeps eight lanes"
            );
            assert!(lanes.values().all(|l| l.len() <= FORK_CAP));
            let all: Vec<&Lane> = lanes.values().flatten().collect();
            let kinds: HashSet<LaneKind> = all.iter().map(|lane| lane.kind).collect();
            assert_eq!(kinds.len(), 4, "{size}: {kinds:?}");
            let retries = all
                .iter()
                .filter(|lane| lane.chosen == Some(true) && lane.parent.is_some())
                .count();
            assert!(retries >= 10, "{size}: {retries} retried runs");
            assert!(all.iter().any(|lane| lane.chosen == Some(false)));
        }
    }

    #[test]
    fn the_heroes_keep_their_explicit_lanes_and_the_filler_derives_its_own() {
        let a = activity();
        assert!(!a.tree.lanes.is_empty());
        assert!(a
            .tree
            .lanes
            .iter()
            .all(|lane| item_number_of(&lane.item_id) < 10_000));
        let lanes = lanes_by_item(&a);
        for item in a.tree.lanes.iter().map(|lane| lane.item_id.as_str()) {
            let own = a.tree.lanes.iter().filter(|l| l.item_id == item).count();
            assert_eq!(lanes[item].len(), own, "{item}");
        }
        let forked = lanes
            .iter()
            .filter(|(item, lanes)| item_number_of(item) >= 10_000 && !lanes.is_empty())
            .count();
        assert!(forked > 1_000, "{forked} filler items fork");
    }

    #[test]
    fn the_default_sample_draws_far_more_forks_than_its_heroes_hold() {
        let a = activity();
        let all = drawn(
            &a,
            &GraphState {
                show_all: true,
                ..state()
            },
        );
        let forked = (0..all.item_count())
            .filter(|&at| {
                !all.item(&a.events, all.row_spans()[at].item)
                    .forks
                    .is_empty()
            })
            .count();
        assert!(
            forked > 10 * a.tree.lanes.len(),
            "{forked} items fork, the heroes hold {} lanes",
            a.tree.lanes.len()
        );
        let off = drawn(
            &a,
            &GraphState {
                show_all: true,
                tree: false,
                ..state()
            },
        );
        assert_eq!(off.rows, off.lanes);
        assert!(all.rows > off.rows);
    }
}
