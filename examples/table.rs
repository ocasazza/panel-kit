//! Table demo — one component, every table feature.
//!
//! Run with: `dx serve --example table --platform web`
//!
//! What it demonstrates:
//! - Multi-column sort from the headers, each carrying its rank (1, 2, …).
//! - A quick filter over every cell, plus a per-column filter row.
//! - A tag ribbon with counts, any/all modes, and an expand control.
//! - Column order, visibility, grouping with collapsed groups, and density.
//! - Selection: single click toggles, shift extends a range, `a` selects all,
//!   `Escape` clears; a keyboard cursor moves with the arrows.
//! - A virtual window: only a slice of rows is mounted, with spacers sized by
//!   the row height.
//! - The state persisted in localStorage and exported as TSV or CSV by the
//!   host (export is the host's call: it owns the clipboard and the file).

use dioxus::prelude::*;
use panel_kit::widgets::{csv, tsv, DataColumnSpec, DataRow, DataTable};
use panel_kit::CSS;
use panel_kit_core::widgets::data_table::{
    Density, Selectable, SortDir, SortSpec, TableDefaults, ViewMode,
};

const DEMO_CSS: &str = "
body { overflow:auto !important; }
.demo { max-width:72rem; margin:0 auto; padding:2rem; }
.demo h1 { margin:0 0 .3rem; font-size:.8rem; }
.demo p.hint { margin:0 0 1rem; color:var(--dim); font-size:.72rem; }
.pk-dt-cursor td { outline:1px solid var(--accent); outline-offset:-1px; }
.export { margin-top:.6rem; display:flex; gap:.4rem; align-items:center; }
.export button { background:var(--bg); color:var(--dim); border:1px solid var(--line2);
  border-radius:3px; padding:.15rem .5rem; font-size:.68rem; cursor:pointer; }
.export pre { flex:1; margin:0; color:var(--dim); font-size:.66rem; max-height:7rem; overflow:auto;
  white-space:pre-wrap; }
";

fn columns() -> Vec<DataColumnSpec> {
    vec![
        DataColumnSpec::new("id", "Item").pinned(),
        DataColumnSpec::new("priority", "P"),
        DataColumnSpec::new("title", "Title"),
        DataColumnSpec::new("tokens", "Tokens"),
        DataColumnSpec::new("state", "State"),
        DataColumnSpec::new("agent", "Agent"),
    ]
}

fn rows() -> Vec<DataRow> {
    let data: [(&str, &str, &str, u32, &str, &str, &[&str]); 8] = [
        ("imp-9", "P2", "Trace the ingest path end to end", 940, "open", "—", &["ingest"]),
        ("imp-10", "P1", "Seat restart keeps the warm weights", 12_400, "claimed", "selfdev", &["p1", "seat"]),
        ("imp-3", "P2", "The ledger prunes a done item twice", 310, "review", "—", &["ingest"]),
        ("imp-1", "P0", "Quorum check steps down on one missed beat", 88_150, "done", "selfdev", &["p1"]),
        ("imp-42", "P3", "Dock restore forgets the section it held", 75, "open", "—", &["seat"]),
        ("imp-7", "P1", "Model health lag hides a demotion", 2_050, "blocked", "selfdev", &["p1", "ingest"]),
        ("imp-51", "P1", "Seats executor launches llama-server", 6_400, "open", "—", &["seat"]),
        ("imp-64", "P2", "Raft mux retries a refused key", 480, "review", "selfdev", &["p1"]),
    ];
    data.into_iter()
        .map(|(id, priority, title, tokens, state, agent, tags)| {
            DataRow::new(id)
                .text(id)
                .text(priority)
                .text(title)
                .num(Some(f64::from(tokens)), tokens.to_string())
                .text(state)
                .text(agent)
                .tags(tags)
        })
        .collect()
}

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    let columns = use_hook(columns);
    let rows = use_hook(rows);
    let mut export = use_signal(|| None::<String>);

    let visible: Vec<bool> = columns.iter().map(|_| true).collect();
    let shown: Vec<usize> = (0..rows.len()).collect();

    rsx! {
        style { {CSS} }
        style { {DEMO_CSS} }
        main { class: "demo",
            h1 { "table · one component" }
            p { class: "hint",
                "headers sort (ranked) · type to filter · #tags narrow · columns / group / dense in the bar · click selects, shift extends, arrows move, a selects all, esc clears"
            }
            DataTable {
                columns: columns.clone(),
                rows: rows.clone(),
                defaults: TableDefaults {
                    sort: vec![SortSpec { column: "id".into(), dir: SortDir::Asc }],
                    density: Density::Normal,
                    group_by: None,
                    selectable: Selectable::Multi,
                    view: ViewMode::Rows,
                    hidden: Default::default(),
                },
                storage_key: Some("panel_kit_example_table".to_string()),
                placeholder: "filter items…",
            }
            div { class: "export",
                {
                    let tsv_columns = columns.clone();
                    let tsv_rows = rows.clone();
                    let tsv_visible = visible.clone();
                    let tsv_shown = shown.clone();
                    rsx! {
                        button {
                            onclick: move |_| {
                                export.set(Some(tsv(&tsv_columns, &tsv_rows, &tsv_visible, &tsv_shown)));
                            },
                            "copy as TSV"
                        }
                    }
                }
                {
                    let csv_columns = columns.clone();
                    let csv_rows = rows.clone();
                    let csv_shown = shown.clone();
                    rsx! {
                        button {
                            onclick: move |_| {
                                export.set(Some(csv(&csv_columns, &csv_rows, &visible, &csv_shown)));
                            },
                            "download as CSV"
                        }
                    }
                }
                pre { {export.read().clone().unwrap_or_default()} }
            }
        }
    }
}
