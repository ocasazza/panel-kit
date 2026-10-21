//! Table demo: the board's work ledger in the one table component.
//!
//! Run with: `dx serve --example table --platform web`
//!
//! A tag ribbon in the gutter, a bold id with its priority, layered status
//! badges, finished rows dimmed, rows grouped by state, typed column filters,
//! multi-column sort, drag-to-reorder and drag-to-resize headers, density,
//! cards, a virtual window over ~270 rows, copy as TSV and CSV download, and
//! the layout persisted in localStorage.

use dioxus::prelude::*;
use panel_kit::badge::Badge;
use panel_kit::widgets::{DataColumnSpec, DataRow, DataTable};
use panel_kit::CSS;
use panel_kit_core::badge::{BadgeKind, BadgeSpec, BadgeTone, BadgeVariant};
use panel_kit_core::widgets::data_table::{Selectable, SortKey, TableDefaults};

const DEMO_CSS: &str = "
body { overflow:hidden !important; }
.demo { height:100vh; box-sizing:border-box; padding:1.5rem 2rem; display:flex; flex-direction:column; }
.demo h1 { margin:0 0 .5rem; font:700 11px/1.4 var(--mono); letter-spacing:.06em; text-transform:uppercase; color:var(--dim); }
.demo .pk-dt { flex:1 1 auto; min-height:0; }
.item-id { font-weight:700; }
.item-priority { margin-left:8px; font:11px/1.4 var(--mono); letter-spacing:.06em; color:var(--dim); }
.badges { display:inline-flex; gap:0; }
";

struct Work {
    id: u32,
    priority: u8,
    title: &'static str,
    agent: Option<&'static str>,
    state: &'static str,
    stale: bool,
    age_secs: u64,
    tags: Vec<&'static str>,
}

#[allow(clippy::too_many_arguments)]
fn item(
    id: u32,
    priority: u8,
    title: &'static str,
    agent: Option<&'static str>,
    state: &'static str,
    stale: bool,
    age_secs: u64,
    tags: &[&'static str],
) -> Work {
    Work {
        id,
        priority,
        title,
        agent,
        state,
        stale,
        age_secs,
        tags: tags.to_vec(),
    }
}

const DAY: u64 = 86_400;
const MBP: Option<&str> = Some("FHKDV644W6-MBP");
const SELFDEV: Option<&str> = Some("selfdev@FHKDV644W6-MBP");

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
const TAGS: [&str; 9] = [
    "infra",
    "ui",
    "raft",
    "ops",
    "docs",
    "gpu",
    "api",
    "design",
    "book-keeper",
];

fn work() -> Vec<Work> {
    let mut all = vec![
        item(
            42,
            2,
            "Assistant role: the human \u{2194} agents bridge",
            MBP,
            "claimed",
            true,
            11 * DAY,
            &["agent", "ux", "web"],
        ),
        item(
            41,
            2,
            "Jujutsu (jj) for imp work tracking",
            MBP,
            "claimed",
            true,
            11 * DAY,
            &["agent", "git", "web"],
        ),
        item(
            36,
            2,
            "Agent roles imps can assume (github, ops, review)",
            MBP,
            "claimed",
            true,
            11 * DAY,
            &["agent", "web"],
        ),
        item(
            32,
            2,
            "Program-of-Thought and Graph-of-Thought planners",
            MBP,
            "claimed",
            true,
            11 * DAY,
            &["agent", "research"],
        ),
        item(
            2100,
            1,
            "Judge: confirm a goal once, not every turn",
            SELFDEV,
            "done",
            false,
            5 * DAY,
            &["kitchen"],
        ),
        item(
            2088,
            1,
            "Appendix A.1 planner reserve prose",
            SELFDEV,
            "done",
            false,
            5 * DAY,
            &["kitchen"],
        ),
        item(
            45,
            0,
            "err clara: ask failed 17:25:58 provider timeout",
            None,
            "open",
            false,
            3_600,
            &[],
        ),
        item(
            31,
            1,
            "Seats executor: launch llama-server per seat",
            None,
            "open",
            false,
            3_600,
            &["inference", "roles"],
        ),
    ];
    for n in 0..259usize {
        let tags: Vec<&str> = match n % 4 {
            0 => vec![],
            1 => vec![TAGS[(n * 7) % TAGS.len()]],
            _ => vec![TAGS[(n * 7) % TAGS.len()], TAGS[(n * 3 + 1) % TAGS.len()]],
        };
        all.push(item(
            100 + n as u32,
            (n % 4) as u8,
            TOPICS[n % TOPICS.len()],
            None,
            "open",
            false,
            3_600 * (1 + n as u64 % 90),
            &tags,
        ));
    }
    all
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

fn rows() -> Vec<DataRow> {
    work()
        .into_iter()
        .map(|w| {
            let id = format!("imp-{}", w.id);
            let agent = w.agent.unwrap_or("\u{2014}");
            let row = DataRow::new(id.clone())
                .tags(&w.tags)
                .cell(
                    rsx! {
                        span { class: "item-id", "{id}" }
                        span { class: "item-priority", "P{w.priority}" }
                    },
                    SortKey::text(&id),
                    id.clone(),
                )
                .text(w.title)
                .cell(
                    rsx! { "{agent}" },
                    w.agent.map(SortKey::text).unwrap_or(SortKey::Missing),
                    w.agent.unwrap_or(""),
                )
                .cell(
                    rsx! {
                        span { class: "badges",
                            {badge(w.state, state_tone(w.state), BadgeVariant::Solid)}
                            if w.stale {
                                {badge("stale\u{2026}", BadgeTone::Warn, BadgeVariant::Outline)}
                            }
                        }
                    },
                    SortKey::text(w.state),
                    w.state,
                )
                .cell(
                    rsx! { "{ago(w.age_secs)}" },
                    SortKey::num(w.age_secs as f64),
                    w.age_secs.to_string(),
                )
                .text("T");
            if w.state == "done" {
                row.muted()
            } else {
                row
            }
        })
        .collect()
}

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    rsx! {
        style { {CSS} }
        style { {DEMO_CSS} }
        main { class: "demo",
            h1 { "work" }
            DataTable {
                id: "work",
                label: "work",
                columns: columns(),
                rows: rows(),
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
