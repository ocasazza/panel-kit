//! Searchable, sortable data table with user-selectable columns.
//!
//! The web painter for [`panel_kit_core::widgets::data_table`]: a filter box
//! matching any cell text, click-to-sort headers, a row count, and a
//! "columns" menu that shows or hides columns. Rows own pre-rendered cells
//! (links, badges, checkboxes) plus typed [`SortKey`]s, so sorting never
//! depends on markup. When `storage_key` is set, sort and column visibility
//! persist in `localStorage` (filter text deliberately does not).

use dioxus::prelude::*;
use panel_kit_core::widgets::data_table::{visible_rows, RowKeys, SortDir, TableQuery};

pub use panel_kit_core::widgets::data_table::{DataColumnSpec, SortKey};

/// One data-table row.
#[derive(Clone)]
pub struct DataRow {
    /// Stable identity used as the DOM key.
    pub key: String,
    /// Rendered cells, one `td` per declared column, in column order.
    pub cells: Vec<Element>,
    /// Sort keys, one per declared column.
    pub keys: Vec<SortKey>,
    /// Lowercased filter haystack.
    pub search: String,
    /// Render as the current selection.
    pub selected: bool,
}

impl PartialEq for DataRow {
    // Cells capture event handlers; never skip a render on a false "equal".
    fn eq(&self, _: &Self) -> bool {
        false
    }
}

impl DataRow {
    /// Empty row; append cells in column order.
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            cells: Vec::new(),
            keys: Vec::new(),
            search: String::new(),
            selected: false,
        }
    }

    fn index(&mut self, search: &str) {
        if !search.is_empty() {
            self.search.push_str(&search.to_lowercase());
            self.search.push('\u{1f}');
        }
    }

    /// Plain text cell: rendered, sorted, and searched by the same string.
    pub fn text(mut self, value: impl Into<String>) -> Self {
        let value = value.into();
        self.index(&value);
        self.keys.push(SortKey::text(&value));
        self.cells.push(rsx! { td { title: "{value}", "{value}" } });
        self
    }

    /// Text cell with an extra CSS class (e.g. `"muted"`).
    pub fn text_class(mut self, value: impl Into<String>, class: &'static str) -> Self {
        let value = value.into();
        self.index(&value);
        self.keys.push(SortKey::text(&value));
        self.cells
            .push(rsx! { td { class: "{class}", title: "{value}", "{value}" } });
        self
    }

    /// Numeric cell rendered with `display`, sorted by `value`.
    pub fn num(mut self, value: Option<f64>, display: impl Into<String>) -> Self {
        let display = display.into();
        self.index(&display);
        self.keys.push(SortKey::opt_num(value));
        self.cells
            .push(rsx! { td { class: "pk-dt-num", title: "{display}", "{display}" } });
        self
    }

    /// Arbitrary cell with an explicit sort key and filter text. The element
    /// must render one `td`.
    pub fn cell(mut self, element: Element, key: SortKey, search: &str) -> Self {
        self.index(search);
        self.keys.push(key);
        self.cells.push(element);
        self
    }

    /// Mark as the current selection.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }
}

#[cfg(target_arch = "wasm32")]
fn load_query(key: &str) -> Option<TableQuery> {
    use gloo_storage::{LocalStorage, Storage};
    LocalStorage::get::<TableQuery>(key).ok()
}

#[cfg(not(target_arch = "wasm32"))]
fn load_query(_key: &str) -> Option<TableQuery> {
    None
}

#[cfg(target_arch = "wasm32")]
fn save_query(key: &str, query: &TableQuery) {
    use gloo_storage::{LocalStorage, Storage};
    let _ = LocalStorage::set(key, query);
}

#[cfg(not(target_arch = "wasm32"))]
fn save_query(_key: &str, _query: &TableQuery) {}

/// Mutate the query and persist the durable parts.
fn apply(mut query: Signal<TableQuery>, key: Option<&str>, f: impl FnOnce(&mut TableQuery)) {
    let mut next = query.read().clone();
    f(&mut next);
    if let Some(key) = key {
        save_query(key, &next);
    }
    query.set(next);
}

