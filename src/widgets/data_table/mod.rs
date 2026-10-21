//! The one table component: a dense, keyboard-driven grid with sorting,
//! filtering, grouping, selection, column layout, a tag ribbon and a virtual
//! window, painted from [`panel_kit_core::widgets::data_table`].
//!
//! The markup follows the board's LiveView data table element for element
//! (`dt-*` there, `pk-dt-*` here), so its stylesheet and its tests port
//! unchanged in meaning.

mod dom;

use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::rc::Rc;

use dioxus::prelude::*;
use panel_kit_core::badge::{BadgeAction, BadgeClickKind, BadgeKind, BadgeSpec};
use panel_kit_core::widgets::data_table::{
    empty_sentence, ribbon_label, segment_label, tag_letter, tag_slot, tally, BuildOptions,
    ColumnFilter, ColumnKind, Entry, FilterKind, MoveTarget, OrderKey, Ordered, QueryMode,
    RibbonMode, ScrollMap, SelectMode, Selectable, SortDir, TableEvent, TableKey, TableQuery,
    TableState, TagMode, View, ViewMode, DEFAULT_PAGE, NARROW_WIDTH, OVERSCAN,
};

use crate::badge::Badge;

pub use panel_kit_core::widgets::data_table::{
    csv, tsv, write_csv, write_tsv, Align, DataColumnSpec, Density, SortSpec, TableData, TableDataBuilder, TableDefaults,
    Value,
};

/// The last row order built, and what it was built from.
type OrderCache = Rc<
    RefCell<
        Option<(
            Vec<DataColumnSpec>,
            TableData,
            OrderKey,
            BuildOptions,
            Rc<Ordered>,
        )>,
    >,
>;
/// The last view grouped from that order, and the folded groups it hid.
type ViewCache = Rc<RefCell<Option<(Rc<Ordered>, BTreeSet<String>, Rc<View>)>>>;
/// The selection and view last checked for every row selected, and the answer.
type AllCache = Rc<RefCell<Option<(Rc<BTreeSet<String>>, Rc<View>, bool)>>>;

