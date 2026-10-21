//! Table demo: the board's work ledger in the one table component, from 267
//! rows to a million.
//!
//! Run with: `dx serve --example table --platform web`
//!
//! A tag ribbon in the gutter, a bold id with its priority, layered status
//! badges, finished rows dimmed, rows grouped by state, typed column filters,
//! multi-column sort, drag-to-reorder and drag-to-resize headers, density,
//! cards, a virtual window, copy as TSV and CSV download, and the layout
//! persisted in localStorage. The size buttons rebuild the data and report
//! how long building it took. `?stress` runs `support/table_stress.js`: a
//! million rows driven through every interaction against per-step budgets,
//! with the result in `#stress` (`data-pass`).

use std::fmt::Write as _;
use std::rc::Rc;

use dioxus::prelude::*;
use panel_kit::badge::Badge;
use panel_kit::widgets::{DataColumnSpec, DataTable, TableData, TableDataBuilder, Value};
use panel_kit::CSS;
use panel_kit_core::badge::{BadgeKind, BadgeSpec, BadgeTone, BadgeVariant};
use panel_kit_core::widgets::data_table::{Selectable, TableDefaults};

const DEMO_CSS: &str = "
body { overflow:hidden !important; }
.demo { height:100vh; box-sizing:border-box; padding:1.5rem 2rem; display:flex; flex-direction:column; gap:.5rem; }
.demo h1 { margin:0; font:700 11px/1.4 var(--mono); letter-spacing:.06em; text-transform:uppercase; color:var(--dim); }
.demo .sizes { display:flex; gap:4px; align-items:center; font:11px/1.4 var(--mono); color:var(--dim); }
.demo .sizes span { margin-left:8px; }
.demo .pk-dt { flex:1 1 auto; min-height:0; }
.item-id { font-weight:700; }
.item-priority { margin-left:8px; font:11px/1.4 var(--mono); letter-spacing:.06em; color:var(--dim); }
.badges { display:inline-flex; gap:0; }
.stress:empty { display:none; }
.stress { margin:0; max-height:12rem; overflow:auto; font:11px/1.4 var(--mono); color:var(--dim);
  border:1px solid var(--line); padding:4px 8px; }
.stress[data-pass=\"false\"] { border-color:var(--red); }
";

struct Work {
    id: u32,
    priority: u8,
    title: &'static str,
    agent: Option<&'static str>,
    state: &'static str,
    stale: bool,
    age_secs: u64,
    tags: &'static [&'static str],
}

const DAY: u64 = 86_400;
const MBP: Option<&str> = Some("FHKDV644W6-MBP");
const SELFDEV: Option<&str> = Some("selfdev@FHKDV644W6-MBP");

const HAND: [Work; 8] = [
    Work {
        id: 42,
        priority: 2,
        title: "Assistant role: the human \u{2194} agents bridge",
        agent: MBP,
        state: "claimed",
        stale: true,
        age_secs: 11 * DAY,
        tags: &["agent", "ux", "web"],
    },
    Work {
        id: 41,
        priority: 2,
        title: "Jujutsu (jj) for imp work tracking",
        agent: MBP,
        state: "claimed",
        stale: true,
        age_secs: 11 * DAY,
        tags: &["agent", "git", "web"],
    },
    Work {
        id: 36,
        priority: 2,
        title: "Agent roles imps can assume (github, ops, review)",
        agent: MBP,
        state: "claimed",
        stale: true,
        age_secs: 11 * DAY,
        tags: &["agent", "web"],
    },
    Work {
        id: 32,
        priority: 2,
        title: "Program-of-Thought and Graph-of-Thought planners",
        agent: MBP,
        state: "claimed",
        stale: true,
        age_secs: 11 * DAY,
        tags: &["agent", "research"],
    },
    Work {
        id: 2100,
        priority: 1,
        title: "Judge: confirm a goal once, not every turn",
        agent: SELFDEV,
        state: "done",
        stale: false,
        age_secs: 5 * DAY,
        tags: &["kitchen"],
    },
    Work {
        id: 2088,
        priority: 1,
        title: "Appendix A.1 planner reserve prose",
        agent: SELFDEV,
        state: "done",
        stale: false,
        age_secs: 5 * DAY,
        tags: &["kitchen"],
    },
    Work {
        id: 45,
        priority: 0,
        title: "err clara: ask failed 17:25:58 provider timeout",
        agent: None,
        state: "open",
        stale: false,
        age_secs: 3_600,
        tags: &[],
    },
    Work {
        id: 31,
        priority: 1,
        title: "Seats executor: launch llama-server per seat",
        agent: None,
        state: "open",
        stale: false,
        age_secs: 3_600,
        tags: &["inference", "roles"],
    },
];

const TOPICS: [&str; 12] = [
    "Trace the ingest path end to end",
    "Dock restore forgets the section it held",
    "Model health lag hides a demotion",
    "Raft mux retries a refused key",
    "The ledger prunes a done item twice",
    "Seat restart keeps the warm weights",
    "Quorum check steps down on one missed beat",
    "Board column widths survive a reload",
    "Book-keeper drops an orphaned claim",
    "Cluster view groups hosts by rack",
    "Asks toast stacks under the dock",
    "Gateway retries a 429 with backoff",
];

/// Tag sets the generated rows cycle through.
const TAG_SETS: [&[&str]; 8] = [
    &[],
    &["infra"],
    &["ui", "design"],
    &["raft", "ops"],
    &["docs"],
    &["gpu", "infra", "ops"],
    &["api"],
    &["book-keeper", "raft"],
];

