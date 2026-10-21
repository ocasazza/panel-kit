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
//!   and fork lanes ending in agent and host leaves.

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
            "reviewed the ingest path after the raft step-down",
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
        if i == 0 {
            e = e.title("Ingest drops rows after a raft step-down");
        }
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
            "SEAT SCHEDULING STAGE 1 \u{2014} investigation, read-only. No code changed. Two Raft seats stay unplaced because the embedding capacity gate rejects them.".into(),
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
            if i == 0 {
                e = e.title(h.title);
            }
            if let Some(status) = d.status {
                e = e.status(status);
            }
            if let Some(pr) = d.pr {
                e = e.pr(pr);
            }
            e
        })
        .collect()
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

/// 30 days of activity ending at [`DEMO_NOW`]; see the module docs.
pub fn activity() -> Activity {
    let mut rng = Lcg::new(SEED);
    let mut events: Vec<Event> = Vec::new();
    let mut tree = SessionTree::default();
    let mut statuses = HashMap::new();

    let heroes = heroes();
    for (index, h) in heroes.iter().enumerate() {
        let evs = hero_events(&mut rng, h);
        let times: Vec<Micros> = evs.iter().map(|e| e.at).collect();
        hero_lanes(&mut rng, index, h, &times, &mut tree);
        statuses.insert(h.id.to_owned(), h.status.to_owned());
        events.extend(evs);
    }

    // Filler items: older than 48 h, spread over days 2 to 30 around the idle
    // days 12 and 13. Up to three follow-up events step at most 50 min each
    // toward now, so a 3 h margin keeps every event of an item off the edges.
    let agents = ["omp", "clara", "worker", "selfdev"];
    let margin = 3 * 3600;
    for n in 0..TOTAL_ITEMS - heroes.len() {
        let id = format!("imp-{}", 10_000 + n);
        let count = 1 + rng.below(4) as usize;
        let hole = (12 * DAY, 14 * DAY + margin);
        let mut age = age_outside(&mut rng, 2 * DAY + margin, 30 * DAY - 1, hole);
        let agent = *rng.pick(&agents);
        for k in 0..count {
            let kind = if k == 0 {
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
            let summary = say(&mut rng, kind);
            let mut e = event(
                format!("{id}:{k}"),
                (DEMO_NOW - age) * SECOND,
                kind,
                Some(agent),
                Some(&id),
                &summary,
            );
            if kind == EventKind::PrMerged {
                let pr = 1 + rng.below(250);
                e = e.pr(pr).summary(format!("PR #{pr} merged"));
            }
            events.push(e);
            age -= 5 + rng.below(50) as i64 * 60;
        }
    }

    // Loose events: no item, same span, nothing on days 12 and 13.
    const BRANCHES: [&str; 5] = [
        "main",
        "fix/imp-16-ingest",
        "feat/assistant-bridge",
        "fix/imp-45-ask",
        "feat/seat-scheduling",
    ];
    let loose = [
        (EventKind::BranchHead, LOOSE_BRANCH_HEADS, "branch"),
        (EventKind::Commit, LOOSE_COMMITS, "commit"),
        (EventKind::PrMerged, LOOSE_PR_MERGED, "pr"),
    ];
    for (kind, count, prefix) in loose {
        for n in 0..count {
            let age = age_outside(&mut rng, 0, 30 * DAY - 1, (12 * DAY, 14 * DAY));
            let agent = match rng.below(10) {
                0 => Some("clara"),
                1 => Some("omp"),
                _ => None,
            };
            let sha = hex(&mut rng);
            let branch = *rng.pick(&BRANCHES);
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
            events.push(e);
        }
    }

    events.sort_by(|a, b| a.at.cmp(&b.at).then_with(|| a.id.cmp(&b.id)));
    for (seq, e) in events.iter_mut().enumerate() {
        e.seq = seq as u64;
    }
    for lane in &tree.lanes {
        debug_assert!(lane.from <= lane.to);
    }
    Activity {
        events,
        tree,
        statuses,
        now: DEMO_NOW,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeSet, HashSet};

    use super::*;
    use crate::widgets::event_graph::query::{GraphState, Tone, Window};
    use crate::widgets::event_graph::rails::{self, Stamp};
    use crate::widgets::event_graph::swimlane::{view, Graph, Options, Range};

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
        assert_eq!(all.items.len(), 2351);
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
}
