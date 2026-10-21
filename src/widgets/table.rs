//! The semantic table painted by the one table component.
//!
//! A [`TableModel`](panel_kit_core::widgets::table::TableModel) is static data:
//! cells, columns, rows. The interactive table is
//! [`data_table`](super::data_table); this module turns the model into its
//! rows and paints it with the same component, so a spec-authored table and a
//! host-built one cannot drift.

use std::rc::Rc;

use dioxus::prelude::*;
use panel_kit_core::widgets::table::{
    ColumnWidth, TableCell, TableColumn, TableRow, TableView, TextAlign,
};

use crate::widgets::data_table::{
    Align, DataColumnSpec, DataTable, TableData, TableDataBuilder, TableDefaults, Value,
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

/// A cell's plain text: what the table sorts, filters and exports.
fn text(cell: &TableCell) -> &str {
    match cell {
        TableCell::Text(text) => text,
        TableCell::Status { label, .. } => label,
        TableCell::Meter { text, .. } => text,
    }
}

fn data(columns: &[DataColumnSpec], rows: &[&TableRow]) -> TableData {
    let mut builder = TableDataBuilder::with_capacity(columns, rows.len());
    for row in rows {
        builder
            .row(
                row.id.as_str(),
                row.cells.iter().map(|cell| Value::Text(text(cell))),
            )
            .tags(&row.tags);
    }
    builder.build()
}

fn paint(cell: &TableCell) -> Element {
    match cell {
        TableCell::Text(text) => rsx! { "{text}" },
        TableCell::Status {
            label,
            color: (r, g, b),
        } => rsx! {
            span { class: "pk-table-status", role: "status", aria_label: "{label}", style: "--status-c:rgb({r},{g},{b});",
                span { aria_hidden: "true", "\u{25cf}" }
                " {label}"
            }
        },
        TableCell::Meter { ratio, text, color } => {
            let percent = (ratio.clamp(0.0, 1.0) * 100.0).round() as u32;
            let style = color
                .map(|(r, g, b)| format!("--meter-c:rgb({r},{g},{b});"))
                .unwrap_or_default();
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
            }
        }
    }
}

/// Paint a static table model with the table component. The DOM id derives
/// from the column keys, since a model carries none of its own.
pub fn table(view: TableView<'_>) -> Element {
    let keys: Vec<&str> = view.columns.iter().map(|c| c.key.as_str()).collect();
    let columns = spec(view.columns);
    let rows = data(&columns, view.rows);
    let cells: Rc<Vec<Vec<TableCell>>> =
        Rc::new(view.rows.iter().map(|row| row.cells.clone()).collect());
    rsx! {
        DataTable {
            id: format!("pk-table-{}", keys.join("-")),
            columns,
            data: rows,
            cell: move |(row, column): (usize, usize)| {
                cells[row].get(column).map(paint).unwrap_or_else(|| rsx! {})
            },
            defaults: TableDefaults::default(),
        }
    }
}
