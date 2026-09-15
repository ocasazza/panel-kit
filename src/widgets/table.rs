//! Semantic table painter over borrowed core table models.

use dioxus::prelude::*;
use panel_kit_core::badge::Rgb;
use panel_kit_core::widgets::table::{ColumnWidth, TableCell, TableColumn, TableView, TextAlign};

fn align_css(align: TextAlign) -> &'static str {
    match align {
        TextAlign::Left => "left",
        TextAlign::Center => "center",
        TextAlign::Right => "right",
    }
}

fn rgb_var(name: &str, color: Rgb) -> String {
    let (r, g, b) = color;
    format!("--{name}:rgb({r},{g},{b});")
}

fn column_style(column: &TableColumn) -> String {
    let width = match column.width {
        ColumnWidth::Fixed { value } => format!("width:{value}ch;"),
        ColumnWidth::Flex { weight } => format!("width:{}fr;", weight.max(1)),
    };
    format!("{width}text-align:{};", align_css(column.align))
}

fn meter_percent(ratio: f64) -> u32 {
    (ratio.clamp(0.0, 1.0) * 100.0).round() as u32
}

fn cell(cell: &TableCell, align: TextAlign) -> Element {
    let align = align_css(align);
    match cell {
        TableCell::Text(text) => rsx! { td { style: "text-align:{align};", "{text}" } },
        TableCell::Status { label, color } => {
            let style = rgb_var("status-c", *color);
            rsx! {
                td { style: "text-align:{align};{style}",
                    span { class: "pk-table-status", role: "status", aria_label: "{label}",
                        span { aria_hidden: "true", "●" }
                        " {label}"
                    }
                }
            }
        }
        TableCell::Meter { ratio, text, color } => {
            let pct = meter_percent(*ratio);
            let style = color.map(|c| rgb_var("meter-c", c)).unwrap_or_default();
            rsx! {
                td { style: "text-align:{align};{style}",
                    span { class: "pk-table-meter", role: "meter", aria_valuemin: "0", aria_valuemax: "100", aria_valuenow: "{pct}", aria_label: "{text}",
                        span { class: "pk-table-meter-fill", style: "width:{pct}%;" }
                        span { class: "pk-table-meter-text", "{text}" }
                    }
                }
            }
        }
    }
}

/// Paint borrowed semantic table data as a native `<table>` with column scopes.
pub fn table(view: TableView<'_>) -> Element {
    rsx! {
        table { class: "pk-widget pk-table",
            thead {
                tr {
                    for column in view.columns.iter() {
                        th { scope: "col", style: "{column_style(column)}", "{column.title}" }
                    }
                }
            }
            tbody {
                for row in view.rows.iter() {
                    tr {
                        for (index, cell_value) in row.cells.iter().enumerate() {
                            {cell(cell_value, view.columns.get(index).map(|c| c.align).unwrap_or(TextAlign::Left))}
                        }
                    }
                }
            }
        }
    }
}
