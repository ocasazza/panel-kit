//! The semantic table painted by the one table component.
//!
//! A [`TableModel`](panel_kit_core::widgets::table::TableModel) is static data:
//! cells, columns, rows. The interactive table is
//! [`data_table`](super::data_table); this module turns the model into its
//! rows and paints it with the same component, so a spec-authored table and a
//! host-built one cannot drift.

use dioxus::prelude::*;
use panel_kit_core::widgets::table::{
    ColumnWidth, TableCell, TableColumn, TableRow, TableView, TextAlign,
};

use crate::widgets::data_table::{
    Align, DataColumnSpec, DataRow, DataTable, SortKey, TableDefaults,
};

fn spec(columns: &[TableColumn]) -> Vec<DataColumnSpec> {
    columns
        .iter()
        .map(|column| {
            let align = match column.align {
                TextAlign::Left => Align::Left,
                TextAlign::Center => Align::Center,
                TextAlign::Right => Align::Right,
            };
            let spec = DataColumnSpec::new(column.key.clone(), column.title.clone()).align(align);
            match column.width {
                ColumnWidth::Fixed { value } => spec.width(format!("{value}ch")),
                ColumnWidth::Flex { .. } => spec,
            }
        })
        .collect()
}

fn rows(rows: &[&TableRow]) -> Vec<DataRow> {
    rows.iter()
        .map(|row| {
            let tags: Vec<&str> = row.tags.iter().map(String::as_str).collect();
            row.cells.iter().fold(DataRow::new(row.id.clone()).tags(&tags), |data, cell| match cell {
                TableCell::Text(text) => data.text(text.clone()),
                TableCell::Status { label, color } => {
                    let (r, g, b) = *color;
                    let style = format!("--status-c:rgb({r},{g},{b});");
                    data.cell(
                        rsx! {
                            span { class: "pk-table-status", role: "status", aria_label: "{label}", style: "{style}",
                                span { aria_hidden: "true", "\u{25cf}" }
                                " {label}"
                            }
                        },
                        SortKey::text(label),
                        label.clone(),
                    )
                }
                TableCell::Meter { ratio, text, color } => {
                    let percent = (ratio.clamp(0.0, 1.0) * 100.0).round() as u32;
                    let style = color
                        .map(|(r, g, b)| format!("--meter-c:rgb({r},{g},{b});"))
                        .unwrap_or_default();
                    data.cell(
                        rsx! {
                            span {
                                class: "pk-table-meter",
                                role: "meter",
                                style: "{style}",
                                aria_valuemin: "0",
                                aria_valuemax: "100",
                                aria_valuenow: "{percent}",
                                aria_label: "{text}",
                                span { class: "pk-table-meter-fill", style: "width:{percent}%;" }
                                span { class: "pk-table-meter-text", "{text}" }
                            }
                        },
                        SortKey::opt_num(Some(*ratio)),
                        text.clone(),
                    )
                }
            })
        })
        .collect()
}

/// Paint a static table model with the table component. The DOM id derives
/// from the column keys, since a model carries none of its own.
pub fn table(view: TableView<'_>) -> Element {
    let keys: Vec<&str> = view.columns.iter().map(|c| c.key.as_str()).collect();
    rsx! {
        DataTable {
            id: format!("pk-table-{}", keys.join("-")),
            columns: spec(view.columns),
            rows: rows(view.rows),
            defaults: TableDefaults::default(),
        }
    }
}
