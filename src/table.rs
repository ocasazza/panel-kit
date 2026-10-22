//! Data table — panel-kit's tabular sibling to the dropdown/cascade family.
//!
//! Semantic `<table>` markup in the library's visual language: thin
//! `--line` borders, mono type, sticky OPAQUE header (the dropdown's sticky
//! group-header garbling taught us: never let scrolled rows bleed through a
//! translucent sticky element), hover + accent-selected rows, and ellipsized
//! cells that carry the full text in a `title` tooltip.
//!
//! Purely visual — no state machine worth extracting to `panel-kit-core`:
//! selection lives in the host, exactly like every other panel-kit widget.
//!
//! # Examples
//!
//! ```no_run
//! use dioxus::prelude::*;
//! use panel_kit::table::Table;
//!
//! # fn ui() -> Element {
//! let mut selected = use_signal(|| None::<usize>);
//! rsx! {
//!     Table {
//!         columns: vec!["policy".into(), "reward".into()],
//!         rows: vec![
//!             vec!["spot-walk-pbt-v74-000".into(), "0.72".into()],
//!             vec!["spot-walk-tyan-001".into(), "0.51".into()],
//!         ],
//!         selected: selected(),
//!         on_row_click: move |i: usize| selected.set(Some(i)),
//!     }
//! }
//! # }
//! ```

use dioxus::prelude::*;

/// Low-chrome data table with sticky header and host-owned row selection.
#[component]
pub fn Table(
    /// Header labels, in column order.
    columns: Vec<String>,
    /// Row-major cell text. Rows shorter than `columns` render empty cells;
    /// extra cells are dropped.
    rows: Vec<Vec<String>>,
    /// Highlighted row index (host-owned, like `Dropdown::selected`).
    #[props(default)]
    selected: Option<usize>,
    /// Tighter padding for dense data (metrics, logs).
    #[props(default = false)]
    dense: bool,
    /// Extra class on the scroll container (sizing hooks for the host).
    #[props(default)]
    class: Option<String>,
    /// Fired with the row index on click; omit for a read-only table.
    #[props(default)]
    on_row_click: Option<EventHandler<usize>>,
) -> Element {
    let root_class = match &class {
        Some(extra) => format!("pk-table-wrap {extra}"),
        None => "pk-table-wrap".to_string(),
    };
    let table_class = if dense { "pk-table pk-table-dense" } else { "pk-table" };
    let clickable = on_row_click.is_some();
    let n_cols = columns.len();

    rsx! {
        div { class: "{root_class}",
            table { class: "{table_class}",
                thead {
                    tr {
                        for (ci, col) in columns.iter().enumerate() {
                            th { key: "h-{ci}", title: "{col}", "{col}" }
                        }
                    }
                }
                tbody {
                    for (ri, row) in rows.iter().enumerate() {
                        {
                            let is_selected = selected == Some(ri);
                            let row_class = match (is_selected, clickable) {
                                (true, _) => "pk-table-row selected",
                                (false, true) => "pk-table-row clickable",
                                (false, false) => "pk-table-row",
                            };
                            let row = row.clone();
                            rsx! {
                                tr {
                                    key: "r-{ri}",
                                    class: "{row_class}",
                                    onclick: move |_| {
                                        if let Some(cb) = &on_row_click {
                                            cb.call(ri);
                                        }
                                    },
                                    for ci in 0..n_cols {
                                        {
                                            let cell = row.get(ci).cloned().unwrap_or_default();
                                            rsx! {
                                                td { key: "c-{ri}-{ci}", title: "{cell}", "{cell}" }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    if rows.is_empty() {
                        tr {
                            td { class: "pk-table-empty", colspan: "{n_cols}", "no rows" }
                        }
                    }
                }
            }
        }
    }
}
