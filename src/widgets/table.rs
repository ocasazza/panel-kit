//! The semantic table painted by the one table component.
//!
//! A [`TableModel`](panel_kit_core::widgets::table::TableModel) is static data:
//! cells, columns, rows. The interactive table is
//! [`data_table`](super::data_table); this module turns the model into its
//! rows and paints it with the same component, so a spec-authored table and a
//! host-built one cannot drift.

use dioxus::prelude::*;
use panel_kit_core::widgets::table::{TableRow, TableView};

use crate::widgets::data_table::{DataColumnSpec, DataRow, DataTable, SortKey, TableDefaults};

type Rgb = (u8, u8, u8);

fn spec(columns: &[panel_kit_core::widgets::table::TableColumn]) -> Vec<DataColumnSpec> {
    columns
        .iter()
        .map(|column| DataColumnSpec::new(column.key.clone(), column.title.clone()))
        .collect()
}

fn rows(rows: &[&TableRow]) -> Vec<DataRow> {
    rows
        .iter()
        .map(|row| {
            let mut data = DataRow::new(row.id.clone()).tags(
                &row.tags.iter().map(String::as_str).collect::<Vec<_>>(),
            );
            for cell in &row.cells {
                data = match cell {
                    panel_kit_core::widgets::table::TableCell::Text(text) => data.text(text.clone()),
                    panel_kit_core::widgets::table::TableCell::Status { label, color } => {
                        let (r, g, b) = *color;
                        let style = format!("--status-c:rgb({r},{g},{b});");
                        data.cell(
                            rsx! {
                                td { title: "{label}",
                                    span { class: "pk-table-status", role: "status", aria_label: "{label}", style: "{style}",
                                        span { aria_hidden: "true", "\u{25cf}" }
                                        " {label}"
                                    }
                                }
                            },
                            SortKey::text(label),
                            label,
                        )
                    }
                    panel_kit_core::widgets::table::TableCell::Meter { ratio, text, color } => {
                        let percent = (ratio.clamp(0.0, 1.0) * 100.0).round() as u32;
                        let style = color
                            .map(|(r, g, b)| format!("--meter-c:rgb({r},{g},{b});"))
                            .unwrap_or_default();
                        data.cell(
                            rsx! {
                                td { title: "{text}", style: "{style}",
                                    span {
                                        class: "pk-table-meter",
                                        role: "meter",
                                        aria_valuemin: "0",
                                        aria_valuemax: "100",
                                        aria_valuenow: "{percent}",
                                        aria_label: "{text}",
                                        span { class: "pk-table-meter-fill", style: "width:{percent}%;" }
                                        span { class: "pk-table-meter-text", "{text}" }
                                    }
                                }
                            },
                            SortKey::opt_num(Some(*ratio)),
                            text,
                        )
                    }
                };
            }
            data
        })
        .collect()
}

/// Paint a static table model with the table component.
pub fn table(view: TableView<'_>) -> Element {
    rsx! {
        DataTable {
            columns: spec(view.columns),
            rows: rows(view.rows),
            defaults: TableDefaults::default(),
        }
    }
}

/// One model row's cells as plain text: what an export or a test reads.
pub fn row_text(row: &TableRow) -> Vec<String> {
    row.cells
        .iter()
        .map(|cell| match cell {
            panel_kit_core::widgets::table::TableCell::Text(text) => text.clone(),
            panel_kit_core::widgets::table::TableCell::Status { label, .. } => label.clone(),
            panel_kit_core::widgets::table::TableCell::Meter { text, .. } => text.clone(),
        })
        .collect()
}

#[allow(dead_code)]
fn _rgb(_: Rgb) {}
