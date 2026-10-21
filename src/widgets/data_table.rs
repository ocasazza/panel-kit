//! Searchable, sortable data table with user-selectable columns.
//!
//! The web painter for [`panel_kit_core::widgets::data_table`]: a filter box
//! matching any cell text, click-to-sort headers, a row count, and a
//! "columns" menu that shows or hides columns. Rows own pre-rendered cells
//! (links, badges, checkboxes) plus typed [`SortKey`]s, so sorting never
//! depends on markup. When `storage_key` is set, sort and column visibility
//! persist in `localStorage` (filter text deliberately does not).

use dioxus::prelude::*;
use panel_kit_core::widgets::data_table::{
    grouped, visible_rows, Density, RowKeys, Selectable, TableEvent, TableKey, TableState, TagMode,
    ViewMode,
};

pub use panel_kit_core::widgets::data_table::{
    DataColumnSpec, SortDir, SortKey, SortSpec, TableDefaults,
};

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
    /// The row's tags, for the ribbon's filter.
    pub tags: Vec<String>,
    /// Each cell's text, for copy and CSV.
    pub text: Vec<String>,
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
            tags: Vec::new(),
            text: Vec::new(),
        }
    }

    /// Attach the row's tags.
    pub fn tags(mut self, tags: &[&str]) -> Self {
        self.tags = tags.iter().map(|tag| tag.to_string()).collect();
        self
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
        self.text.push(value.clone());
        self.cells.push(rsx! { td { title: "{value}", "{value}" } });
        self
    }

    /// Text cell with an extra CSS class (e.g. `"muted"`).
    pub fn text_class(mut self, value: impl Into<String>, class: &'static str) -> Self {
        let value = value.into();
        self.index(&value);
        self.keys.push(SortKey::text(&value));
        self.text.push(value.clone());
        self.cells
            .push(rsx! { td { class: "{class}", title: "{value}", "{value}" } });
        self
    }

    /// Numeric cell rendered with `display`, sorted by `value`.
    pub fn num(mut self, value: Option<f64>, display: impl Into<String>) -> Self {
        let display = display.into();
        self.index(&display);
        self.keys.push(SortKey::opt_num(value));
        self.text.push(display.clone());
        self.cells
            .push(rsx! { td { class: "pk-dt-num", title: "{display}", "{display}" } });
        self
    }

    /// Arbitrary cell with an explicit sort key and filter text. The element
    /// must render one `td`.
    pub fn cell(mut self, element: Element, key: SortKey, search: &str) -> Self {
        self.index(search);
        self.keys.push(key);
        self.text.push(search.to_string());
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
fn load_state(key: &str) -> Option<panel_kit_core::widgets::data_table::PersistedTable> {
    use gloo_storage::{LocalStorage, Storage};
    LocalStorage::get::<panel_kit_core::widgets::data_table::PersistedTable>(key).ok()
}

#[cfg(not(target_arch = "wasm32"))]
fn load_state(_key: &str) -> Option<panel_kit_core::widgets::data_table::PersistedTable> {
    None
}

#[cfg(target_arch = "wasm32")]
fn save_state(key: &str, state: &TableState) {
    use gloo_storage::{LocalStorage, Storage};
    let _ = LocalStorage::set(key, state.persisted());
}

#[cfg(not(target_arch = "wasm32"))]
fn save_state(_key: &str, _state: &TableState) {}

/// Whether focus is in a text field: a table that owns a key handler must not
/// turn typed text into row commands.
fn typing() -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        return web_sys::window()
            .and_then(|window| window.document())
            .and_then(|document| document.active_element())
            .map(|element| {
                let tag = element.tag_name();
                tag == "INPUT" || tag == "TEXTAREA" || element.has_attribute("contenteditable")
            })
            .unwrap_or(false);
    }
    #[cfg(not(target_arch = "wasm32"))]
    false
}

/// Rows as tab-separated text, a header line first: what a host puts on the
/// clipboard (the painter does not own a clipboard).
pub fn tsv(columns: &[DataColumnSpec], rows: &[DataRow], visible: &[bool], shown: &[usize]) -> String {
    let heads: Vec<&str> = columns
        .iter()
        .enumerate()
        .filter(|(index, _)| visible.get(*index).copied().unwrap_or(true))
        .map(|(_, column)| column.title.as_str())
        .collect();
    let mut out = vec![heads.join("\t")];
    for index in shown {
        let row = &rows[*index];
        let cells: Vec<&str> = row
            .text
            .iter()
            .enumerate()
            .filter(|(column, _)| visible.get(*column).copied().unwrap_or(true))
            .map(|(_, text)| text.as_str())
            .collect();
        out.push(cells.join("\t"));
    }
    out.join("\n")
}

