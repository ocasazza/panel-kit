//! The one table component: a dense, keyboard-driven grid with sorting,
//! filtering, grouping, selection, column layout, a tag ribbon and a virtual
//! window, painted from [`panel_kit_core::widgets::data_table`].
//!
//! The markup follows the board's LiveView data table element for element
//! (`dt-*` there, `pk-dt-*` here), so its stylesheet and its tests port
//! unchanged in meaning.

mod dom;
mod row;

use std::collections::BTreeSet;

use dioxus::prelude::*;
use panel_kit_core::widgets::data_table::{
    empty_sentence, ribbon_label, row_labels, segment_label, tag_letter, tag_slot, tally,
    ColumnFilter, ColumnKind, Entry, MoveTarget, QueryMode, RibbonMode, RowKeys, SelectMode,
    Selectable, SortDir, TableEvent, TableKey, TableQuery, TableState, View, ViewMode,
    DEFAULT_PAGE, NARROW_WIDTH, OVERSCAN,
};

pub use panel_kit_core::widgets::data_table::{
    csv, tsv, Align, DataColumnSpec, Density, SortKey, SortSpec, TableDefaults,
};
pub use row::DataRow;

/// A toolbar control, in the order the bar draws them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Control {
    /// The quick filter box.
    Quick,
    /// The filter-row toggle and the clear button.
    Filters,
    /// The columns menu.
    Columns,
    /// The group-by select.
    Group,
    /// The density toggle.
    Density,
    /// The rows/cards toggle.
    View,
    /// Copy as TSV.
    Copy,
    /// Download as CSV.
    Export,
    /// The row count.
    Tally,
}

impl Control {
    /// Every control.
    pub const ALL: [Control; 9] = [
        Control::Quick,
        Control::Filters,
        Control::Columns,
        Control::Group,
        Control::Density,
        Control::View,
        Control::Copy,
        Control::Export,
        Control::Tally,
    ];
}