/// Searchable, sortable table with a column-visibility menu.
///
/// `initial` seeds sort/visibility when nothing is persisted under
/// `storage_key`. Cells for hidden columns are simply not painted.
#[component]
pub fn DataTable(
    /// Column declarations, parallel to each row's cells and keys.
    columns: Vec<DataColumnSpec>,
    /// Rows in source order.
    rows: Vec<DataRow>,
    /// Initial query (sort, hidden columns) used when nothing is persisted.
    #[props(default)]
    initial: TableQuery,
    /// `localStorage` key for sort + column visibility. `None` = session only.
    #[props(default)]
    storage_key: Option<String>,
    /// Message when there are no rows at all.
    #[props(default = "no rows".to_string())]
    empty: String,
    /// Filter input placeholder.
    #[props(default = "filter…".to_string())]
    placeholder: String,
) -> Element {
    let mut query = use_signal({
        let key = storage_key.clone();
        move || key.as_deref().and_then(load_query).unwrap_or(initial)
    });
    let mut menu_open = use_signal(|| false);

    let q = query.read().clone();
    let row_keys: Vec<RowKeys> = rows
        .iter()
        .map(|r| RowKeys {
            search: &r.search,
            keys: &r.keys,
        })
        .collect();
    let shown = visible_rows(&columns, &row_keys, &q);
    let total = rows.len();
    let visible: Vec<bool> = columns.iter().map(|c| q.is_visible(c)).collect();
    let visible_count = visible.iter().filter(|v| **v).count().max(1);
    let hideable = columns.iter().any(|c| c.hideable);
    let count = if shown.len() == total {
        format!("{total}")
    } else {
        format!("{} / {total}", shown.len())
    };

    rsx! {
        div { class: "pk-widget pk-dt",
            div { class: "pk-dt-bar",
                input {
                    class: "pk-dt-filter",
                    r#type: "search",
                    placeholder: "{placeholder}",
                    aria_label: "filter rows",
                    value: "{q.filter}",
                    oninput: move |e| {
                        let text = e.value();
                        // Filter text is session state: not persisted.
                        query.write().filter = text;
                    },
                }
                span { class: "pk-dt-count", "{count}" }
                if hideable {
                    div { class: "pk-dt-cols",
                        button {
                            class: "pk-dt-cols-btn",
                            aria_expanded: "{menu_open()}",
                            onclick: move |_| menu_open.toggle(),
                            "columns \u{25be}"
                        }
                        if menu_open() {
                            div { class: "pk-dt-cols-menu", role: "menu",
                                for (i, c) in columns.iter().enumerate().filter(|(_, c)| c.hideable) {
                                    {
                                        let id = c.id.clone();
                                        let cols = columns.clone();
                                        let key = storage_key.clone();
                                        let title = if c.title.is_empty() { c.id.clone() } else { c.title.clone() };
                                        let on = visible[i];
                                        rsx! {
                                            label { class: "pk-dt-col-opt", key: "{c.id}",
                                                input {
                                                    r#type: "checkbox",
                                                    checked: on,
                                                    disabled: on && visible_count <= 1,
                                                    onchange: move |_| apply(query, key.as_deref(), |q| {
                                                        q.toggle_column(&cols, &id);
                                                    }),
                                                }
                                                " {title}"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            div { class: "pk-table-wrap pk-dt-wrap",
                table { class: "pk-table pk-table-dense",
                    thead {
                        tr {
                            for c in columns.iter().enumerate().filter(|(i, _)| visible[*i]).map(|(_, c)| c) {
                                {
                                    let arrow = match &q.sort {
                                        Some(s) if s.column == c.id && s.dir == SortDir::Asc => " \u{25b4}",
                                        Some(s) if s.column == c.id => " \u{25be}",
                                        _ => "",
                                    };
                                    let sort_state = match &q.sort {
                                        Some(s) if s.column == c.id && s.dir == SortDir::Asc => "ascending",
                                        Some(s) if s.column == c.id => "descending",
                                        _ => "none",
                                    };
                                    let id = c.id.clone();
                                    let cols = columns.clone();
                                    let key = storage_key.clone();
                                    let class = if c.sortable { "pk-dt-sortable" } else { "" };
                                    rsx! {
                                        th {
                                            key: "{c.id}",
                                            scope: "col",
                                            class: "{class}",
                                            aria_sort: "{sort_state}",
                                            onclick: move |_| apply(query, key.as_deref(), |q| q.click_header(&cols, &id)),
                                            "{c.title}{arrow}"
                                        }
                                    }
                                }
                            }
                        }
                    }
                    tbody {
                        if shown.is_empty() {
                            tr {
                                td { class: "pk-table-empty", colspan: "{visible_count}",
                                    if total == 0 { "{empty}" } else { "no rows match the filter" }
                                }
                            }
                        }
                        for i in shown {
                            tr {
                                key: "{rows[i].key}",
                                class: if rows[i].selected { "pk-table-row selected" } else { "pk-table-row" },
                                for (j, cell) in rows[i].cells.iter().enumerate() {
                                    if visible.get(j).copied().unwrap_or(true) {
                                        {cell.clone()}
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