/// How long the quick filter waits after a keystroke before re-querying.
const QUICK_DEBOUNCE_MS: i32 = 150;

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
    /// Column declarations, in the order each row holds its values.
    columns: Vec<DataColumnSpec>,
    /// The rows, by column; see [`TableData::builder`].
    data: TableData,
    /// Renders a cell from `(row, column)` indices into `data` and
    /// `columns`; the cell's text when absent. Called for mounted rows only.
    #[props(default)]
    cell: Option<Callback<(usize, usize), Element>>,
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
    // Quick-filter text typed but not yet applied, and which keystroke may
    // apply it.
    let mut typed = use_signal(|| None::<String>);
    let mut pending = use_signal(|| 0u64);

    let host_owned = state.is_some();
    let mut current = state.unwrap_or_else(|| owned.read().clone());
    current.sync(&columns);
    let options = BuildOptions {
        mode: if on_query.is_some() {
            QueryMode::Server
        } else {
            QueryMode::Client
        },
        tags: ribbon,
    };
    // Ordering depends on the data and the query; grouping on the order and
    // the folded groups. Cursor moves, selection and scrolling reuse both. A
    // narrower quick filter refines the last order; appended rows extend it.
    let order_cache: OrderCache = use_hook(|| Rc::new(RefCell::new(None)));
    let view_cache: ViewCache = use_hook(|| Rc::new(RefCell::new(None)));
    let ordering: Rc<Ordered> = {
        let key = current.order_key();
        let mut slot = order_cache.borrow_mut();
        let (hit, reused) = match slot.as_ref() {
            Some((c, d, k, o, ordered)) if *c == columns && *o == options => {
                if *d == data && *k == key {
                    (true, Some(ordered.clone()))
                } else if *d == data && key.narrows(k) {
                    let refined = ordered.refine(&columns, &data, &current, options);
                    (false, Some(Rc::new(refined)))
                } else if *k == key && data.extends(d) {
                    let extended = ordered.extend(&columns, &data, &current, options, d.len());
                    (false, Some(Rc::new(extended)))
                } else {
                    (false, None)
                }
            }
            _ => (false, None),
        };
        let ordered =
            reused.unwrap_or_else(|| Rc::new(Ordered::build(&columns, &data, &current, options)));
        if !hit {
            *slot = Some((columns.clone(), data.clone(), key, options, ordered.clone()));
        }
        ordered
    };
    let view: Rc<View> = {
        let mut slot = view_cache.borrow_mut();
        match slot.as_ref() {
            Some((o, folded, view)) if Rc::ptr_eq(o, &ordering) && *folded == current.collapsed => {
                view.clone()
            }
            _ => {
                let view = Rc::new(View::group(&ordering, &columns, &data, &current));
                *slot = Some((ordering.clone(), current.collapsed.clone(), view.clone()));
                view
            }
        }
    };
    let tagged = ribbon && data.tagged();

    let index_of = |column: &str| columns.iter().position(|c| c.id == column);
    let ordered: Vec<DataColumnSpec> = current.ordered(&columns).into_iter().cloned().collect();
    let visible: Vec<DataColumnSpec> = current.visible(&columns).into_iter().cloned().collect();
    let visible_at: Vec<usize> = visible.iter().filter_map(|c| index_of(&c.id)).collect();
    let selectable = current.selectable;
    let multi = selectable == Selectable::Multi;
    let selection: Rc<BTreeSet<String>> = match &selected {
        Some(ids) => Rc::new(ids.iter().cloned().collect()),
        None => current.selection.clone(),
    };
    // Whether every row in view is selected: one lookup per row, so it is
    // recomputed only when the selection or the view changes.
    let all_cache: AllCache = use_hook(|| Rc::new(RefCell::new(None)));
    let all_checked = multi && !view.rows.is_empty() && selection.len() >= view.rows.len() && {
        let mut slot = all_cache.borrow_mut();
        match slot.as_ref() {
            Some((s, v, all)) if Rc::ptr_eq(s, &selection) && Rc::ptr_eq(v, &view) => *all,
            _ => {
                let all = view.ids(&data).all(|i| selection.contains(i));
                *slot = Some((selection.clone(), view.clone(), all));
                all
            }
        }
    };
    let filterable = columns.iter().any(|c| c.filterable);
    let header_rows = if current.filters_open && visible.iter().any(|c| c.filterable) {
        2
    } else {
        1
    };
    let colspan = visible.len() + usize::from(multi) + usize::from(tagged);
    let mounted = current.mounted(view.entries.len(), windowed);
    let row_height = current.view.row_height(current.density);
    let total = view.entries.len();
    let scaled = windowed && ScrollMap::new(total, row_height, 0.0).scaled();
    let first_row = view.entries[mounted.start..mounted.end]
        .iter()
        .position(|e| matches!(e, Entry::Row(_)))
        .map(|at| at + mounted.start);
    // Each entry's key, so a window shift keeps the rows both windows mount.
    let entry_key = |index: usize| match &view.entries[index] {
        Entry::Group { value, .. } => format!("group-{value}"),
        Entry::Row(at) => data.id(*at).to_string(),
    };
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
        let data = data.clone();
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
            event.apply(&mut next, &columns, &view, &data);
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
    // order, else every row shown. Cells stream into one buffer.
    let export = {
        let data = data.clone();
        let visible_at = visible_at.clone();
        let view = view.clone();
        let selection = selection.clone();
        let heads: Vec<String> = visible.iter().map(|c| c.title.clone()).collect();
        move |selected_only: bool, as_csv: bool| {
            let only = selected_only && !selection.is_empty() && !all_checked;
            let rows = view
                .rows
                .iter()
                .copied()
                .filter(|&row| !only || selection.contains(data.id(row)));
            let mut text = String::new();
            if as_csv {
                write_csv(&mut text, &data, &heads, rows, &visible_at);
            } else {
                write_tsv(&mut text, &data, &heads, rows, &visible_at);
            }
            text
        }
    };
    let copy = {
        let export = export.clone();
        move || {
            let text = export(true, false);
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
            let text = export(false, true);
            match on_csv {
                Some(handler) => handler.call(text),
                None => dom::download(&name, "text/csv", &text),
            }
        }
    };

    // The cursor row is the one tab stop; when it moves while the grid has
    // focus, focus follows it. A scaled table scrolls the row into view by its
    // own map: the browser's scroll into view moves it by offset.
    let cursor_dom = current.cursor.as_ref().map(|c| format!("{id}-{c}"));
    let focus_scroll = scroll_id.clone();
    use_effect(use_reactive!(|(cursor_dom,)| {
        if let Some(dom_id) = cursor_dom {
            if *focused.peek() {
                dom::focus(&dom_id, !scaled);
                let row = dom::entry(&dom_id).zip(dom::scroll(&focus_scroll));
                if let (true, Some((entry, scroll))) = (scaled, row) {
                    let map = ScrollMap::new(total, row_height, scroll.visible);
                    if let Some(top) = map.reveal(scroll.top, entry) {
                        dom::scroll_to(&focus_scroll, top);
                    }
                }
            }
        }
    }));

    // A scaled table windows by the scroller's offset and height: report both
    // when either moved, or when the table moved the offset itself.
    let sync_scroll = {
        let scroll_id = scroll_id.clone();
        let seen = current.viewport;
        move || {
            let Some(scroll) = dom::scroll(&scroll_id) else {
                return;
            };
            if seen.seek || scroll.top != seen.top || scroll.visible != seen.visible {
                act.call(TableEvent::Scroll {
                    top: scroll.top,
                    visible: scroll.visible,
                });
            }
        }
    };

    let on_key = {
        let data = data.clone();
        let labels_of = move |id: &str| -> Vec<String> {
            data.row_of(id)
                .map(|row| data.tags(row).map(str::to_string).collect())
                .unwrap_or_default()
        };
        let snapshot = current.clone();
        let sync_scroll = sync_scroll.clone();
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
                let labels = labels_of(&row);
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
                    let first = labels_of(&row).into_iter().next();
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
            if scaled {
                sync_scroll();
            }
            let page = dom::scroll(&scroll_id)
                .map(|s| {
                    ((s.visible / row_height).floor() as usize)
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

    // Mount the rows the scroll offset needs, when the window lacks them.
    let sync_window = {
        let scroll_id = scroll_id.clone();
        let sync_scroll = sync_scroll.clone();
        let now = current.mounted(total, true);
        move || {
            if !windowed || total == 0 {
                return;
            }
            if scaled {
                sync_scroll();
                return;
            }
            let Some(scroll) = dom::scroll(&scroll_id) else {
                return;
            };
            let first = (scroll.top / row_height).floor() as usize;
            let shown = (scroll.visible / row_height).ceil() as usize + 1;
            let need_end = (first + shown).min(total);
            if first < now.start || need_end > now.end || now.end == now.start {
                act.call(TableEvent::Window {
                    start: first.saturating_sub(OVERSCAN),
                    count: shown + 2 * OVERSCAN,
                    row_height: None,
                });
            }
        }
    };
    let on_scroll = {
        let sync_window = sync_window.clone();
        move |_| sync_window()
    };

    // A view mode or density changes every row's height: keep the first row
    // in view and mount what the new height needs. Crossing into or out of
    // scaled scrolling changes what an offset means, so the window re-reads it.
    let last_height = use_hook(|| Rc::new(Cell::new(row_height)));
    let was_scaled = use_hook(|| Rc::new(Cell::new(scaled)));
    let rescale_id = scroll_id.clone();
    use_effect(use_reactive!(|(row_height, scaled)| {
        let before = last_height.replace(row_height);
        let crossed = was_scaled.replace(scaled) != scaled;
        if windowed && before != row_height {
            if let Some(scroll) = dom::scroll(&rescale_id) {
                let from = ScrollMap::new(total, before, scroll.visible);
                let to = ScrollMap::new(total, row_height, scroll.visible);
                dom::scroll_to(&rescale_id, from.rescaled(scroll.top, &to));
            }
        }
        if windowed && (before != row_height || crossed) {
            sync_window();
        }
    }));

    // A scaled table that moved its own offset (a key) has the scroller follow.
    let seek = (windowed && current.viewport.seek).then_some(current.viewport.top);
    let seek_id = scroll_id.clone();
    use_effect(use_reactive!(|(seek,)| {
        if let Some(top) = seek {
            dom::scroll_to(&seek_id, top);
        }
    }));

    // A table narrower than NARROW_WIDTH folds its `narrow` columns, except
    // the ones the user asked for by name; widening shows them again. A scaled
    // table also re-reads the scroller's height, which its offsets map by.
    let on_resize = {
        let narrow = narrow.clone();
        let columns = columns.clone();
        let now = current.clone();
        let sync_scroll = sync_scroll.clone();
        move |event: Event<ResizeData>| {
            if scaled {
                sync_scroll();
            }
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
    let filter_count = current.filters.len();
    let active = current.active_filters(&columns);
    let tag_filtering = !current.tag_filter.is_empty();

    rsx! {
        div {
            id: "{id}",
            class: "pk-widget pk-dt",
            style: "--dt-row:{row_height}px;{style}",
            "data-density": current.density.as_str(),
            "data-view": current.view.label(),
            "data-virtual": windowed.then_some("true"),
            "data-start": "{mounted.start}",
            "data-row-height": "{row_height}",
            "data-total": "{view.entries.len()}",
            "data-cursor": cursor_attr,
            "data-selectable": match selectable { Selectable::None => None, Selectable::Single => Some("single"), Selectable::Multi => Some("multi") },
            "data-tags": tagged.then_some("true"),
            "data-tags-mode": tagged.then_some(tags_mode.as_str()),
            "data-narrow-cols": narrow_attr,
            "data-tag-filter": tag_filtering.then_some("true"),
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
                                value: "{typed.read().clone().unwrap_or_else(|| current.quick.clone())}",
                                placeholder: "{placeholder}",
                                aria_label: "filter rows",
                                autocomplete: "off",
                                oninput: move |e| {
                                    let text = e.value();
                                    typed.set(Some(text.clone()));
                                    let ticket = *pending.peek() + 1;
                                    pending.set(ticket);
                                    spawn(async move {
                                        dom::sleep(QUICK_DEBOUNCE_MS).await;
                                        if *pending.peek() == ticket {
                                            typed.set(None);
                                            act.call(TableEvent::Quick(text));
                                        }
                                    });
                                },
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

            if !active.is_empty() {
                div { class: "pk-dt-active", role: "group", aria_label: "active filters",
                    for filter in active.iter() {
                        {
                            let kind = match filter.kind {
                                FilterKind::Tag => BadgeKind::Tag,
                                _ => BadgeKind::Generic,
                            };
                            let spec = BadgeSpec {
                                active: true,
                                with_x: true,
                                small: true,
                                click_kind: BadgeClickKind::Clicked,
                                ..BadgeSpec::new(filter.label.clone(), filter.value.clone(), kind)
                            };
                            let remove = filter.remove.clone();
                            rsx! {
                                span { class: "pk-dt-active-chip", key: "{filter.label}={filter.value}",
                                    if filter.kind != FilterKind::Tag {
                                        span { class: "pk-dt-active-label", "{filter.label}" }
                                    }
                                    Badge {
                                        spec,
                                        on_action: move |action| {
                                            if matches!(action, BadgeAction::Toggle { .. }) {
                                                act.call(remove.clone());
                                            }
                                        },
                                    }
                                }
                            }
                        }
                    }
                    if current.tag_filter.len() > 1 {
                        button {
                            r#type: "button",
                            class: "pk-dt-btn",
                            title: "rows need any of these tags, or all of them",
                            aria_pressed: "{current.tag_mode == TagMode::All}",
                            onclick: move |_| act.call(TableEvent::TagMode(match current.tag_mode {
                                TagMode::Any => TagMode::All,
                                TagMode::All => TagMode::Any,
                            })),
                            match current.tag_mode {
                                TagMode::Any => "any tag",
                                TagMode::All => "all tags",
                            }
                        }
                    }
                    button { r#type: "button", class: "pk-dt-btn", onclick: move |_| act.call(TableEvent::Clear), "clear all" }
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
                                            class: if current.filters.contains_key(&column.id) { "{class} pk-dt-filtered" } else { "{class}" },
                                            title: current.filters.get(&column.id).map(|f| format!("filtered: {}", f.summary())),
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
                                                    if current.filters.contains_key(&column.id) {
                                                        span { class: "pk-dt-filtered-mark", aria_hidden: "true", "\u{25cf}" }
                                                    }
                                                    span { class: "pk-dt-mark", aria_hidden: "true", "{mark}" }
                                                    if let Some(rank) = rank {
                                                        span { class: "pk-dt-rank", aria_hidden: "true", "{rank}" }
                                                    }
                                                }
                                            } else {
                                                span { class: "pk-dt-label", "{column.title}" }
                                                if current.filters.contains_key(&column.id) {
                                                    span { class: "pk-dt-filtered-mark", aria_hidden: "true", "\u{25cf}" }
                                                }
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
                        if mounted.above_px > 0.0 {
                            tr { class: "pk-dt-spacer", aria_hidden: "true",
                                td { colspan: "{colspan}", style: "height: {mounted.above_px}px" }
                            }
                        }
                        for index in mounted.start..mounted.end {
                            Fragment { key: "{entry_key(index)}",
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
                                    let row = *at;
                                    let row_id = data.id(row).to_string();
                                    let cursor = current.cursor.as_deref() == Some(row_id.as_str());
                                    let is_selected = selection.contains(&row_id);
                                    let tabindex = if cursor || (current.cursor.is_none() && first_row == Some(index)) { "0" } else { "-1" };
                                    let (click_id, enter_id, check_id) = (row_id.clone(), row_id.clone(), row_id.clone());
                                    let labels: Vec<String> = if tagged { data.tags(row).map(str::to_string).collect() } else { Vec::new() };
                                    let cells: Vec<Element> = visible_at
                                        .iter()
                                        .map(|&c| match cell {
                                            Some(render) => render.call((row, c)),
                                            None => rsx! { {data.text(row, c).into_owned()} },
                                        })
                                        .collect();
                                    let tips: Vec<Option<String>> = visible_at
                                        .iter()
                                        .map(|&c| Some(data.text(row, c).into_owned()).filter(|t| !t.is_empty()))
                                        .collect();
                                    rsx! {
                                        tr {
                                            key: "{row_id}",
                                            id: "{id}-{row_id}",
                                            class: data.class(row).map(str::to_string),
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
                        }
                        if mounted.below_px > 0.0 {
                            tr { class: "pk-dt-spacer", aria_hidden: "true",
                                td { colspan: "{colspan}", style: "height: {mounted.below_px}px" }
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