/// Rows as CSV (RFC 4180 quoting), a header line first: what a host downloads.
pub fn csv(columns: &[DataColumnSpec], rows: &[DataRow], visible: &[bool], shown: &[usize]) -> String {
    fn quote(value: &str) -> String {
        match value.contains([',', '"', '\n']) {
            true => format!("\"{}\"", value.replace('"', "\"\"")),
            false => value.to_string(),
        }
    }
    let heads: Vec<String> = columns
        .iter()
        .enumerate()
        .filter(|(index, _)| visible.get(*index).copied().unwrap_or(true))
        .map(|(_, column)| quote(&column.title))
        .collect();
    let mut out = vec![heads.join(",")];
    for index in shown {
        let row = &rows[*index];
        let cells: Vec<String> = row
            .text
            .iter()
            .enumerate()
            .filter(|(column, _)| visible.get(*column).copied().unwrap_or(true))
            .map(|(_, text)| quote(text))
            .collect();
        out.push(cells.join(","));
    }
    out.join("\n")
}

/// The table: one component over one state.
///
/// A toolbar (quick filter, filter row, columns, group, density, copy, CSV and
/// the row count), a tag ribbon, sortable headers carrying their rank, a
/// selection column when the table is selectable, grouping headers that
/// collapse, and a virtual window that mounts a slice of the rows.
///
/// The component owns its state; `on_event` lets a host observe or mint its
/// own instead, and `storage_key` persists everything durable.
#[component]
pub fn DataTable(
    /// Column declarations, parallel to each row's cells and keys.
    columns: Vec<DataColumnSpec>,
    /// Rows in source order.
    rows: Vec<DataRow>,
    /// Declared defaults: initial sort, density, group, selection mode.
    #[props(default)]
    defaults: panel_kit_core::widgets::data_table::TableDefaults,
    /// `localStorage` key for the durable state. `None` = session only.
    #[props(default)]
    storage_key: Option<String>,
    /// Message when there are no rows at all.
    #[props(default = "no rows".to_string())]
    empty: String,
    /// Filter input placeholder.
    #[props(default = "filter…".to_string())]
    placeholder: String,
    /// Largest body height, in pixels, before it scrolls.
    #[props(default = 520.0)]
    max_height: f64,
    /// Called with the row key when a row is clicked, for a host that opens
    /// something on the click rather than only selecting.
    #[props(default)]
    on_row: Option<EventHandler<String>>,
    /// Called with the visible rows as TSV when COPY is pressed. `None` hides
    /// the control: the painter owns no clipboard.
    #[props(default)]
    on_copy: Option<EventHandler<String>>,
    /// Called with the visible rows as CSV when CSV is pressed. `None` hides
    /// the control: the painter owns no download.
    #[props(default)]
    on_csv: Option<EventHandler<String>>,
) -> Element {
    let mut state = use_signal({
        let columns = columns.clone();
        let key = storage_key.clone();
        move || {
            let mut fresh = TableState::new(&columns, defaults.clone());
            if let Some(payload) = key.as_deref().and_then(load_state) {
                fresh.restore(&payload, &columns);
            }
            fresh
        }
    });
    let mut menu_open = use_signal(|| false);

    let current = state.read().clone();
    let searched: Vec<RowKeys> = rows
        .iter()
        .map(|row| RowKeys {
            id: &row.key,
            search: &row.search,
            tags: &row.tags,
            keys: &row.keys,
        })
        .collect();
    let shown = visible_rows(&columns, &searched, &current);
    let ids: Vec<String> = shown.iter().map(|index| rows[*index].key.clone()).collect();
    let ordered = current.columns(&columns);
    let visible: Vec<bool> = columns.iter().map(|c| current.is_visible(c)).collect();
    let visible_flags: Vec<bool> = ordered.iter().map(|c| current.is_visible(c)).collect();
    let visible_count = visible_flags.iter().filter(|on| **on).count().max(1);
    let total = rows.len();
    let count = if shown.len() == total {
        format!("{total} ROWS")
    } else {
        format!("{} / {total} ROWS", shown.len())
    };
    let groups = grouped(&columns, &searched, &current, &shown);
    let tags = TableState::tag_counts(&searched);
    let mounted = current.mounted(shown.len());
    let density = current.density;
    let view_mode = current.view;
    let selectable = current.selectable;
    let row_height = mounted.row_height;

    let act = {
        let columns = columns.clone();
        let ids = ids.clone();
        let storage = storage_key.clone();
        EventHandler::new(move |event: TableEvent| {
            let mut next = state.read().clone();
            event.apply(&mut next, &columns, &ids);
            if let Some(key) = storage.as_deref() {
                save_state(key, &next);
            }
            state.set(next);
        })
    };

    let on_key = {
        let ids = ids.clone();
        let storage = storage_key.clone();
        move |event: KeyboardEvent| {
            if typing() || selectable == Selectable::None {
                return;
            }
            let key = match event.key() {
                Key::ArrowDown if event.modifiers().shift() => TableKey::ExtendDown,
                Key::ArrowUp if event.modifiers().shift() => TableKey::ExtendUp,
                Key::ArrowDown => TableKey::Down,
                Key::ArrowUp => TableKey::Up,
                Key::Home => TableKey::Home,
                Key::End => TableKey::End,
                Key::PageDown => TableKey::PageDown,
                Key::PageUp => TableKey::PageUp,
                Key::Escape => TableKey::ClearSelection,
                Key::Character(text) if text == " " => TableKey::Toggle,
                Key::Character(text) if text == "a" && event.modifiers().ctrl() => TableKey::SelectAll,
                _ => return,
            };
            event.prevent_default();
            let mut next = state.read().clone();
            next.key(key, &ids, 20);
            if let Some(storage) = storage.as_deref() {
                save_state(storage, &next);
            }
            state.set(next);
        }
    };

    let body_style = format!("max-height:{max_height}px;");
    let spacer = format!("height:{}px;", row_height * mounted.before as f64);
    let tail = format!("height:{}px;", row_height * mounted.after as f64);
    let colspan = visible_count + usize::from(selectable != Selectable::None);
    let all_selected = !ids.is_empty() && ids.iter().all(|id| current.selection.contains(id));

    rsx! {
        div { class: "pk-widget pk-dt", tabindex: "0", onkeydown: on_key,
            div { class: "pk-dt-bar",
                input {
                    class: "pk-dt-filter",
                    r#type: "search",
                    placeholder: "{placeholder}",
                    aria_label: "filter rows",
                    value: "{current.quick}",
                    oninput: move |e| {
                        let mut next = state.read().clone();
                        // Filter text is session state: not persisted.
                        next.set_quick(e.value());
                        state.set(next);
                    },
                }
                button {
                    class: if current.filters_open { "pk-dt-btn on" } else { "pk-dt-btn" },
                    aria_expanded: "{current.filters_open}",
                    onclick: move |_| act.call(TableEvent::FiltersOpen(!current.filters_open)),
                    "filters"
                }
                span { class: "pk-dt-count", "{count}" }
                if columns.iter().any(|c| c.hideable) {
                    div { class: "pk-dt-cols",
                        button {
                            class: "pk-dt-btn",
                            aria_expanded: "{menu_open()}",
                            onclick: move |_| menu_open.toggle(),
                            "columns \u{25be}"
                        }
                        if menu_open() {
                            div { class: "pk-dt-cols-menu", role: "menu",
                                for (index, column) in ordered.iter().enumerate().filter(|(_, c)| c.hideable) {
                                    {
                                        let id = column.id.clone();
                                        let title = if column.title.is_empty() { column.id.clone() } else { column.title.clone() };
                                        let on = visible_flags[index];
                                        rsx! {
                                            label { class: "pk-dt-col-opt", key: "{column.id}",
                                                input {
                                                    r#type: "checkbox",
                                                    checked: on,
                                                    disabled: on && visible_count <= 1,
                                                    onchange: move |_| act.call(TableEvent::ToggleColumn(id.clone())),
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
                select {
                    class: "pk-dt-group",
                    aria_label: "group rows by",
                    value: "{current.group_by.clone().unwrap_or_default()}",
                    onchange: move |e| {
                        let value = e.value();
                        act.call(TableEvent::Group((!value.is_empty()).then_some(value)));
                    },
                    option { value: "", "group: none" }
                    for column in columns.iter().filter(|c| !c.title.is_empty()) {
                        option { value: "{column.id}", "group: {column.title}" }
                    }
                }
                button {
                    class: "pk-dt-btn",
                    onclick: move |_| {
                        let next = match density {
                            Density::Normal => Density::Compact,
                            Density::Compact => Density::Normal,
                        };
                        act.call(TableEvent::Density(next));
                    },
                    title: "row density",
                    match density {
                        Density::Normal => "normal",
                        Density::Compact => "compact",
                    }
                }
                button {
                    class: "pk-dt-btn",
                    title: "rows, compact cards, or detail cards",
                    onclick: move |_| act.call(TableEvent::View(view_mode.next())),
                    "{view_mode.label()}"
                }
            }
            if !tags.is_empty() {
                div { class: "pk-dt-ribbon", role: "toolbar", aria_label: "tags",
                    for (tag, n) in tags.iter().take(if current.tags_expand { tags.len() } else { 12 }) {
                        {
                            let on = current.tag_filter.contains(tag);
                            let name = tag.clone();
                            rsx! {
                                button {
                                    class: if on { "pk-dt-tag on" } else { "pk-dt-tag" },
                                    key: "{tag}",
                                    aria_pressed: on,
                                    onclick: move |_| act.call(TableEvent::Tag(name.clone())),
                                    "#{tag} {n}"
                                }
                            }
                        }
                    }
                    if tags.len() > 12 {
                        button {
                            class: "pk-dt-tag",
                            onclick: move |_| act.call(TableEvent::TagsExpand(!current.tags_expand)),
                            if current.tags_expand { "less" } else { "more" }
                        }
                    }
                    if !current.tag_filter.is_empty() {
                        button {
                            class: "pk-dt-tag",
                            title: "any or all of the filtered tags",
                            onclick: move |_| {
                                let next = match current.tag_mode {
                                    TagMode::Any => TagMode::All,
                                    TagMode::All => TagMode::Any,
                                };
                                act.call(TableEvent::TagMode(next));
                            },
                            match current.tag_mode {
                                TagMode::Any => "any",
                                TagMode::All => "all",
                            }
                        }
                    }
                }
            }
            div { class: "pk-table-wrap pk-dt-wrap", style: "{body_style}",
                table { class: "pk-table pk-table-dense", role: "grid",
                    thead {
                        tr {
                            if selectable != Selectable::None {
                                th { scope: "col", class: "pk-dt-check",
                                    input {
                                        r#type: "checkbox",
                                        checked: all_selected,
                                        aria_label: "select every row",
                                        disabled: selectable != Selectable::Multi,
                                        onchange: move |_| act.call(TableEvent::SelectAll),
                                    }
                                }
                            }
                            for column in ordered.iter() {
                                if current.is_visible(column) {
                                    {
                                        let id = column.id.clone();
                                        let direction = current.direction(&column.id);
                                        let rank = current.sort_rank(&column.id);
                                        let arrow = match direction {
                                            Some(SortDir::Asc) => " \u{25b4}",
                                            Some(SortDir::Desc) => " \u{25be}",
                                            None => "",
                                        };
                                        let rank = match rank {
                                            Some(rank) if rank > 1 => format!(" {rank}"),
                                            _ => String::new(),
                                        };
                                        let sort_state = match direction {
                                            Some(SortDir::Asc) => "ascending",
                                            Some(SortDir::Desc) => "descending",
                                            None => "none",
                                        };
                                        let class = if column.sortable { "pk-dt-sortable" } else { "" };
                                        let width = current.width(&column.id);
                                        let style = width.map(|w| format!("width:{w}px;")).unwrap_or_default();
                                        rsx! {
                                            th {
                                                key: "{column.id}",
                                                scope: "col",
                                                class: "{class}",
                                                style: "{style}",
                                                aria_sort: "{sort_state}",
                                                onclick: move |_| act.call(TableEvent::Sort(id.clone())),
                                                "{column.title}{arrow}{rank}"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        if current.filters_open {
                            tr { class: "pk-dt-filters",
                                if selectable != Selectable::None {
                                    th {}
                                }
                                for column in ordered.iter() {
                                    if current.is_visible(column) {
                                        {
                                            let id = column.id.clone();
                                            rsx! {
                                                th {
                                                    th {
                                                        input {
                                                            r#type: "search",
                                                            class: "pk-dt-col-filter",
                                                            aria_label: "filter {column.title}",
                                                            oninput: move |e| act.call(TableEvent::Filter(id.clone(), e.value())),
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
                    tbody {
                        if shown.is_empty() {
                            tr {
                                td { class: "pk-table-empty", colspan: "{colspan}",
                                    if total == 0 { "{empty}" } else { "no rows match the filter" }
                                }
                            }
                        }
                        if mounted.before > 0 {
                            tr { aria_hidden: "true", td { colspan: "{colspan}", style: "{spacer}" } }
                        }
                        for (group, indices) in groups.iter() {
                            if !group.is_empty() {
                                {
                                    let collapsed = current.is_collapsed(group);
                                    let name = group.clone();
                                    rsx! {
                                        tr { class: "pk-dt-group-row", key: "group-{group}",
                                            td { colspan: "{colspan}",
                                                button {
                                                    class: "pk-dt-group-toggle",
                                                    aria_expanded: (!collapsed).to_string(),
                                                    onclick: move |_| act.call(TableEvent::ToggleGroup(name.clone())),
                                                    if collapsed { "\u{25b8}" } else { "\u{25be}" }
                                                    " {group} {indices.len()}"
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            if group.is_empty() || !current.is_collapsed(group) {
                                for index in indices.iter().copied().skip(mounted.start.saturating_sub(mounted.before)).take(mounted.end.saturating_sub(mounted.start)) {
                                    {
                                        let row = &rows[index];
                                        let key = row.key.clone();
                                        let cursor = current.is_cursor(&key);
                                        let selected = current.is_selected(&key);
                                        let class = match (cursor, selected) {
                                            (true, _) => "pk-table-row pk-dt-cursor selected",
                                            (false, true) => "pk-table-row selected",
                                            _ => "pk-table-row",
                                        };
                                        let shift_key = key.clone();
                                        rsx! {
                                            tr {
                                                key: "{key}",
                                                class: "{class}",
                                                aria_selected: selected,
                                                aria_current: cursor.to_string(),
                                                onclick: move |event: MouseEvent| {
                                                    act.call(TableEvent::Cursor(shift_key.clone()));
                                                    act.call(TableEvent::Select { id: shift_key.clone(), shift: event.modifiers().shift() });
                                                    if let Some(handler) = on_row {
                                                        handler.call(shift_key.clone());
                                                    }
                                                },
                                                if view_mode != ViewMode::Rows {
                                                    td { colspan: "{colspan}",
                                                        div {
                                                            class: if view_mode == ViewMode::CompactCards { "pk-dt-card compact" } else { "pk-dt-card detail" },
                                                            div { class: "pk-dt-card-title",
                                                                for (j, text) in row.text.iter().enumerate() {
                                                                    if j == 0 {
                                                                        span { class: "pk-dt-card-id", "{text}" }
                                                                    }
                                                                }
                                                                for (j, text) in row.text.iter().enumerate() {
                                                                    if j == 2 {
                                                                        span { class: "pk-dt-card-head", "{text}" }
                                                                    }
                                                                }
                                                            }
                                                            if view_mode == ViewMode::DetailCards {
                                                                div { class: "pk-dt-card-meta",
                                                                    for (j, column) in ordered.iter().enumerate() {
                                                                        if j != 0 && j != 2 && current.is_visible(column) {
                                                                            span {
                                                                                class: "pk-dt-field",
                                                                                "data-label": "{column.title}",
                                                                                span { class: "pk-dt-field-label", "{column.title}" }
                                                                                span { class: "pk-dt-field-value", "{row.text.get(j).cloned().unwrap_or_default()}" }
                                                                            }
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                            if !row.tags.is_empty() {
                                                                div { class: "pk-dt-card-tags",
                                                                    for tag in row.tags.iter() {
                                                                        span { class: "pk-chip", "#{tag}" }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                                if selectable != Selectable::None {
                                                    td { class: "pk-dt-check",
                                                        input {
                                                            r#type: "checkbox",
                                                            checked: selected,
                                                            aria_label: "select {key}",
                                                            disabled: selectable == Selectable::None,
                                                            onchange: move |_| act.call(TableEvent::Select { id: key.clone(), shift: false }),
                                                        }
                                                    }
                                                }
                                                if view_mode == ViewMode::Rows {
                                                    for (j, cell) in row.cells.iter().enumerate() {
                                                        if ordered.get(j).map(|c| current.is_visible(c)).unwrap_or(true) {
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
                        if mounted.after > 0 {
                            tr { aria_hidden: "true", td { colspan: "{colspan}", style: "{tail}" } }
                        }
                    }
                }
            }
        }
    }
}
