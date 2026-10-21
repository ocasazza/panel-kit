//! Table demo: the board's work ledger in the one table component, from 267
//! rows to a million.
//!
//! Run with: `dx serve --example table --platform web`
//!
//! A tag ribbon in the gutter, a bold id with its priority, layered status
//! badges, finished rows dimmed, rows grouped by state, typed column filters,
//! multi-column sort, drag-to-reorder and drag-to-resize headers, density,
//! cards, a virtual window, multi-row selection, copy as TSV and CSV
//! download, and the layout persisted in localStorage. The size buttons
//! rebuild the data in slices behind a progress bar; `live` appends rows
//! every 100 ms and reports the append-to-paint latency. Every seventh title
//! is non-ASCII and the `note` column is unique long text. `?stress` runs
//! `support/table_stress.js`: a million rows driven through every interaction
//! against per-step budgets, with the result in `#stress` (`data-pass`).

use std::fmt::Write as _;
use std::ops::Range;

use dioxus::prelude::*;
use js_sys::{Array, ArrayBuffer, Function, Promise, Reflect};
use panel_kit::badge::Badge;
use panel_kit::loading::ProgressBar;
use panel_kit::widgets::{DataColumnSpec, DataTable, TableData, TableDataBuilder, Value};
use panel_kit::CSS;
use panel_kit_core::badge::{BadgeKind, BadgeSpec, BadgeTone, BadgeVariant};
use panel_kit_core::widgets::data_table::{Selectable, TableDefaults};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

const DEMO_CSS: &str = "
body { overflow:hidden !important; }
.demo { height:100vh; box-sizing:border-box; padding:1.5rem 2rem; display:flex; flex-direction:column; gap:.5rem; }
.demo h1 { margin:0; font:700 11px/1.4 var(--mono); letter-spacing:.06em; text-transform:uppercase; color:var(--dim); }
.demo .sizes { display:flex; flex-wrap:wrap; gap:4px; align-items:center; font:11px/1.4 var(--mono); color:var(--dim); }
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

#[derive(Clone, Copy)]
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

/// Accented, CJK and special-casing titles for the non-ASCII share.
const TOPICS_I18N: [&str; 6] = [
    "Réunion d'équipe du matin",
    "Straße sperren und prüfen",
    "日本語のログを検索する",
    "İstanbul gateway timeout",
    "Crème brûlée recipe import",
    "Ünïcödé header parsing",
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

/// Row `row` of the ledger: the hand-written rows, then generated ones.
/// Every seventh generated title is non-ASCII.
fn work_at(row: usize) -> Work {
    if let Some(hand) = HAND.get(row) {
        return *hand;
    }
    let n = row - HAND.len();
    let state = STATES[n % STATES.len()];
    Work {
        id: 100 + n as u32,
        priority: (n % 4) as u8,
        title: if row.is_multiple_of(7) {
            TOPICS_I18N[(row / 7) % TOPICS_I18N.len()]
        } else {
            TOPICS[n % TOPICS.len()]
        },
        agent: if state == "open" {
            None
        } else {
            AGENTS[n % AGENTS.len()]
        },
        state,
        stale: state == "claimed" && n.is_multiple_of(3),
        age_secs: 3_600 * (1 + (n as u64 * 7_919) % 2_000),
        tags: TAG_SETS[n % TAG_SETS.len()],
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
fn push(builder: &mut TableDataBuilder, rows: Range<usize>) {
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

/// One cell of row `row`, by declared column.
fn cell(row: usize, column: usize) -> Element {
    let w = work_at(row);
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
        5 => rsx! { "T" },
        _ => {
            let mut note = String::new();
            write_note(&mut note, row);
            rsx! { "{note}" }
        }
    }
}

const SIZES: [usize; 4] = [267, 10_000, 100_000, 1_000_000];

/// Rows built per slice before yielding to the browser.
const SLICE: usize = 50_000;

/// Columns whose sort ranks are computed at load, one per frame. `note`
/// stays cold: sorting it is the long-shared-prefix path.
const WARM: [usize; 2] = [0, 1];

/// Rows the live toggle appends per tick, and the tick's period.
const LIVE_ROWS: usize = 200;
const LIVE_MS: i32 = 100;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = performance, js_name = now)]
    fn clock() -> f64;
    #[wasm_bindgen(js_name = requestAnimationFrame)]
    fn request_animation_frame(callback: &Function) -> i32;
    #[wasm_bindgen(js_name = setTimeout)]
    fn set_timeout(callback: &Function, ms: i32) -> i32;
}

/// Resolves `ms` milliseconds from now; zero yields one task.
async fn after(ms: i32) {
    let promise = Promise::new(&mut |resolve, _| {
        set_timeout(&resolve, ms);
    });
    let _ = JsFuture::from(promise).await;
}

/// Resolves at the next animation frame.
async fn next_frame() {
    let promise = Promise::new(&mut |resolve, _| {
        request_animation_frame(&resolve);
    });
    let _ = JsFuture::from(promise).await;
}

/// Bytes of the wasm linear memory.
fn wasm_bytes() -> f64 {
    wasm_bindgen::memory()
        .dyn_into::<js_sys::WebAssembly::Memory>()
        .map(|memory| ArrayBuffer::from(memory.buffer()).byte_length() as f64)
        .unwrap_or(0.0)
}

/// Milliseconds since the last click anywhere on the page.
fn since_click() -> f64 {
    Reflect::get(&js_sys::global(), &JsValue::from_str("__click"))
        .ok()
        .and_then(|at| at.as_f64())
        .map_or(0.0, |at| clock() - at)
}