/// Generated rows' states, weighted toward open as on the board.
const STATES: [&str; 10] = [
    "open", "open", "open", "open", "open", "claimed", "review", "blocked", "done", "done",
];
const AGENTS: [Option<&str>; 4] = [None, MBP, SELFDEV, Some("L96N-mini")];

fn work(count: usize) -> Vec<Work> {
    let mut all: Vec<Work> = HAND.into_iter().take(count).collect();
    for n in 0..count.saturating_sub(HAND.len()) {
        let state = STATES[n % STATES.len()];
        all.push(Work {
            id: 100 + n as u32,
            priority: (n % 4) as u8,
            title: TOPICS[n % TOPICS.len()],
            agent: if state == "open" {
                None
            } else {
                AGENTS[n % AGENTS.len()]
            },
            state,
            stale: state == "claimed" && n % 3 == 0,
            age_secs: 3_600 * (1 + (n as u64 * 7_919) % 2_000),
            tags: TAG_SETS[n % TAG_SETS.len()],
        });
    }
    all
}

fn columns() -> Vec<DataColumnSpec> {
    vec![
        DataColumnSpec::new("item", "item").pinned().width("15ch"),
        DataColumnSpec::new("title", "title").class("pk-dt-col-title"),
        DataColumnSpec::new("agent", "agent")
            .enumerated()
            .width("26ch")
            .narrow(),
        DataColumnSpec::new("state", "state")
            .enumerated()
            .width("20ch"),
        DataColumnSpec::new("since", "since").number().width("7ch"),
        DataColumnSpec::new("type", "type")
            .enumerated()
            .width("6ch")
            .narrow(),
    ]
}

fn table(work: &[Work]) -> TableData {
    let mut builder = TableDataBuilder::with_capacity(&columns(), work.len());
    let mut id = String::new();
    for w in work {
        id.clear();
        let _ = write!(id, "imp-{}", w.id);
        let row = builder
            .row(
                id.as_str(),
                [
                    Value::Text(&id),
                    Value::Text(w.title),
                    Value::from(w.agent),
                    Value::Text(w.state),
                    Value::Number(w.age_secs as f64),
                    Value::Text("T"),
                ],
            )
            .tags(w.tags);
        if w.state == "done" {
            row.class("pk-dt-muted");
        }
    }
    builder.build()
}

fn ago(secs: u64) -> String {
    match secs {
        s if s >= DAY => format!("{}d", s / DAY),
        s if s >= 3_600 => format!("{}h", s / 3_600),
        s => format!("{}m", s / 60),
    }
}

fn badge(value: &str, tone: BadgeTone, variant: BadgeVariant) -> Element {
    let spec = BadgeSpec {
        small: true,
        ..BadgeSpec::new("state", value, BadgeKind::Status)
    }
    .with_tone(tone, variant);
    rsx! { Badge { spec, on_action: move |_| {} } }
}

fn state_tone(state: &str) -> BadgeTone {
    match state {
        "claimed" | "in_progress" => BadgeTone::Info,
        "review" => BadgeTone::Warn,
        "blocked" => BadgeTone::Err,
        "done" => BadgeTone::Ok,
        _ => BadgeTone::Neutral,
    }
}

/// One cell of `work[row]`, by declared column.
fn cell(w: &Work, column: usize) -> Element {
    match column {
        0 => rsx! {
            span { class: "item-id", "imp-{w.id}" }
            span { class: "item-priority", "P{w.priority}" }
        },
        1 => rsx! { "{w.title}" },
        2 => rsx! { {w.agent.unwrap_or("\u{2014}")} },
        3 => rsx! {
            span { class: "badges",
                {badge(w.state, state_tone(w.state), BadgeVariant::Solid)}
                if w.stale {
                    {badge("stale\u{2026}", BadgeTone::Warn, BadgeVariant::Outline)}
                }
            }
        },
        4 => rsx! { "{ago(w.age_secs)}" },
        _ => rsx! { "T" },
    }
}

const SIZES: [usize; 4] = [267, 10_000, 100_000, 1_000_000];

/// One loaded size: the rows the cells render from, the table's data, and
/// how long building both took.
#[derive(Clone)]
struct Loaded {
    work: Rc<Vec<Work>>,
    data: TableData,
    built: f64,
}

impl PartialEq for Loaded {
    fn eq(&self, other: &Self) -> bool {
        self.data == other.data
    }
}

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    let mut size = use_signal(|| SIZES[0]);
    let loaded = use_memo(move || {
        let started = js_sys::Date::now();
        let work = Rc::new(work(size()));
        let data = table(&work);
        Loaded {
            work,
            data,
            built: js_sys::Date::now() - started,
        }
    });
    let Loaded { work, data, built } = loaded();
    use_effect(|| {
        document::eval(include_str!("support/table_stress.js"));
    });
    rsx! {
        style { {CSS} }
        style { {DEMO_CSS} }
        main { class: "demo",
            h1 { "work" }
            div { class: "sizes",
                for n in SIZES {
                    button {
                        class: if size() == n { "pk-dt-btn on" } else { "pk-dt-btn" },
                        "data-rows": "{n}",
                        onclick: move |_| size.set(n),
                        "{n} rows"
                    }
                }
                span { id: "built", "data-ms": "{built}", "data built in {built:.0} ms" }
            }
            pre { id: "stress", class: "stress" }
            DataTable {
                id: "work",
                label: "work",
                columns: columns(),
                data,
                cell: move |(row, column): (usize, usize)| cell(&work[row], column),
                defaults: TableDefaults {
                    group_by: Some("state".into()),
                    selectable: Selectable::Single,
                    ..TableDefaults::default()
                },
                windowed: true,
                storage_key: Some("panel_kit_example_work".to_string()),
            }
        }
    }
}
