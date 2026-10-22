//! The table demo's ledger (`examples/table.rs`): its columns and its rows, so
//! the benches and the scale gate time the shape the browser stress run loads.

use std::fmt::Write;
use std::ops::Range;

use panel_kit_core::widgets::data_table::{DataColumnSpec, TableData, TableDataBuilder, Value};

#[derive(Clone, Copy)]
struct Work {
    id: u32,
    title: &'static str,
    agent: Option<&'static str>,
    state: &'static str,
    age_secs: u64,
    tags: &'static [&'static str],
}

const DAY: u64 = 86_400;
const MBP: Option<&str> = Some("FHKDV644W6-MBP");
const SELFDEV: Option<&str> = Some("selfdev@FHKDV644W6-MBP");

const fn work(
    id: u32,
    title: &'static str,
    agent: Option<&'static str>,
    state: &'static str,
    age_secs: u64,
    tags: &'static [&'static str],
) -> Work {
    Work {
        id,
        title,
        agent,
        state,
        age_secs,
        tags,
    }
}

const HAND: [Work; 8] = [
    work(
        42,
        "Assistant role: the human \u{2194} agents bridge",
        MBP,
        "claimed",
        11 * DAY,
        &["agent", "ux", "web"],
    ),
    work(
        41,
        "Jujutsu (jj) for imp work tracking",
        MBP,
        "claimed",
        11 * DAY,
        &["agent", "git", "web"],
    ),
    work(
        36,
        "Agent roles imps can assume (github, ops, review)",
        MBP,
        "claimed",
        11 * DAY,
        &["agent", "web"],
    ),
    work(
        32,
        "Program-of-Thought and Graph-of-Thought planners",
        MBP,
        "claimed",
        11 * DAY,
        &["agent", "research"],
    ),
    work(
        2100,
        "Judge: confirm a goal once, not every turn",
        SELFDEV,
        "done",
        5 * DAY,
        &["kitchen"],
    ),
    work(
        2088,
        "Appendix A.1 planner reserve prose",
        SELFDEV,
        "done",
        5 * DAY,
        &["kitchen"],
    ),
    work(
        45,
        "err clara: ask failed 17:25:58 provider timeout",
        None,
        "open",
        3_600,
        &[],
    ),
    work(
        31,
        "Seats executor: launch llama-server per seat",
        None,
        "open",
        3_600,
        &["inference", "roles"],
    ),
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

/// Accented, CJK and special-casing titles for the non-ASCII share.
const TOPICS_I18N: [&str; 6] = [
    "Réunion d'équipe du matin",
    "Straße sperren und prüfen",
    "日本語のログを検索する",
    "İstanbul gateway timeout",
    "Crème brûlée recipe import",
    "Ünïcödé header parsing",
];

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

const STATES: [&str; 10] = [
    "open", "open", "open", "open", "open", "claimed", "review", "blocked", "done", "done",
];
const AGENTS: [Option<&str>; 4] = [None, MBP, SELFDEV, Some("L96N-mini")];

/// Row `row` of the ledger: the hand-written rows, then generated ones.
/// Every seventh generated title is non-ASCII.
fn work_at(row: usize) -> Work {
    if let Some(hand) = HAND.get(row) {
        return *hand;
    }
    let n = row - HAND.len();
    let state = STATES[n % STATES.len()];
    work(
        100 + n as u32,
        if row % 7 == 0 {
            TOPICS_I18N[(row / 7) % TOPICS_I18N.len()]
        } else {
            TOPICS[n % TOPICS.len()]
        },
        if state == "open" {
            None
        } else {
            AGENTS[n % AGENTS.len()]
        },
        state,
        3_600 * (1 + (n as u64 * 7_919) % 2_000),
        TAG_SETS[n % TAG_SETS.len()],
    )
}

pub fn columns() -> Vec<DataColumnSpec> {
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
        DataColumnSpec::new("note", "note").width("48ch"),
    ]
}

/// A note unique to its row that shares a long prefix with every other.
fn write_note(out: &mut String, row: usize) {
    let _ = write!(
        out,
        "Trace the ingest path through shard {row:07} replica {}",
        row % 3
    );
}

/// Pushes rows `rows` of the ledger onto `builder`.
pub fn push(builder: &mut TableDataBuilder, rows: Range<usize>) {
    let mut id = String::new();
    let mut note = String::new();
    for row in rows {
        let w = work_at(row);
        id.clear();
        let _ = write!(id, "imp-{}", w.id);
        note.clear();
        write_note(&mut note, row);
        let writer = builder
            .row(
                id.as_str(),
                [
                    Value::Text(&id),
                    Value::Text(w.title),
                    Value::from(w.agent),
                    Value::Text(w.state),
                    Value::Number(w.age_secs as f64),
                    Value::Text("T"),
                    Value::Text(&note),
                ],
            )
            .tags(w.tags);
        if w.state == "done" {
            writer.class("pk-dt-muted");
        }
    }
}

/// The demo's load: slices of 50 000 rows appended one after another.
pub fn data(count: usize) -> TableData {
    const SLICE: usize = 50_000;
    let first = SLICE.min(count);
    let mut builder = TableDataBuilder::with_capacity(&columns(), first);
    push(&mut builder, 0..first);
    let mut data = builder.build();
    let mut done = first;
    while done < count {
        let end = (done + SLICE).min(count);
        data = data.append(|builder| push(builder, done..end));
        done = end;
    }
    data
}
