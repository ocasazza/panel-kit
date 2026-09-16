//! Interactive core-model table with host-owned selection and density policy.
//!
//! Run with: `dx serve --example table --platform web`

use dioxus::prelude::*;
use panel_kit::widgets::table;
use panel_kit::CSS;
use panel_kit_core::widgets::table::{
    ColumnWidth, TableCell, TableColumn, TableModel, TableRow, TableView, TextAlign,
};

const DEMO_CSS: &str = "
body { overflow:auto !important; }
.demo { max-width:64rem; margin:0 auto; padding:2rem; }
.demo h1 { margin:0 0 1rem; font-size:.8rem; }
.controls { display:flex; align-items:center; gap:1rem; margin-bottom:.75rem; color:var(--dim); }
.controls label { display:inline-flex; align-items:center; gap:.35rem; }
.selection { color:var(--dim); }
";

fn main() {
    dioxus::launch(App);
}

fn model() -> TableModel {
    let columns = vec![
        TableColumn {
            key: "policy".into(),
            title: "Policy".into(),
            width: ColumnWidth::Flex { weight: 3 },
            align: TextAlign::Left,
        },
        TableColumn {
            key: "reward".into(),
            title: "Reward".into(),
            width: ColumnWidth::Fixed { value: 9 },
            align: TextAlign::Right,
        },
        TableColumn {
            key: "notes".into(),
            title: "Notes".into(),
            width: ColumnWidth::Flex { weight: 4 },
            align: TextAlign::Left,
        },
        TableColumn {
            key: "status".into(),
            title: "Status".into(),
            width: ColumnWidth::Fixed { value: 12 },
            align: TextAlign::Left,
        },
    ];
    let rows = (0..20)
        .map(|index| TableRow {
            cells: vec![
                TableCell::Text(format!("spot-walk-pbt-v74-{index:03}")),
                TableCell::Text(format!("{:.2}", 0.72 - f64::from(index) * 0.03)),
                TableCell::Text(if index % 3 == 0 {
                    "real trot with a deliberately long description preserved by the tooltip".into()
                } else {
                    "velocity tracker".into()
                }),
                TableCell::Status {
                    label: if index % 2 == 0 { "kept" } else { "reverted" }.into(),
                    color: if index % 2 == 0 {
                        (39, 201, 63)
                    } else {
                        (255, 189, 46)
                    },
                },
            ],
        })
        .collect();
    TableModel { columns, rows }
}

#[component]
fn App() -> Element {
    let mut selected = use_signal(|| None::<usize>);
    let mut dense = use_signal(|| false);
    let mut empty = use_signal(|| false);
    let model = model();
    let rows = if empty() {
        &[][..]
    } else {
        model.rows.as_slice()
    };
    let view = TableView {
        columns: &model.columns,
        rows,
    };
    let row_click = EventHandler::new(move |index: usize| selected.set(Some(index)));

    rsx! {
        style { {CSS} }
        style { {DEMO_CSS} }
        main { class: "demo",
            h1 { "host-owned interactive table" }
            div { class: "controls",
                label {
                    input {
                        r#type: "checkbox",
                        checked: dense(),
                        onchange: move |event| dense.set(event.checked()),
                    }
                    "dense"
                }
                label {
                    input {
                        r#type: "checkbox",
                        checked: empty(),
                        onchange: move |event| empty.set(event.checked()),
                    }
                    "empty state"
                }
                span { class: "selection",
                    match selected() {
                        Some(index) => format!("selected row: {index}"),
                        None => "click a row".to_string(),
                    }
                }
            }
            {table::table(view, selected(), Some(row_click), dense())}
        }
    }
}