/// The table.
///
/// It owns its [`TableState`]: `storage_key` persists the durable part,
/// `on_event` observes every change, and `on_query` puts it in server mode,
/// where the host re-queries and the table only renders and groups.
#[component]
pub fn DataTable(
    /// DOM id; row, header and control ids derive from it.
    id: String,
    /// Column declarations, parallel to each row's cells.
    columns: Vec<DataColumnSpec>,
    /// Rows in source order.
    rows: Vec<DataRow>,
    /// Declared sort, density, grouping, selection mode, view.
    #[props(default)]
    defaults: TableDefaults,
    /// `localStorage` key for the durable state.
    #[props(default)]
    storage_key: Option<String>,
    /// The grid's accessible name.
    #[props(default)]
    label: Option<String>,
    /// What to say when there are no rows at all.
    #[props(default)]
    empty: Option<Element>,
    /// Quick filter placeholder.
    #[props(default = "filter rows".to_string())]
    placeholder: String,
    /// A click or Enter on a row calls this with its id instead of selecting.
    #[props(default)]
    on_row: Option<EventHandler<String>>,
    /// The host's own selection; `None` defers to the table's.
    #[props(default)]
    selected: Option<Vec<String>>,
    /// Draw each row's tags as a ribbon in a gutter column.
    #[props(default = true)]
    ribbon: bool,
    /// Segments drawn before the `+N` tail.
    #[props(default = 4)]
    tags_limit: usize,
    /// How much of the ribbon to draw.
    #[props(default)]
    tags_mode: RibbonMode,
    /// Mount only the window of rows the scroller shows.
    #[props(default)]
    windowed: bool,
    /// Which toolbar controls to show; `None` shows all.
    #[props(default)]
    toolbar: Option<Vec<Control>>,
    /// A full-width footer row.
    #[props(default)]
    footer: Option<Element>,
    /// Receives the TSV that COPY or ctrl-C produce; the clipboard otherwise.
    #[props(default)]
    on_copy: Option<EventHandler<String>>,
    /// Receives the CSV that CSV produces; a download otherwise.
    #[props(default)]
    on_csv: Option<EventHandler<String>>,
    /// Server mode: called with the query whenever it changes.
    #[props(default)]
    on_query: Option<EventHandler<TableQuery>>,
    /// Observes every event the table applies.
    #[props(default)]
    on_event: Option<EventHandler<TableEvent>>,
    /// A host-owned state: the table paints it and hands every event to
    /// `on_event` for the host to apply, instead of keeping its own.
    #[props(default)]
    state: Option<TableState>,
    /// Largest height, in pixels, before the grid scrolls.
    #[props(default)]
    max_height: Option<f64>,
) -> Element {
    let mut owned = use_signal({
        let columns = columns.clone();
        let stored = storage_key.as_deref().and_then(dom::load);
        move || {
            let mut fresh = TableState::new(&columns, defaults.clone());
            if let Some(payload) = stored {
                fresh.restore(&payload);
            }
            fresh
        }
    });
    let mut dragging = use_signal(|| None::<String>);
    let mut drop_mark = use_signal(|| None::<(String, bool)>);
    // A column being dragged wider or narrower: (column, pointer x at the
    // start, width at the start, live width).
    let mut resizing = use_signal(|| None::<(String, f64, f64, f64)>);
    let mut asked = use_signal(BTreeSet::<String>::new);
    let mut folded = use_signal(BTreeSet::<String>::new);
    let mut focused = use_signal(|| false);

    let host_owned = state.is_some();
    let mut current = state.unwrap_or_else(|| owned.read().clone());
    current.sync(&columns);
    let server = on_query.is_some();
    let keyed: Vec<RowKeys> = rows.iter().map(|row| row.keys(ribbon)).collect();
    let mode = if server {
        QueryMode::Server
    } else {
        QueryMode::Client
    };
    let view = View::build(&columns, &keyed, &current, mode);
    let tagged = ribbon && rows.iter().any(|row| !row.tags.is_empty());

    let index_of = |column: &str| columns.iter().position(|c| c.id == column);
    let ordered: Vec<DataColumnSpec> = current.ordered(&columns).into_iter().cloned().collect();
    let visible: Vec<DataColumnSpec> = current.visible(&columns).into_iter().cloned().collect();
    let visible_at: Vec<usize> = visible.iter().filter_map(|c| index_of(&c.id)).collect();
    let selectable = current.selectable;
    let multi = selectable == Selectable::Multi;
    let selection: BTreeSet<String> = match &selected {
        Some(ids) => ids.iter().cloned().collect(),
        None => current.selection.clone(),
    };
    let filterable = columns.iter().any(|c| c.filterable);
    let header_rows = if current.filters_open && visible.iter().any(|c| c.filterable) {
        2
    } else {
        1
    };
    let colspan = visible.len() + usize::from(multi) + usize::from(tagged);
    let mounted = current.mounted(view.entries.len(), windowed);
    let row_height = current.row_height();
    let first_row = view.entries[mounted.start..mounted.end]
        .iter()
        .position(|e| matches!(e, Entry::Row(_)))
        .map(|at| at + mounted.start);
    let controls = toolbar.clone().unwrap_or_else(|| Control::ALL.to_vec());
    let shows = |control: Control| controls.contains(&control);
    let narrow: Vec<String> = columns
        .iter()
        .filter(|c| c.narrow)
        .map(|c| c.id.clone())
        .collect();
    let scroll_id = format!("{id}-scroll");
    let tag_focusing = current.tag_focus.is_some();

    // Every change goes through here: apply, persist, tell the host.
    let act = {
        let columns = columns.clone();
        let view = view.clone();
        let storage = storage_key.clone();
        EventHandler::new(move |event: TableEvent| {
            if host_owned {
                if let Some(on_event) = on_event {
                    on_event.call(event);
                }
                return;
            }
            let mut next = owned.read().clone();
            next.sync(&columns);
            event.apply(&mut next, &columns, &view);
            if event.persists() {
                if let Some(key) = storage.as_deref() {
                    dom::save(key, &next.persisted());
                }
            }
            if let (Some(on_query), true) = (on_query, event.requeries()) {
                on_query.call(next.query());
            }
            if let Some(on_event) = on_event {
                on_event.call(event);
            }
            owned.set(next);
        })
    };

    // The rows an export carries: the selection when there is one, in view
    // order, else every row shown.
    let export = {
        let rows = rows.clone();
        let visible = visible.clone();
        let visible_at = visible_at.clone();
        let view = view.clone();
        let selection = selection.clone();
        move |selected_only: bool| -> (Vec<String>, Vec<Vec<String>>) {
            let heads = visible.iter().map(|c| c.title.clone()).collect();
            let picked: Vec<usize> = view
                .row_indices()
                .filter(|&at| {
                    !selected_only || selection.is_empty() || selection.contains(&rows[at].key)
                })
                .collect();
            let body = picked
                .iter()
                .map(|&at| {
                    visible_at
                        .iter()
                        .map(|&c| rows[at].text.get(c).cloned().unwrap_or_default())
                        .collect()
                })
                .collect();
            (heads, body)
        }
    };
    let copy = {
        let export = export.clone();
        move || {
            let (heads, body) = export(true);
            let text = tsv(&heads, &body);
            match on_copy {
                Some(handler) => handler.call(text),
                None => dom::copy(&text),
            }
        }
    };
    let download = {
        let export = export.clone();
        let name = format!("{id}.csv");
        move || {
            let (heads, body) = export(false);
            let text = csv(&heads, &body);
            match on_csv {
                Some(handler) => handler.call(text),
                None => dom::download(&name, "text/csv", &text),
            }
        }
    };

    // The cursor row is the one tab stop; when it moves while the grid has
    // focus, focus follows it.
    let cursor_dom = current.cursor.as_ref().map(|c| format!("{id}-{c}"));
    use_effect(use_reactive!(|(cursor_dom,)| {
        if let Some(dom_id) = cursor_dom {
            if *focused.peek() {
                dom::focus(&dom_id);
            }
        }
    }));

    let on_key = {
        let rows = rows.clone();
        let snapshot = current.clone();
        let scroll_id = scroll_id.clone();
        let copy = copy.clone();
        move |event: KeyboardEvent| {
            if dom::typing() {
                return;
            }
            let modifiers = event.modifiers();
            let ctrl = modifiers.ctrl() || modifiers.meta();
            let shift = modifiers.shift();
            let key = event.key();
            let now = snapshot.clone();

            if let Some((row, tag)) = now.tag_focus.clone() {
                let labels = rows
                    .iter()
                    .find(|r| r.key == row)
                    .map(|r| row_labels(&r.tags))
                    .unwrap_or_default();
                let at = labels.iter().position(|l| *l == tag);
                let handled = match &key {
                    Key::Escape => Some(TableEvent::TagFocus(None)),
                    Key::Character(c) if c == "t" && !shift => Some(TableEvent::TagFocus(None)),
                    Key::ArrowRight | Key::ArrowLeft => at
                        .and_then(|at| {
                            let to = if key == Key::ArrowRight {
                                at.checked_add(1)
                            } else {
                                at.checked_sub(1)
                            };
                            labels.get(to?)
                        })
                        .map(|next| TableEvent::TagFocus(Some((row.clone(), next.clone())))),
                    Key::Enter => Some(TableEvent::Tag {
                        tag: tag.clone(),
                        all: shift,
                    }),
                    Key::Character(c) if c == " " => Some(TableEvent::Tag {
                        tag: tag.clone(),
                        all: shift,
                    }),
                    _ => None,
                };
                if let Some(next) = handled {
                    event.prevent_default();
                    event.stop_propagation();
                    act.call(next);
                    return;
                }
            }

            if let (Key::Character(c), false, Some(row)) =
                (&key, ctrl || modifiers.alt(), now.cursor.clone())
            {
                if c == "t" {
                    let first = rows
                        .iter()
                        .find(|r| r.key == row)
                        .and_then(|r| row_labels(&r.tags).into_iter().next());
                    if let Some(first) = first {
                        event.prevent_default();
                        act.call(TableEvent::TagFocus(Some((row, first))));
                    }
                    return;
                }
            }

            if ctrl && matches!(&key, Key::Character(c) if c.eq_ignore_ascii_case("c")) {
                if !dom::selecting_text() {
                    event.prevent_default();
                    copy();
                }
                return;
            }

            let table_key = match &key {
                Key::ArrowDown => TableKey::Down,
                Key::ArrowUp => TableKey::Up,
                Key::Home => TableKey::Home,
                Key::End => TableKey::End,
                Key::PageDown => TableKey::PageDown,
                Key::PageUp => TableKey::PageUp,
                Key::Escape => TableKey::Escape,
                Key::Character(c) if c == " " => TableKey::Space,
                Key::Character(c) if ctrl && c.eq_ignore_ascii_case("a") => TableKey::SelectAll,
                _ => return,
            };
            event.prevent_default();
            let page = dom::scroll(&scroll_id)
                .map(|s| {
                    ((s.visible / s.row.unwrap_or(row_height)).floor() as usize)
                        .saturating_sub(1)
                        .max(1)
                })
                .unwrap_or(DEFAULT_PAGE);
            act.call(TableEvent::Key {
                key: table_key,
                shift,
                page,
            });
        }
    };

    let on_scroll = {
        let scroll_id = scroll_id.clone();
        let total = view.entries.len();
        let now = current.mounted(total, true);
        move |_| {
            if !windowed || total == 0 {
                return;
            }
            let Some(scroll) = dom::scroll(&scroll_id) else {
                return;
            };
            let height = scroll.row.unwrap_or(row_height);
            let first = (scroll.top / height).floor() as usize;
            let shown = (scroll.visible / height).ceil() as usize + 1;
            let need_end = (first + shown).min(total);
            if first < now.start || need_end > now.end || now.end == now.start {
                act.call(TableEvent::Window {
                    start: first.saturating_sub(OVERSCAN),
                    count: shown + 2 * OVERSCAN,
                    row_height: Some(height),
                });
            }
        }
    };

    // A table narrower than NARROW_WIDTH folds its `narrow` columns, except
    // the ones the user asked for by name; widening shows them again.
    let on_resize = {
        let narrow = narrow.clone();
        let columns = columns.clone();
        let now = current.clone();
        move |event: Event<ResizeData>| {
            let Ok(size) = event.get_border_box_size() else {
                return;
            };
            if size.width < NARROW_WIDTH {
                for key in &narrow {
                    let shown = columns.iter().any(|c| &c.id == key && now.is_visible(c));
                    if shown && !asked.peek().contains(key) {
                        folded.write().insert(key.clone());
                        act.call(TableEvent::ToggleColumn(key.clone()));
                    }
                }
            } else {
                let back: Vec<String> = folded.peek().iter().cloned().collect();
                for key in back {
                    if now.hidden.contains(&key) {
                        act.call(TableEvent::ToggleColumn(key.clone()));
                    }
                    folded.write().remove(&key);
                }
            }
        }
    };

    let style = max_height
        .map(|h| format!("max-height:{h}px;"))
        .unwrap_or_default();
    let cursor_attr = current.cursor.clone();
    let narrow_attr = (!narrow.is_empty()).then(|| narrow.join(" "));
    let all_checked = !view.ids.is_empty() && view.ids.iter().all(|i| selection.contains(i));
    let filter_count = current.filters.len();

    rsx! {
        div {
            id: "{id}",
            class: "pk-widget pk-dt",
            style: "{style}",
            "data-density": current.density.as_str(),
            "data-virtual": windowed.then_some("true"),
            "data-start": "{mounted.start}",
            "data-row-height": "{row_height}",
            "data-total": "{view.entries.len()}",
            "data-cursor": cursor_attr,
            "data-selectable": match selectable { Selectable::None => None, Selectable::Single => Some("single"), Selectable::Multi => Some("multi") },
            "data-tags": tagged.then_some("true"),
            "data-tags-mode": tagged.then_some(tags_mode.as_str()),
            "data-narrow-cols": narrow_attr,
            onkeydown: on_key,
            onfocusin: move |_| focused.set(true),
            onfocusout: {
                let root = id.clone();
                move |e: FocusEvent| {
                    if dom::left_for_outside(&e, &root) {
                        focused.set(false);
                    }
                }
            },
            onresize: on_resize,

            if !controls.is_empty() {
                div { class: "pk-dt-bar", role: "toolbar", aria_label: "{label.clone().unwrap_or(id.clone())} controls",
                    if shows(Control::Quick) {
                        div { class: "pk-dt-quick",
                            input {
                                id: "{id}-quick",
                                r#type: "search",
                                name: "q",
                                value: "{current.quick}",
                                placeholder: "{placeholder}",
                                aria_label: "filter rows",
                                autocomplete: "off",
                                oninput: move |e| act.call(TableEvent::Quick(e.value())),
                            }
                        }
                    }
                    if shows(Control::Filters) && filterable {
                        button {
                            r#type: "button",
                            class: if current.filters_open { "pk-dt-btn on" } else { "pk-dt-btn" },
                            aria_pressed: "{current.filters_open}",
                            aria_controls: "{id}-filters",
                            onclick: move |_| act.call(TableEvent::ToggleFilters),
                            "filters"
                            if filter_count > 0 {
                                span { class: "pk-dt-n", "{filter_count}" }
                            }
                        }
                    }
                    if shows(Control::Filters) && current.filtering() {
                        button { r#type: "button", class: "pk-dt-btn", onclick: move |_| act.call(TableEvent::Clear), "clear" }
                    }
                    if shows(Control::Columns) {
                        details { class: "pk-dt-menu", "data-dt-menu": "columns",
                            summary { class: "pk-dt-btn", "columns" }
                            div { class: "pk-dt-menu-body", role: "group", aria_label: "columns",
                                for (index, column) in ordered.iter().enumerate() {
                                    {
                                        let key = column.id.clone();
                                        let left = key.clone();
                                        let right = key.clone();
                                        let last = ordered.len() - 1;
                                        rsx! {
                                            div { class: "pk-dt-menu-row", key: "{column.id}",
                                                label { class: "pk-dt-check",
                                                    input {
                                                        r#type: "checkbox",
                                                        checked: current.is_visible(column),
                                                        disabled: !column.hideable,
                                                        onclick: move |_| {
                                                            asked.write().insert(key.clone());
                                                            act.call(TableEvent::ToggleColumn(key.clone()));
                                                        },
                                                    }
                                                    " {column.title}"
                                                }
                                                button {
                                                    r#type: "button",
                                                    class: "pk-dt-mini",
                                                    disabled: index == 0,
                                                    aria_label: "move {column.title} left",
                                                    onclick: move |_| act.call(TableEvent::MoveColumn(left.clone(), MoveTarget::Left)),
                                                    "\u{25c2}"
                                                }
                                                button {
                                                    r#type: "button",
                                                    class: "pk-dt-mini",
                                                    disabled: index == last,
                                                    aria_label: "move {column.title} right",
                                                    onclick: move |_| act.call(TableEvent::MoveColumn(right.clone(), MoveTarget::Right)),
                                                    "\u{25b8}"
                                                }
                                            }
                                        }
                                    }
                                }
                                button { r#type: "button", class: "pk-dt-btn", onclick: move |_| act.call(TableEvent::Reset), "reset layout" }
                            }
                        }
                    }
                    if shows(Control::Group) {
                        div { class: "pk-dt-group",
                            label { class: "pk-dt-field",
                                "group"
                                select {
                                    name: "col",
                                    aria_label: "group rows by",
                                    onchange: move |e| {
                                        let value = e.value();
                                        act.call(TableEvent::Group((!value.is_empty()).then_some(value)));
                                    },
                                    option { value: "", selected: current.group_by.is_none(), "none" }
                                    for column in ordered.iter() {
                                        option {
                                            key: "{column.id}",
                                            value: "{column.id}",
                                            selected: current.group_by.as_deref() == Some(column.id.as_str()),
                                            "{column.title}"
                                        }
                                    }
                                }
                            }
                        }
                    }
                    if shows(Control::Density) {
                        button {
                            r#type: "button",
                            class: "pk-dt-btn",
                            aria_pressed: "{current.density == Density::Compact}",
                            title: "toggle row density",
                            onclick: move |_| act.call(TableEvent::ToggleDensity),
                            "{current.density.as_str()}"
                        }
                    }
                    if shows(Control::View) {
                        button {
                            r#type: "button",
                            class: "pk-dt-btn",
                            title: "rows, compact cards, or detail cards",
                            onclick: move |_| act.call(TableEvent::View(current.view.next())),
                            "{current.view.label()}"
                        }
                    }
                    if shows(Control::Copy) {
                        button {
                            r#type: "button",
                            class: "pk-dt-btn",
                            title: "copy the selected rows as TSV (every row when nothing is selected)",
                            onclick: {
                                let copy = copy.clone();
                                move |_| copy()
                            },
                            "copy"
                        }
                    }
                    if shows(Control::Export) {
                        button {
                            r#type: "button",
                            class: "pk-dt-btn",
                            title: "download the rows shown as CSV",
                            onclick: {
                                let download = download.clone();
                                move |_| download()
                            },
                            "csv"
                        }
                    }
                    if shows(Control::Tally) {
                        span { class: "pk-dt-tally", aria_live: "polite", {tally(&view, selection.len())} }
                    }
                }
            }

            div { id: "{scroll_id}", class: "pk-dt-scroll", "data-dt-scroll": "true", tabindex: "-1", onscroll: on_scroll,
                table {
                    id: "{id}-grid",
                    class: "pk-dt-grid",
                    role: "grid",
                    aria_label: label.clone(),
                    aria_rowcount: "{header_rows + view.entries.len()}",
                    aria_colcount: "{colspan}",
                    aria_multiselectable: "{multi}",
                    colgroup {
                        if tagged {
                            col { class: "pk-dt-col-tags" }
                        }
                        if multi {
                            col { class: "pk-dt-col-select" }
                        }
                        for column in visible.iter() {
                            {
                                let width = match (resizing.read().as_ref(), current.widths.get(&column.id)) {
                                    (Some((key, _, _, live)), _) if *key == column.id => Some(format!("width: {}px", live.round())),
                                    (_, Some(px)) => Some(format!("width: {px}px")),
                                    (_, None) => column.width.as_ref().map(|w| format!("width: {w}")),
                                };
                                rsx! { col { key: "{column.id}", style: width } }
                            }
                        }
                    }
                    thead {
                        tr { role: "row", aria_rowindex: "1",
                            if tagged {
                                th { role: "columnheader", scope: "col", class: "pk-dt-th pk-dt-th-tags", aria_label: "tags",
                                    span { class: "pk-dt-label", aria_hidden: "true" }
                                }
                            }
                            if multi {
                                th { role: "columnheader", scope: "col", class: "pk-dt-th pk-dt-th-select",
                                    input {
                                        r#type: "checkbox",
                                        aria_label: "select every row shown",
                                        "data-dt-check-all": "true",
                                        checked: all_checked,
                                        onclick: move |e| {
                                            e.stop_propagation();
                                            act.call(TableEvent::SelectAll);
                                        },
                                    }
                                }
                            }
                            for (index, column) in visible.iter().enumerate() {
                                {
                                    let key = column.id.clone();
                                    let sort_key = key.clone();
                                    let (drag_key, over_key, drop_key, resize_key) = (key.clone(), key.clone(), key.clone(), key.clone());
                                    let resize_th = format!("{id}-th-{key}");
                                    let aria_sort = column.sortable.then(|| match current.direction(&column.id) {
                                        Some(SortDir::Asc) => "ascending",
                                        Some(SortDir::Desc) => "descending",
                                        None => "none",
                                    });
                                    let mark = match current.direction(&column.id) {
                                        Some(SortDir::Asc) => "\u{25b2}",
                                        Some(SortDir::Desc) => "\u{25bc}",
                                        None => "",
                                    };
                                    let rank = current.sort_rank(&column.id).filter(|_| current.sort.len() > 1);
                                    let mut class = cell_class("pk-dt-th", column, index, multi);
                                    match (dragging.read().as_ref(), drop_mark.read().as_ref()) {
                                        (Some(dragged), _) if *dragged == column.id => class.push_str(" pk-dt-dragging"),
                                        (_, Some((target, before))) if *target == column.id => {
                                            class.push_str(if *before { " pk-dt-drop-before" } else { " pk-dt-drop-after" })
                                        }
                                        _ => {}
                                    }
                                    let order_now: Vec<String> = ordered.iter().map(|c| c.id.clone()).collect();
                                    rsx! {
                                        th {
                                            key: "{column.id}",
                                            id: "{resize_th}",
                                            role: "columnheader",
                                            scope: "col",
                                            class: "{class}",
                                            aria_sort: aria_sort,
                                            "data-col": "{column.id}",
                                            draggable: "true",
                                            ondragstart: move |e| {
                                                if resizing.peek().is_some() {
                                                    e.prevent_default();
                                                    return;
                                                }
                                                dragging.set(Some(drag_key.clone()));
                                            },
                                            ondragover: move |e| {
                                                e.prevent_default();
                                                let Some(source) = dragging.peek().clone() else { return };
                                                let at = |k: &str| order_now.iter().position(|c| c == k);
                                                let before = at(&source) > at(&over_key);
                                                if source != over_key {
                                                    drop_mark.set(Some((over_key.clone(), before)));
                                                }
                                            },
                                            ondrop: move |e| {
                                                e.prevent_default();
                                                let source = dragging.peek().clone();
                                                let mark = drop_mark.peek().clone();
                                                if let (Some(source), Some((target, before))) = (source, mark) {
                                                    if source != drop_key && target == drop_key {
                                                        let to = if before { MoveTarget::Before(target) } else { MoveTarget::After(target) };
                                                        act.call(TableEvent::MoveColumn(source, to));
                                                    }
                                                }
                                                dragging.set(None);
                                                drop_mark.set(None);
                                            },
                                            ondragend: move |_| {
                                                dragging.set(None);
                                                drop_mark.set(None);
                                            },
                                            if column.sortable {
                                                button {
                                                    r#type: "button",
                                                    class: "pk-dt-sort",
                                                    "data-dt-sort": "{column.id}",
                                                    aria_label: "sort by {column.title}",
                                                    title: "click to sort, shift-click to add a column",
                                                    onclick: move |e: MouseEvent| {
                                                        e.stop_propagation();
                                                        act.call(TableEvent::Sort { column: sort_key.clone(), append: e.modifiers().shift() });
                                                    },
                                                    span { class: "pk-dt-label", "{column.title}" }
                                                    span { class: "pk-dt-mark", aria_hidden: "true", "{mark}" }
                                                    if let Some(rank) = rank {
                                                        span { class: "pk-dt-rank", aria_hidden: "true", "{rank}" }
                                                    }
                                                }
                                            } else {
                                                span { class: "pk-dt-label", "{column.title}" }
                                            }
                                            span {
                                                class: "pk-dt-resize",
                                                "data-dt-resize": "{column.id}",
                                                aria_hidden: "true",
                                                onclick: move |e| e.stop_propagation(),
                                                onpointerdown: move |e: PointerEvent| {
                                                    e.stop_propagation();
                                                    crate::input::capture_pointer(&e);
                                                    let start = dom::width(&resize_th).unwrap_or(120.0);
                                                    resizing.set(Some((resize_key.clone(), e.client_coordinates().x, start, start)));
                                                },
                                                onpointermove: move |e: PointerEvent| {
                                                    let Some((key, x0, w0, _)) = resizing.peek().clone() else { return };
                                                    resizing.set(Some((key, x0, w0, w0 + e.client_coordinates().x - x0)));
                                                },
                                                onpointerup: move |e: PointerEvent| {
                                                    crate::input::release_pointer(&e);
                                                    let Some((key, x0, w0, _)) = resizing.peek().clone() else { return };
                                                    resizing.set(None);
                                                    act.call(TableEvent::Resize(key, w0 + e.client_coordinates().x - x0));
                                                },
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        if header_rows == 2 {
                            tr { id: "{id}-filters", class: "pk-dt-filters", role: "row", aria_rowindex: "2",
                                if tagged {
                                    th { class: "pk-dt-th pk-dt-th-tags" }
                                }
                                if multi {
                                    th { class: "pk-dt-th pk-dt-th-select" }
                                }
                                for (index, column) in visible.iter().enumerate() {
                                    th { key: "{column.id}", class: cell_class("pk-dt-th", column, index, multi), "data-col": "{column.id}",
                                        if column.filterable {
                                            {filter_control(&id, column, &current, view.options.get(&column.id).cloned().unwrap_or_default(), act)}
                                        }
                                    }
                                }
                            }
                        }
                    }
                    tbody { id: "{id}-rows",
                        if mounted.above > 0 {
                            tr { class: "pk-dt-spacer", aria_hidden: "true",
                                td { colspan: "{colspan}", style: "height: {mounted.above as f64 * row_height}px" }
                            }
                        }
                        for index in mounted.start..mounted.end {
                            match &view.entries[index] {
                                Entry::Group { value, count, folded } => {
                                    let name = value.clone();
                                    let shown = if value.is_empty() { "\u{2014}".to_string() } else { value.clone() };
                                    rsx! {
                                        tr {
                                            key: "group-{value}",
                                            id: "{id}-group-{index}",
                                            class: "pk-dt-group-row",
                                            role: "row",
                                            aria_rowindex: "{header_rows + index + 1}",
                                            aria_expanded: "{!folded}",
                                            "data-group": "{value}",
                                            "data-index": "{index}",
                                            td { colspan: "{colspan}", role: "gridcell",
                                                button {
                                                    r#type: "button",
                                                    class: "pk-dt-fold",
                                                    aria_expanded: "{!folded}",
                                                    onclick: move |_| act.call(TableEvent::ToggleGroup(name.clone())),
                                                    span { class: "pk-dt-caret", aria_hidden: "true", if *folded { "\u{25b8}" } else { "\u{25be}" } }
                                                    span { class: "pk-dt-group-value", "{shown}" }
                                                    span { class: "pk-dt-group-count", "{count}" }
                                                }
                                            }
                                        }
                                    }
                                }
                                Entry::Row(at) => {
                                    let row = &rows[*at];
                                    let row_id = row.key.clone();
                                    let cursor = current.cursor.as_deref() == Some(row_id.as_str());
                                    let is_selected = selection.contains(&row_id);
                                    let tabindex = if cursor || (current.cursor.is_none() && first_row == Some(index)) { "0" } else { "-1" };
                                    let (click_id, enter_id, check_id) = (row_id.clone(), row_id.clone(), row_id.clone());
                                    let labels = if tagged { row_labels(&row.tags) } else { Vec::new() };
                                    let cells: Vec<Element> = visible_at.iter().map(|&c| row.cells.get(c).cloned().unwrap_or_else(|| rsx! {})).collect();
                                    let tips: Vec<Option<String>> = visible_at.iter().map(|&c| row.text.get(c).filter(|t| !t.is_empty()).cloned()).collect();
                                    rsx! {
                                        tr {
                                            key: "{row_id}",
                                            id: "{id}-{row_id}",
                                            class: row.class.clone(),
                                            role: "row",
                                            "data-id": "{row_id}",
                                            "data-index": "{index}",
                                            "data-cursor": cursor.then_some("true"),
                                            aria_rowindex: "{header_rows + index + 1}",
                                            aria_selected: "{is_selected}",
                                            tabindex: tabindex,
                                            onclick: move |e: MouseEvent| {
                                                act.call(TableEvent::Cursor(click_id.clone()));
                                                match on_row {
                                                    Some(handler) => handler.call(click_id.clone()),
                                                    None if selectable != Selectable::None => {
                                                        let mode = if e.modifiers().shift() { SelectMode::Range } else { SelectMode::Toggle };
                                                        act.call(TableEvent::Select { id: click_id.clone(), mode });
                                                    }
                                                    None => {}
                                                }
                                            },
                                            onkeydown: move |e: KeyboardEvent| {
                                                if e.key() != Key::Enter || tag_focusing {
                                                    return;
                                                }
                                                e.prevent_default();
                                                match on_row {
                                                    Some(handler) => handler.call(enter_id.clone()),
                                                    None => act.call(TableEvent::Select { id: enter_id.clone(), mode: SelectMode::Toggle }),
                                                }
                                            },
                                            if tagged {
                                                td { role: "gridcell", class: "pk-dt-td pk-dt-td-tags",
                                                    if !labels.is_empty() {
                                                        {paint_ribbon(&row_id, &labels, &current, cursor, tags_limit, tags_mode, act)}
                                                    }
                                                }
                                            }
                                            if multi {
                                                td { role: "gridcell", class: "pk-dt-td pk-dt-td-select",
                                                    input {
                                                        r#type: "checkbox",
                                                        checked: is_selected,
                                                        "data-dt-check": "{row_id}",
                                                        aria_label: "select {row_id}",
                                                        onclick: move |e: MouseEvent| {
                                                            e.stop_propagation();
                                                            let mode = if e.modifiers().shift() { SelectMode::Range } else { SelectMode::Toggle };
                                                            act.call(TableEvent::Select { id: check_id.clone(), mode });
                                                        },
                                                    }
                                                }
                                            }
                                            if current.view == ViewMode::Rows {
                                                for (j, column) in visible.iter().enumerate() {
                                                    td { key: "{column.id}", role: "gridcell", class: cell_class("pk-dt-td", column, j, multi), title: tips[j].clone(),
                                                        {cells[j].clone()}
                                                    }
                                                }
                                            } else {
                                                td { role: "gridcell", class: "pk-dt-td pk-dt-td-card", colspan: "{visible.len()}",
                                                    {card(current.view, &visible, &cells, &labels)}
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        if mounted.below > 0 {
                            tr { class: "pk-dt-spacer", aria_hidden: "true",
                                td { colspan: "{colspan}", style: "height: {mounted.below as f64 * row_height}px" }
                            }
                        }
                    }
                    if let Some(footer) = footer {
                        tfoot {
                            tr { class: "pk-dt-footer", role: "row",
                                td { colspan: "{colspan}", role: "gridcell", {footer} }
                            }
                        }
                    }
                }
                if view.given == 0 {
                    p { class: "pk-dt-empty",
                        match empty {
                            Some(empty) => empty,
                            None => rsx! { "no rows" },
                        }
                    }
                } else if view.total == 0 {
                    p { class: "pk-dt-empty",
                        {empty_sentence(&current.tag_filter)}
                        button { r#type: "button", class: "pk-dt-btn", onclick: move |_| act.call(TableEvent::Clear), "clear" }
                    }
                }
            }
        }
    }
}

/// A header or cell's classes: alignment, stickiness, first column, and the
/// column's own.
fn cell_class(base: &str, column: &DataColumnSpec, index: usize, multi: bool) -> String {
    let mut class = format!("{base} pk-dt-align-{}", column.align.as_str());
    if column.sticky {
        class.push_str(" pk-dt-sticky");
        if multi {
            class.push_str(" pk-dt-sticky-after-select");
        }
    }
    if index == 0 {
        class.push_str(" pk-dt-first");
    }
    if let Some(extra) = &column.class {
        class.push(' ');
        class.push_str(extra);
    }
    class
}

/// One column's filter control, by the column's kind.
fn filter_control(
    id: &str,
    column: &DataColumnSpec,
    state: &TableState,
    options: Vec<String>,
    act: EventHandler<TableEvent>,
) -> Element {
    let key = column.id.clone();
    let current = state.filters.get(&column.id).cloned();
    match column.kind {
        ColumnKind::Enum => {
            let picked: Vec<String> = match current {
                Some(ColumnFilter::Values(values)) => values,
                _ => Vec::new(),
            };
            let mut offered = options;
            for value in &picked {
                if !offered.contains(value) {
                    offered.push(value.clone());
                }
            }
            let summary = if picked.is_empty() {
                "any".to_string()
            } else {
                picked.join(", ")
            };
            rsx! {
                details { class: "pk-dt-menu pk-dt-enum", "data-dt-menu": "filter-{column.id}",
                    summary { class: "pk-dt-btn", aria_label: "filter {column.title}", "{summary}" }
                    div { class: "pk-dt-menu-body",
                        for option in offered.iter() {
                            {
                                let (key, option_value, picked) = (key.clone(), option.clone(), picked.clone());
                                let on = picked.contains(option);
                                rsx! {
                                    label { class: "pk-dt-check", key: "{option}",
                                        input {
                                            r#type: "checkbox",
                                            name: "value[]",
                                            value: "{option}",
                                            checked: on,
                                            onchange: move |_| {
                                                let mut next = picked.clone();
                                                match next.iter().position(|v| *v == option_value) {
                                                    Some(at) => {
                                                        next.remove(at);
                                                    }
                                                    None => next.push(option_value.clone()),
                                                }
                                                act.call(TableEvent::Filter(key.clone(), ColumnFilter::values(next.iter().map(String::as_str))));
                                            },
                                        }
                                        " {option}"
                                    }
                                }
                            }
                        }
                        if offered.is_empty() {
                            span { class: "pk-dt-faint", "no values" }
                        }
                    }
                }
            }
        }
        ColumnKind::Number => {
            let (min, max) = match current {
                Some(ColumnFilter::Range { min, max }) => (min, max),
                _ => (None, None),
            };
            let show = |n: Option<f64>| n.map(|n| n.to_string()).unwrap_or_default();
            let (min_key, max_key) = (key.clone(), key);
            rsx! {
                div { class: "pk-dt-range",
                    input {
                        r#type: "number",
                        name: "min",
                        value: "{show(min)}",
                        placeholder: "min",
                        aria_label: "{column.title} at least",
                        step: "any",
                        oninput: move |e| act.call(TableEvent::Filter(min_key.clone(), ColumnFilter::range(e.value().trim().parse().ok(), max))),
                    }
                    input {
                        r#type: "number",
                        name: "max",
                        value: "{show(max)}",
                        placeholder: "max",
                        aria_label: "{column.title} at most",
                        step: "any",
                        oninput: move |e| act.call(TableEvent::Filter(max_key.clone(), ColumnFilter::range(min, e.value().trim().parse().ok()))),
                    }
                }
            }
        }
        ColumnKind::Text => {
            let text = match current {
                Some(ColumnFilter::Text(text)) => text,
                _ => String::new(),
            };
            rsx! {
                div { class: "pk-dt-text",
                    input {
                        id: "{id}-filter-{column.id}",
                        r#type: "search",
                        name: "value",
                        value: "{text}",
                        placeholder: "filter",
                        aria_label: "filter {column.title}",
                        autocomplete: "off",
                        oninput: move |e| act.call(TableEvent::Filter(key.clone(), ColumnFilter::text(&e.value()))),
                    }
                }
            }
        }
    }
}

/// A row's tags as a layered ribbon, or spelled out on the cursor row when
/// expanded. Segments join the table's one roving tab stop.
fn paint_ribbon(
    row: &str,
    labels: &[String],
    state: &TableState,
    cursor: bool,
    limit: usize,
    mode: RibbonMode,
    act: EventHandler<TableEvent>,
) -> Element {
    let name = ribbon_label(labels);
    if state.tags_expand && cursor {
        return rsx! {
            span { class: "pk-dt-tag-expanded", role: "group", aria_label: "{name}",
                for tag in labels.iter() {
                    span { key: "{tag}", "{tag}" }
                }
            }
        };
    }
    let shown = mode.shown(labels, limit);
    let hidden = labels.len() - shown.len();
    let row_id = row.to_string();
    let expand = state.tags_expand;
    rsx! {
        span { class: "pk-dt-ribbon", role: "group", aria_label: "{name}",
            for tag in shown.iter() {
                {
                    let on = state.tag_filter.contains(tag);
                    let focus = cursor && state.tag_focus.as_ref() == Some(&(row.to_string(), tag.clone()));
                    let label = segment_label(tag, &state.tag_filter);
                    let value = tag.clone();
                    rsx! {
                        button {
                            key: "{tag}",
                            r#type: "button",
                            class: if on { "pk-dt-tag pk-dt-tag-{tag_slot(tag)} on" } else { "pk-dt-tag pk-dt-tag-{tag_slot(tag)}" },
                            "data-dt-tag": "{tag}",
                            tabindex: if focus { "0" } else { "-1" },
                            aria_pressed: "{on}",
                            aria_label: "{label}",
                            title: "{label}",
                            onclick: move |e: MouseEvent| {
                                e.stop_propagation();
                                act.call(TableEvent::Tag { tag: value.clone(), all: e.modifiers().shift() });
                            },
                            span { class: "pk-dt-tag-letter", aria_hidden: "true", "{tag_letter(tag)}" }
                        }
                    }
                }
            }
            if hidden > 0 {
                button {
                    r#type: "button",
                    class: "pk-dt-tag pk-dt-tag-tail",
                    "data-dt-tag-more": "{hidden}",
                    aria_haspopup: "dialog",
                    aria_label: "show all {labels.len()} tags on this row",
                    title: "{labels.join(\", \")}",
                    onclick: move |e: MouseEvent| {
                        e.stop_propagation();
                        act.call(TableEvent::Cursor(row_id.clone()));
                        act.call(TableEvent::TagsExpand(!expand));
                    },
                    "+{hidden}"
                }
            }
        }
    }
}

/// A row as a card: compact cards set every cell on one line; detail cards
/// lead with the first cell and label the rest by their column.
fn card(
    mode: ViewMode,
    columns: &[DataColumnSpec],
    cells: &[Element],
    labels: &[String],
) -> Element {
    let compact = mode == ViewMode::CompactCards;
    rsx! {
        div { class: if compact { "pk-dt-card compact" } else { "pk-dt-card detail" },
            if compact {
                for (column, cell) in columns.iter().zip(cells) {
                    span { key: "{column.id}", class: "pk-dt-card-field", "data-label": "{column.title}", {cell.clone()} }
                }
            } else {
                div { class: "pk-dt-card-title", {cells.first().cloned().unwrap_or_else(|| rsx! {})} }
                div { class: "pk-dt-card-meta",
                    for (column, cell) in columns.iter().zip(cells).skip(1) {
                        span { key: "{column.id}", class: "pk-dt-card-field", "data-label": "{column.title}",
                            span { class: "pk-dt-card-field-label", "{column.title}" }
                            span { class: "pk-dt-card-field-value", {cell.clone()} }
                        }
                    }
                }
                if !labels.is_empty() {
                    div { class: "pk-dt-card-tags",
                        for tag in labels.iter() {
                            span { key: "{tag}", class: "pk-dt-tag-chip pk-dt-tag-{tag_slot(tag)}", "{tag}" }
                        }
                    }
                }
            }
        }
    }
}