/// Appends one append-to-paint latency to `window.__live`.
fn record_live(ms: f64) {
    let global = js_sys::global();
    let key = JsValue::from_str("__live");
    let list = match Reflect::get(&global, &key) {
        Ok(list) if Array::is_array(&list) => list.unchecked_into::<Array>(),
        _ => {
            let list = Array::new();
            let _ = Reflect::set(&global, &key, &list);
            list
        }
    };
    list.push(&JsValue::from_f64(ms));
}

/// The table's data and how long building it took.
#[derive(Clone)]
struct Loaded {
    data: TableData,
    built: f64,
}

/// Loading progress: fraction of the work done and what is under way.
type Progress = Option<(f64, String)>;

/// Builds `count` rows in slices, yielding between them, then ranks the
/// `WARM` columns one per frame. Progress counts rows built plus rows ranked.
async fn build(count: usize, mut progress: Signal<Progress>) -> TableData {
    let units = (count * (1 + WARM.len())) as f64;
    let specs = columns();
    next_frame().await;
    let first = SLICE.min(count);
    let mut builder = TableDataBuilder::with_capacity(&specs, first);
    push(&mut builder, 0..first);
    let mut data = builder.build();
    let mut done = first;
    while done < count {
        progress.set(Some((
            done as f64 / units,
            format!("building rows {done} of {count}"),
        )));
        next_frame().await;
        let end = (done + SLICE).min(count);
        data = data.append(|builder| push(builder, done..end));
        done = end;
    }
    for (at, &column) in WARM.iter().enumerate() {
        progress.set(Some((
            (count * (1 + at)) as f64 / units,
            format!("ranking {}", specs[column].title),
        )));
        next_frame().await;
        data.warm(column);
    }
    data
}

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    let mut size = use_signal(|| SIZES[0]);
    let mut loaded = use_signal(|| None::<Loaded>);
    let mut progress = use_signal(|| None::<(f64, String)>);
    let mut live = use_signal(|| false);
    let mut last_append = use_signal(|| None::<f64>);
    // (exports so far, kind, bytes, milliseconds from the click).
    let mut exported = use_signal(|| (0u32, "", 0usize, 0.0f64));

    // A new size cancels a build still under way.
    let _loading = use_resource(move || async move {
        let count = size();
        let started = clock();
        progress.set(Some((0.0, "building rows".to_string())));
        let data = build(count, progress).await;
        loaded.set(Some(Loaded {
            data,
            built: clock() - started,
        }));
        progress.set(None);
    });

    use_future(move || async move {
        loop {
            after(LIVE_MS).await;
            if !*live.peek() || progress.peek().is_some() {
                continue;
            }
            let Some(Loaded { data, built }) = (*loaded.peek()).clone() else {
                continue;
            };
            let started = clock();
            let from = data.len();
            let data = data.append(|builder| push(builder, from..from + LIVE_ROWS));
            loaded.set(Some(Loaded { data, built }));
            wasm_bindgen_futures::spawn_local(async move {
                next_frame().await;
                next_frame().await;
                let ms = clock() - started;
                record_live(ms);
                last_append.set(Some(ms));
            });
        }
    });

    use_effect(|| {
        document::eval(
            "addEventListener('click', () => { window.__click = performance.now(); }, true);",
        );
        document::eval(include_str!("support/table_stress.js"));
    });

    let current = loaded();
    let heap = current.as_ref().map_or(0, |l| l.data.memory_bytes());
    let wasm = wasm_bytes();
    let (heap_mb, wasm_mb) = (heap as f64 / 1_048_576.0, wasm / 1_048_576.0);
    let (exports, kind, bytes, took) = exported();
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
                button {
                    id: "live",
                    class: if live() { "pk-dt-btn on" } else { "pk-dt-btn" },
                    aria_pressed: "{live()}",
                    title: "append {LIVE_ROWS} rows every {LIVE_MS} ms",
                    onclick: move |_| live.set(!live()),
                    "live"
                }
                if let Some(ms) = last_append() {
                    span { id: "last-append", "data-ms": "{ms}", "append to paint {ms:.0} ms" }
                }
                if let Some(Loaded { built, .. }) = &current {
                    span { id: "built", "data-ms": "{built}", "data built in {built:.0} ms" }
                }
                span {
                    id: "mem",
                    "data-heap": "{heap}",
                    "data-wasm": "{wasm}",
                    "data {heap_mb:.1} MB, wasm {wasm_mb:.0} MB"
                }
                if exports > 0 {
                    span {
                        id: "export",
                        "data-n": "{exports}",
                        "data-kind": "{kind}",
                        "data-bytes": "{bytes}",
                        "data-ms": "{took}",
                        "{kind} {bytes} bytes {took:.0} ms after click"
                    }
                }
            }
            pre { id: "stress", class: "stress" }
            if let Some((fraction, detail)) = progress() {
                ProgressBar {
                    fraction: Some(fraction),
                    label: format!("loading {} rows", size()),
                    detail: Some(detail),
                }
            }
            if let Some(Loaded { data, .. }) = current {
                DataTable {
                    id: "work",
                    label: "work",
                    columns: columns(),
                    data,
                    cell: move |(row, column): (usize, usize)| cell(row, column),
                    defaults: TableDefaults {
                        group_by: Some("state".into()),
                        selectable: Selectable::Multi,
                        ..TableDefaults::default()
                    },
                    windowed: true,
                    storage_key: Some("panel_kit_example_work".to_string()),
                    on_csv: move |text: String| {
                        exported.set((exports + 1, "csv", text.len(), since_click()));
                    },
                    on_copy: move |text: String| {
                        exported.set((exports + 1, "copy", text.len(), since_click()));
                    },
                }
            }
        }
    }
}
