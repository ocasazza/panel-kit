//! Interactive semantic table painter over borrowed core table models.

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
        TableCell::Text(text) => {
            rsx! { td { title: "{text}", style: "text-align:{align};", "{text}" } }
        }
        TableCell::Status { label, color } => {
            let style = rgb_var("status-c", *color);
            rsx! {
                td { title: "{label}", style: "text-align:{align};{style}",
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
                td { title: "{text}", style: "text-align:{align};{style}",
                    span { class: "pk-table-meter", role: "meter", aria_valuemin: "0", aria_valuemax: "100", aria_valuenow: "{pct}", aria_label: "{text}",
                        span { class: "pk-table-meter-fill", style: "width:{pct}%;" }
                        span { class: "pk-table-meter-text", "{text}" }
                    }
                }
            }
        }
    }
}

/// Paint borrowed semantic table data with host-owned selection and row actions.
///
/// `selected` only controls presentation; this painter never retains selection
/// state. `on_row_click` makes rows interactive when present, while `dense`
/// selects the compact padding variant. Every cell carries its complete value
/// in a `title` tooltip so clipped text remains available.
pub fn table(
    view: TableView<'_>,
    selected: Option<usize>,
    on_row_click: Option<EventHandler<usize>>,
    dense: bool,
) -> Element {
    let table_class = if dense {
        "pk-widget pk-table pk-table-dense"
    } else {
        "pk-widget pk-table"
    };
    let clickable = on_row_click.is_some();
    let column_count = view.columns.len().max(1);

    rsx! {
        div { class: "pk-table-wrap",
            table { class: "{table_class}",
                thead {
                    tr {
                        for column in view.columns.iter() {
                            th {
                                scope: "col",
                                title: "{column.title}",
                                style: "{column_style(column)}",
                                "{column.title}"
                            }
                        }
                    }
                }
                tbody {
                    for (row_index, row) in view.rows.iter().enumerate() {
                        {
                            let row_class = match (selected == Some(row_index), clickable) {
                                (true, true) => "pk-table-row selected clickable",
                                (true, false) => "pk-table-row selected",
                                (false, true) => "pk-table-row clickable",
                                (false, false) => "pk-table-row",
                            };
                            rsx! {
                                tr {
                                    key: "row-{row_index}",
                                    class: "{row_class}",
                                    aria_selected: selected.map(|index| index == row_index),
                                    onclick: move |_| {
                                        if let Some(handler) = on_row_click {
                                            handler.call(row_index);
                                        }
                                    },
                                    for (index, cell_value) in row.cells.iter().enumerate() {
                                        {cell(
                                            cell_value,
                                            view.columns
                                                .get(index)
                                                .map(|column| column.align)
                                                .unwrap_or(TextAlign::Left),
                                        )}
                                    }
                                }
                            }
                        }
                    }
                    if view.rows.is_empty() {
                        tr {
                            td {
                                class: "pk-table-empty",
                                colspan: "{column_count}",
                                "no rows"
                            }
                        }
                    }
                }
            }
        }
    }
}
