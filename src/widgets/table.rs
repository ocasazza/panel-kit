//! Interactive semantic table painter over borrowed core table models.

use dioxus::prelude::*;
use panel_kit_core::badge::Rgb;
use panel_kit_core::widgets::table::{
    ColumnWidth, Density, TableCell, TableColumn, TableCommand, TableView, TextAlign,
};

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

/// Paint one table episode: the ordered, filtered window the host computed,
/// with the host's sort, focus and selection shown.
///
/// The painter owns no state. A header click asks for `SortBy`, a row click
/// focuses by id, and both arrive through `on_command` — a host that wants a
/// read-only table passes `None` and gets plain headers.
pub fn table(view: TableView<'_>, on_command: Option<EventHandler<TableCommand>>) -> Element {
    let table_class = match view.state.density {
        Density::Dense => "pk-widget pk-table pk-table-dense",
        Density::Comfortable => "pk-widget pk-table",
    };
    let column_count = view.columns.len().max(1);
    let interactive = on_command.is_some();

    rsx! {
        div { class: "pk-table-wrap",
            table { class: "{table_class}", role: "grid",
                thead {
                    tr {
                        for column in view.columns.iter() {
                            {
                                let key = column.key.clone();
                                let clicked = key.clone();
                                let direction = view.state.sort_direction(&key);
                                let marker = match direction {
                                    Some(true) => " \u{25bc}",
                                    Some(false) => " \u{25b2}",
                                    None => "",
                                };
                                let header_class = match direction {
                                    Some(_) => "pk-table-sortable sorted",
                                    None => "pk-table-sortable",
                                };
                                rsx! {
                                    th {
                                        scope: "col",
                                        class: "{header_class}",
                                        title: "{column.title}",
                                        style: "{column_style(column)}",
                                        aria_sort: match direction {
                                            Some(true) => "descending",
                                            Some(false) => "ascending",
                                            None => "none",
                                        },
                                        onclick: move |_| {
                                            if let Some(handler) = on_command {
                                                handler.call(TableCommand::SortBy(clicked.clone()));
                                            }
                                        },
                                        "{column.title}{marker}"
                                    }
                                }
                            }
                        }
                    }
                }
                tbody {
                    for row in view.rows.iter().copied() {
                        {
                            let focused = view.state.is_focused(&row.id);
                            let selected = view.state.is_selected(&row.id);
                            let id = row.id.clone();
                            let row_class = match (focused, selected, interactive) {
                                (true, _, true) => "pk-table-row focused selected clickable",
                                (false, true, true) => "pk-table-row selected clickable",
                                (_, _, true) => "pk-table-row clickable",
                                _ => "pk-table-row",
                            };
                            rsx! {
                                tr {
                                    key: "{row.id}",
                                    class: "{row_class}",
                                    aria_selected: selected,
                                    aria_current: if focused { "true" } else { "false" },
                                    onclick: move |_| {
                                        if let Some(handler) = on_command {
                                            handler.call(TableCommand::Focus(id.clone()));
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
                                if view.total > 0 {
                                    "{view.total} rows, none matching the filter"
                                } else {
                                    "no rows"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
