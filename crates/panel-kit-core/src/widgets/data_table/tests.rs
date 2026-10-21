//! The LiveView data table's suites (`data_table_test.exs`,
//! `data_table_tags_test.exs`, `tags_test.exs`), migrated. Sequences are data:
//! each step is an event and the state it must leave, run by one harness.

use std::collections::BTreeSet;

use proptest::prelude::*;
use rstest::rstest;
use serde_json::{json, Value};

use super::*;

// -- fixtures -----------------------------------------------------------------

/// A cell value as a host hands it over.
#[derive(Clone, Copy)]
enum V<'a> {
    T(&'a str),
    N(f64),
    Nil,
}

/// One row before it goes into a table.
struct Row {
    id: String,
    cells: Vec<Cell>,
    tags: Vec<String>,
}

enum Cell {
    T(String),
    N(f64),
    Nil,
}

impl Row {
    fn of(id: &str, cells: &[V<'_>]) -> Row {
        let cells = cells
            .iter()
            .map(|cell| match cell {
                V::T(text) => Cell::T(text.to_string()),
                V::N(n) => Cell::N(*n),
                V::Nil => Cell::Nil,
            })
            .collect();
        Row {
            id: id.to_string(),
            cells,
            tags: Vec::new(),
        }
    }

    fn tagged(mut self, tags: &[&str]) -> Row {
        self.tags = tags.iter().map(|t| t.to_string()).collect();
        self
    }
}

/// The rows as a table holds them.
fn data(columns: &[DataColumnSpec], rows: &[Row]) -> TableData {
    let mut builder = TableDataBuilder::with_capacity(columns, rows.len());
    for row in rows {
        let values = row.cells.iter().map(|cell| match cell {
            Cell::T(text) => super::Value::Text(text),
            Cell::N(n) => super::Value::Number(*n),
            Cell::Nil => super::Value::Missing,
        });
        builder.row(row.id.as_str(), values).tags(&row.tags);
    }
    builder.build()
}

const STATUSES: [&str; 6] = [
    "open",
    "claimed",
    "in_progress",
    "review",
    "blocked",
    "done",
];
const BASE_MS: f64 = 1_791_021_600_000.0;

fn columns() -> Vec<DataColumnSpec> {
    vec![
        DataColumnSpec::new("id", "item").sticky().width("12ch"),
        DataColumnSpec::new("title", "title"),
        DataColumnSpec::new("status", "status").enumerated(),
        DataColumnSpec::new("priority", "priority")
            .number()
            .align(Align::Right)
            .width("4ch"),
        DataColumnSpec::new("assignee", "agent")
            .enumerated()
            .hidden(),
        DataColumnSpec::new("updated_at", "since")
            .unsorted()
            .unfiltered(),
    ]
}

fn item(n: usize) -> Row {
    let agent = format!("agent-{}", n % 4);
    Row::of(
        &format!("imp-{n}"),
        &[
            V::T(&format!("imp-{n}")),
            V::T(&format!("item {n}")),
            V::T(STATUSES[n % STATUSES.len()]),
            V::N((n % 5) as f64),
            if n.is_multiple_of(3) {
                V::Nil
            } else {
                V::T(&agent)
            },
            V::N(BASE_MS + n as f64 * 1_000.0),
        ],
    )
}

fn rows(count: usize) -> Vec<Row> {
    (1..=count).map(item).collect()
}

fn state() -> TableState {
    TableState::new(&columns(), TableDefaults::default())
}

fn selectable(mode: Selectable) -> TableState {
    TableState::new(
        &columns(),
        TableDefaults {
            selectable: mode,
            ..TableDefaults::default()
        },
    )
}

/// A view and the data it indexes.
struct Shown {
    view: View,
    data: TableData,
}

impl Shown {
    fn ids(&self) -> Vec<String> {
        self.view.ids(&self.data).map(str::to_string).collect()
    }
}

impl std::ops::Deref for Shown {
    type Target = View;

    fn deref(&self) -> &View {
        &self.view
    }
}

fn view_with(
    columns: &[DataColumnSpec],
    rows: &[Row],
    state: &TableState,
    options: BuildOptions,
) -> Shown {
    let data = data(columns, rows);
    let view = View::build(columns, &data, state, options);
    Shown { view, data }
}

fn view_of(columns: &[DataColumnSpec], rows: &[Row], state: &TableState) -> Shown {
    view_with(columns, rows, state, BuildOptions::default())
}

fn view(rows: &[Row], state: &TableState) -> Shown {
    view_of(&columns(), rows, state)
}

fn ids(rows: &[Row], state: &TableState) -> Vec<String> {
    view(rows, state).ids()
}

/// `"imp-1 imp-10"` as ids.
fn names(list: &str) -> Vec<String> {
    list.split_whitespace().map(str::to_string).collect()
}

fn set(list: &str) -> BTreeSet<String> {
    names(list).into_iter().collect()
}

fn imp(range: std::ops::RangeInclusive<usize>) -> Vec<String> {
    range.map(|n| format!("imp-{n}")).collect()
}

/// Apply one event against the view the state yields now.
fn apply(state: &mut TableState, rows: &[Row], event: TableEvent) {
    let shown = view(rows, state);
    event.apply(state, &columns(), &shown.view, &shown.data);
}

/// Run `steps`; after each, `check` reads the state and must equal the step's
/// expectation.
fn script<T: PartialEq + std::fmt::Debug>(
    mut state: TableState,
    rows: &[Row],
    check: impl Fn(&TableState) -> T,
    steps: Vec<(TableEvent, T)>,
) -> TableState {
    for (step, (event, want)) in steps.into_iter().enumerate() {
        let label = format!("{event:?}");
        apply(&mut state, rows, event);
        assert_eq!(check(&state), want, "step {step}: {label}");
    }
    state
}

fn sort(column: &str, append: bool) -> TableEvent {
    TableEvent::Sort {
        column: column.into(),
        append,
    }
}

fn select(id: &str, mode: SelectMode) -> TableEvent {
    TableEvent::Select {
        id: id.into(),
        mode,
    }
}

fn key(key: TableKey) -> TableEvent {
    TableEvent::Key {
        key,
        shift: false,
        page: DEFAULT_PAGE,
    }
}

fn shift(key: TableKey) -> TableEvent {
    TableEvent::Key {
        key,
        shift: true,
        page: DEFAULT_PAGE,
    }
}

fn page(key: TableKey, page: usize) -> TableEvent {
    TableEvent::Key {
        key,
        shift: false,
        page,
    }
}

fn spec(column: &str, dir: SortDir) -> SortSpec {
    SortSpec::new(column, dir)
}

/// A JSON payload as a host reads it from storage.
fn stored(payload: Value) -> StoredTable {
    serde_json::from_value(payload).expect("a map payload reads")
}

use SortDir::{Asc, Desc};

// -- sorting ------------------------------------------------------------------

#[test]
fn a_click_cycles_a_column_asc_desc_off_and_replaces_any_other_sort() {
    script(
        state(),
        &rows(3),
        |s| s.sort.clone(),
        vec![
            (sort("title", false), vec![spec("title", Asc)]),
            (sort("title", false), vec![spec("title", Desc)]),
            (sort("priority", false), vec![spec("priority", Asc)]),
            (sort("priority", false), vec![spec("priority", Desc)]),
            (sort("priority", false), vec![]),
            (sort("nope", false), vec![]),
        ],
    );
}

#[test]
fn shift_click_appends_a_column_and_the_view_sorts_by_both_in_order() {
    let rows = rows(10);
    let state = script(
        state(),
        &rows,
        |s| s.sort.clone(),
        vec![
            (sort("priority", false), vec![spec("priority", Asc)]),
            (
                sort("id", true),
                vec![spec("priority", Asc), spec("id", Asc)],
            ),
            (
                sort("id", true),
                vec![spec("priority", Asc), spec("id", Desc)],
            ),
        ],
    );
    assert_eq!(state.sort_rank("id"), Some(2));
    assert_eq!(state.direction("id"), Some(Desc));

    let order: Vec<(usize, usize)> = view(&rows, &state)
        .ids()
        .iter()
        .map(|id| {
            let n: usize = id.trim_start_matches("imp-").parse().unwrap();
            (n % 5, n)
        })
        .collect();
    let mut want = order.clone();
    want.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)));
    assert_eq!(order, want);
}

fn mixed() -> Vec<Row> {
    let day = 86_400_000.0;
    vec![
        Row::of(
            "gn10",
            &[
                V::T("gn10"),
                V::T("b"),
                V::T(""),
                V::Nil,
                V::Nil,
                V::N(2.0 * day),
            ],
        ),
        Row::of(
            "gn9",
            &[
                V::T("gn9"),
                V::T("A"),
                V::T(""),
                V::N(2.0),
                V::Nil,
                V::N(3.0 * day),
            ],
        ),
        Row::of(
            "GN1",
            &[
                V::T("GN1"),
                V::T("c"),
                V::T(""),
                V::N(1.0),
                V::Nil,
                V::N(day),
            ],
        ),
    ]
}

#[rstest]
#[case::naturally_and_case_insensitively("id", Asc, "GN1 gn9 gn10")]
#[case::by_text("title", Asc, "gn9 gn10 GN1")]
#[case::numbers_with_blanks_last("priority", Asc, "GN1 gn9 gn10")]
#[case::blanks_last_descending_too("priority", Desc, "gn9 GN1 gn10")]
#[case::times_as_instants("updated_at", Desc, "gn9 gn10 GN1")]
fn strings_order_naturally_times_as_instants_blanks_last(
    #[case] column: &str,
    #[case] dir: SortDir,
    #[case] want: &str,
) {
    let state = TableState {
        sort: vec![spec(column, dir)],
        ..state()
    };
    assert_eq!(ids(&mixed(), &state), names(want));
}

#[test]
fn a_server_mode_view_keeps_the_hosts_order() {
    let rows = rows(3);
    let state = TableState {
        sort: vec![spec("id", Desc)],
        quick: "imp-1".into(),
        ..state()
    };
    let view = view_with(
        &columns(),
        &rows,
        &state,
        BuildOptions {
            mode: QueryMode::Server,
            tags: true,
        },
    );
    assert_eq!(view.ids(), names("imp-1 imp-2 imp-3"));
    assert_eq!(view.total, 3);
}

#[rstest]
#[case::sort(sort("id", false), true)]
#[case::quick(TableEvent::Quick("x".into()), true)]
#[case::filter(TableEvent::Filter("title".into(), None), true)]
#[case::clear(TableEvent::Clear, true)]
#[case::density(TableEvent::ToggleDensity, false)]
#[case::cursor(TableEvent::Cursor("imp-1".into()), false)]
fn a_server_mode_host_requeries_on_query_events_only(
    #[case] event: TableEvent,
    #[case] want: bool,
) {
    assert_eq!(event.requeries(), want);
}

#[test]
fn the_query_is_the_sort_the_filters_and_the_trimmed_quick_filter() {
    let mut state = state();
    state.click_header("id", false);
    state.set_quick("  needle ");
    assert_eq!(
        state.query(),
        TableQuery {
            sort: vec![spec("id", Asc)],
            filters: Default::default(),
            quick: "needle".into(),
        }
    );
}

// -- filtering ----------------------------------------------------------------

#[rstest]
#[case::every_row_with_the_word("imp-1", false, "imp-1 imp-10 imp-11 imp-12")]
#[case::every_word_case_insensitively("IMP-1 done", false, "imp-11")]
#[case::not_in_a_hidden_column("agent-1", false, "")]
#[case::in_a_column_once_shown("agent-1", true, "imp-1 imp-5")]
fn the_quick_filter_matches_every_word_against_the_visible_columns(
    #[case] quick: &str,
    #[case] show_all: bool,
    #[case] want: &str,
) {
    let mut state = state();
    state.set_quick(quick);
    if show_all {
        state.hidden.clear();
    }
    assert_eq!(ids(&rows(12), &state), names(want));
}

#[rstest]
#[case::text("title", ColumnFilter::text("ITEM 1"), "imp-1 imp-10 imp-11 imp-12")]
#[case::enumerated("status", ColumnFilter::values(["open", "done"]), "imp-5 imp-6 imp-11 imp-12")]
#[case::at_least(
    "priority",
    ColumnFilter::range(Some(3.0), None),
    "imp-3 imp-4 imp-8 imp-9"
)]
#[case::at_most("priority", ColumnFilter::range(None, Some(0.0)), "imp-5 imp-10")]
#[case::on_a_hidden_column("assignee", ColumnFilter::values(["agent-2"]), "imp-2 imp-10")]
fn text_enum_and_number_filters_each_by_the_columns_kind(
    #[case] column: &str,
    #[case] filter: Option<ColumnFilter>,
    #[case] want: &str,
) {
    let mut state = state();
    state.set_filter(column, filter);
    assert_eq!(ids(&rows(12), &state), names(want));
}

#[test]
fn the_total_counts_the_rows_a_filter_keeps() {
    let mut state = state();
    state.set_filter("status", ColumnFilter::values(["open"]));
    assert_eq!(view(&rows(12), &state).total, 2);
}

#[rstest]
#[case::text_is_trimmed(ColumnFilter::text("  one  "), Some(ColumnFilter::Text("one".into())))]
#[case::blank_text_is_none(ColumnFilter::text("  "), None)]
#[case::values_drop_blanks(
    ColumnFilter::values(["open", "", "done", "open"]),
    Some(ColumnFilter::Values(names("open done")))
)]
#[case::no_values_is_none(ColumnFilter::values([""]), None)]
#[case::one_bound(ColumnFilter::range(Some(1.0), None), Some(ColumnFilter::Range { min: Some(1.0), max: None }))]
#[case::no_bounds_is_none(ColumnFilter::range(None, None), None)]
#[case::non_finite_is_open(ColumnFilter::range(Some(f64::NAN), None), None)]
fn filter_input_normalises_and_blank_is_no_filter(
    #[case] got: Option<ColumnFilter>,
    #[case] want: Option<ColumnFilter>,
) {
    assert_eq!(got, want);
}

#[test]
fn filter_events_store_replace_and_clear_by_column() {
    let rows = rows(3);
    let filter = |column: &str, f: Option<ColumnFilter>| TableEvent::Filter(column.into(), f);
    let state = script(
        state(),
        &rows,
        |s| s.filters.keys().cloned().collect::<Vec<_>>(),
        vec![
            (filter("title", ColumnFilter::text("one")), names("title")),
            (
                filter("status", ColumnFilter::values(["open"])),
                names("status title"),
            ),
            (
                filter("priority", ColumnFilter::range(Some(1.0), None)),
                names("priority status title"),
            ),
            (filter("priority", None), names("status title")),
            (filter("title", None), names("status")),
            (filter("status", None), vec![]),
            (filter("nope", ColumnFilter::text("x")), vec![]),
        ],
    );
    let state = script(
        state,
        &rows,
        |s| (s.quick.clone(), s.filtering(), s.filters_open),
        vec![
            (
                TableEvent::Quick("needle".into()),
                ("needle".into(), true, false),
            ),
            (TableEvent::Clear, (String::new(), false, false)),
            (TableEvent::ToggleFilters, (String::new(), false, true)),
        ],
    );
    assert!(state.filters.is_empty());
}

// -- grouping -----------------------------------------------------------------

fn groups(view: &View) -> Vec<(String, usize, bool)> {
    view.entries
        .iter()
        .filter_map(|entry| match entry {
            Entry::Group {
                value,
                count,
                folded,
            } => Some((value.clone(), *count, *folded)),
            Entry::Row(_) => None,
        })
        .collect()
}

#[test]
fn group_by_folds_rows_under_headers_with_counts_and_a_folded_group_hides_its_rows() {
    let rows = rows(6);
    let mut state = state();
    apply(&mut state, &rows, TableEvent::Group(Some("status".into())));
    assert_eq!(state.group_by.as_deref(), Some("status"));

    let grouped = view(&rows, &state);
    let unfolded: Vec<(String, usize, bool)> = [
        "blocked",
        "claimed",
        "done",
        "in_progress",
        "open",
        "review",
    ]
    .iter()
    .map(|status| (status.to_string(), 1, false))
    .collect();
    assert_eq!(groups(&grouped), unfolded);
    assert_eq!(grouped.ids(), names("imp-4 imp-1 imp-5 imp-2 imp-6 imp-3"));

    apply(&mut state, &rows, TableEvent::ToggleGroup("open".into()));
    let folded = view(&rows, &state);
    assert_eq!(folded.ids(), names("imp-4 imp-1 imp-5 imp-2 imp-3"));
    assert!(groups(&folded).contains(&("open".into(), 1, true)));
    assert_eq!(folded.total, 6);

    let mut regrouped = state.clone();
    apply(
        &mut regrouped,
        &rows,
        TableEvent::Group(Some("priority".into())),
    );
    assert!(
        regrouped.collapsed.is_empty(),
        "a new group column unfolds everything"
    );
    for none in [None, Some(String::new()), Some("nope".into())] {
        let mut ungrouped = state.clone();
        apply(&mut ungrouped, &rows, TableEvent::Group(none));
        assert_eq!(ungrouped.group_by, None);
    }
}

#[test]
fn the_users_sort_orders_the_rows_inside_each_group() {
    let state = TableState {
        group_by: Some("status".into()),
        sort: vec![spec("id", Desc)],
        ..state()
    };
    assert_eq!(
        ids(&rows(12), &state),
        names("imp-10 imp-4 imp-7 imp-1 imp-11 imp-5 imp-8 imp-2 imp-12 imp-6 imp-9 imp-3")
    );
}

// -- selection ----------------------------------------------------------------

#[test]
fn multi_toggle_only_range_from_the_anchor_and_select_all_over_the_visible_rows() {
    let rows = rows(6);
    let all = set("imp-1 imp-2 imp-3 imp-4 imp-5 imp-6");
    let state = script(
        selectable(Selectable::Multi),
        &rows,
        |s| (*s.selection).clone(),
        vec![
            (select("imp-2", SelectMode::Toggle), set("imp-2")),
            (
                select("imp-5", SelectMode::Range),
                set("imp-2 imp-3 imp-4 imp-5"),
            ),
            (
                select("imp-3", SelectMode::Toggle),
                set("imp-2 imp-4 imp-5"),
            ),
            (select("imp-1", SelectMode::Only), set("imp-1")),
            (TableEvent::SelectAll, all),
            (TableEvent::SelectAll, set("")),
            (TableEvent::Quick("imp-1".into()), set("")),
            (TableEvent::SelectAll, set("imp-1")),
        ],
    );
    assert_eq!(state.anchor.as_deref(), Some("imp-1"));
}

#[rstest]
#[case::single_holds_one_row(Selectable::Single, vec![
    (select("imp-1", SelectMode::Toggle), "imp-1"),
    (select("imp-2", SelectMode::Range), "imp-2"),
    (select("imp-2", SelectMode::Toggle), ""),
    (TableEvent::SelectAll, ""),
])]
#[case::none_selects_nothing(Selectable::None, vec![
    (select("imp-1", SelectMode::Toggle), ""),
    (TableEvent::SelectAll, ""),
])]
fn single_selects_one_row_at_a_time_and_none_selects_nothing(
    #[case] mode: Selectable,
    #[case] steps: Vec<(TableEvent, &str)>,
) {
    script(
        selectable(mode),
        &rows(3),
        |s| (*s.selection).clone(),
        steps
            .into_iter()
            .map(|(event, want)| (event, set(want)))
            .collect(),
    );
}

// -- keyboard -----------------------------------------------------------------

#[test]
fn arrows_home_end_and_paging_move_the_cursor_through_the_visible_order() {
    let rows = rows(50);
    let cursor = |id: &str| Some(id.to_string());
    let state = script(
        state(),
        &rows,
        |s| s.cursor.clone(),
        vec![
            (key(TableKey::Down), cursor("imp-1")),
            (key(TableKey::Down), cursor("imp-2")),
            (TableEvent::Cursor("imp-10".into()), cursor("imp-10")),
            (key(TableKey::Down), cursor("imp-11")),
            (key(TableKey::Up), cursor("imp-10")),
            (page(TableKey::PageDown, 15), cursor("imp-25")),
            (page(TableKey::PageUp, 100), cursor("imp-1")),
            (key(TableKey::End), cursor("imp-50")),
            (key(TableKey::Down), cursor("imp-50")),
            (key(TableKey::Home), cursor("imp-1")),
            (key(TableKey::Up), cursor("imp-1")),
        ],
    );

    let mut empty = state.clone();
    empty.set_quick("zzz");
    let before = empty.clone();
    apply(&mut empty, &rows, key(TableKey::Down));
    assert_eq!(empty, before, "a view with no rows moves nothing");
}

#[test]
fn space_toggles_the_cursor_row_shift_extends_escape_clears_ctrl_a_selects_all() {
    let rows = rows(6);
    let state = script(
        selectable(Selectable::Multi),
        &rows,
        |s| (s.cursor.clone().unwrap_or_default(), (*s.selection).clone()),
        vec![
            (
                TableEvent::Cursor("imp-2".into()),
                ("imp-2".into(), set("")),
            ),
            (key(TableKey::Space), ("imp-2".into(), set("imp-2"))),
            (shift(TableKey::Down), ("imp-3".into(), set("imp-2 imp-3"))),
            (
                shift(TableKey::Down),
                ("imp-4".into(), set("imp-2 imp-3 imp-4")),
            ),
            (
                TableEvent::Cursor("imp-3".into()),
                ("imp-3".into(), set("imp-2 imp-3 imp-4")),
            ),
            (key(TableKey::Space), ("imp-3".into(), set("imp-2 imp-4"))),
            (key(TableKey::Escape), ("imp-3".into(), set(""))),
            (
                key(TableKey::SelectAll),
                ("imp-3".into(), set("imp-1 imp-2 imp-3 imp-4 imp-5 imp-6")),
            ),
        ],
    );
    assert_eq!(
        state.anchor, None,
        "escape drops the anchor; ctrl-a sets none"
    );

    let mut plain = self::state();
    apply(&mut plain, &rows, TableEvent::Cursor("imp-2".into()));
    apply(&mut plain, &rows, key(TableKey::Space));
    assert!(
        plain.selection.is_empty(),
        "a table that is not selectable ignores Space"
    );
}

#[test]
fn in_a_virtual_window_the_cursor_pulls_the_window_along() {
    let rows = rows(1_000);
    let mut state = state();
    state.window = TableWindow {
        start: 0,
        count: 60,
        row_height: Some(26.0),
    };
    let state = script(
        state,
        &rows,
        |s| (s.cursor.clone().unwrap_or_default(), s.window.start),
        vec![
            (TableEvent::Cursor("imp-1".into()), ("imp-1".into(), 0)),
            (page(TableKey::PageDown, 200), ("imp-201".into(), 170)),
            (key(TableKey::Down), ("imp-202".into(), 170)),
            (key(TableKey::Home), ("imp-1".into(), 0)),
        ],
    );
    let mounted = state.mounted(1_000, true);
    assert_eq!((mounted.start, mounted.end, mounted.above), (0, 60, 0));
    assert_eq!(mounted.below as f64 * state.row_height(), 24_440.0);
}

#[test]
fn the_clients_scroll_position_sets_the_window_and_nonsense_keeps_what_was_there() {
    let mut state = state();
    state.set_window(120, 40, Some(20.5));
    let window = TableWindow {
        start: 120,
        count: 40,
        row_height: Some(20.5),
    };
    assert_eq!(state.window, window);

    let mut clamped = state.clone();
    clamped.set_window(120, 0, Some(0.0));
    assert_eq!(clamped.window, TableWindow { count: 1, ..window });

    let rows = rows(200);
    let view = view(&rows, &state);
    let mounted = state.mounted(view.entries.len(), true);
    let shown: Vec<String> = view.ids()[mounted.start..mounted.end].to_vec();
    assert_eq!(shown, imp(121..=160));
    assert_eq!(mounted.above as f64 * state.row_height(), 2_460.0);
    assert_eq!(mounted.below as f64 * state.row_height(), 820.0);
}

#[rstest]
#[case::start_past_the_end(500, 40, 100, (60, 100, 60, 0))]
#[case::window_wider_than_the_rows(0, 80, 10, (0, 10, 0, 0))]
#[case::nothing_to_mount(10, 40, 0, (0, 0, 0, 0))]
fn the_window_clamps_into_the_entries(
    #[case] start: usize,
    #[case] count: usize,
    #[case] total: usize,
    #[case] want: (usize, usize, usize, usize),
) {
    let mut state = state();
    state.set_window(start, count, None);
    let m = state.mounted(total, true);
    assert_eq!((m.start, m.end, m.above, m.below), want);
    let all = state.mounted(total, false);
    assert_eq!((all.start, all.end, all.above, all.below), (0, total, 0, 0));
}

/// Fewest rows of 64-pixel detail cards that stand taller than a scroller lays out.
const SCALED_ROWS: usize = 240_000;

#[test]
fn a_table_taller_than_the_scroll_cap_mounts_what_the_viewport_shows() {
    let rows = rows(SCALED_ROWS);
    let mut state = TableState {
        view: ViewMode::DetailCards,
        ..state()
    };
    let shown = view(&rows, &state);
    let total = shown.entries.len();
    let map = ScrollMap::new(total, 64.0, 600.0);
    assert!(map.scaled());

    // A window the client reported before does not matter: the viewport decides.
    state.set_window(100_000, 80, None);
    let reach = map.height() - 600.0;
    for top in [0.0, 1_234_567.0, reach / 2.0, reach - 1.0, reach] {
        let scroll = TableEvent::Scroll { top, visible: 600.0 };
        scroll.apply(&mut state, &columns(), &shown.view, &shown.data);
        let m = state.mounted(total, true);
        assert_eq!((m.start, m.end), map.window(top), "at {top}");
        let rows_px = (m.end - m.start) as f64 * 64.0;
        assert_eq!(m.above_px + rows_px + m.below_px, MAX_SCROLL, "at {top}");
        assert_eq!((m.above, m.below), (m.start, total - m.end));
    }
    assert_eq!(state.mounted(total, true).end, total);
    assert_eq!(state.mounted(total, true).below_px, 0.0);
}

#[test]
fn a_table_that_fits_keeps_spacers_of_whole_rows_whatever_the_viewport() {
    let mut state = state();
    state.set_window(120, 40, None);
    for top in [0.0, 2_000.0, 5_000.0] {
        state.set_viewport(top, 600.0);
        let m = state.mounted(200, true);
        assert_eq!((m.above_px, m.below_px), (3_120.0, 1_040.0));
    }
}

#[test]
fn the_clients_viewport_keeps_what_is_a_length_and_zeroes_the_rest() {
    let mut state = state();
    state.set_viewport(1_500.0, 640.0);
    assert_eq!(
        state.viewport,
        Viewport { top: 1_500.0, visible: 640.0, seek: false }
    );
    state.set_viewport(f64::NAN, -3.0);
    assert_eq!(state.viewport, Viewport::default());
}

#[test]
fn keys_in_a_scaled_table_scroll_the_viewport_to_the_cursor_row() {
    let rows = rows(SCALED_ROWS);
    let mut state = TableState {
        view: ViewMode::DetailCards,
        ..state()
    };
    let shown = view(&rows, &state);
    let total = shown.entries.len();
    let reach = MAX_SCROLL - 600.0;
    let go = |state: &mut TableState, event: TableEvent| {
        event.apply(state, &columns(), &shown.view, &shown.data)
    };
    go(&mut state, TableEvent::Scroll { top: 0.0, visible: 600.0 });

    go(&mut state, key(TableKey::Down));
    assert_eq!(state.cursor.as_deref(), Some("imp-1"));
    assert!(!state.viewport.seek, "the first row is already in view");

    go(&mut state, key(TableKey::End));
    assert_eq!(state.cursor.as_deref(), Some("imp-240000"));
    assert_eq!((state.viewport.top, state.viewport.seek), (reach, true));
    let m = state.mounted(total, true);
    assert_eq!((m.end, m.below_px), (total, 0.0), "the last row is mounted");

    // The client scrolls there and reports it: the request is settled.
    go(&mut state, TableEvent::Scroll { top: reach, visible: 600.0 });
    assert!(!state.viewport.seek);

    go(&mut state, key(TableKey::Up));
    assert_eq!(state.cursor.as_deref(), Some("imp-239999"));
    assert!(!state.viewport.seek, "the row above is in view too");

    go(&mut state, key(TableKey::Home));
    assert_eq!(state.cursor.as_deref(), Some("imp-1"));
    assert_eq!((state.viewport.top, state.viewport.seek), (0.0, true));
    assert_eq!(state.mounted(total, true).start, 0);

    // A page down lands the row at the viewport's bottom edge.
    go(&mut state, TableEvent::Scroll { top: 0.0, visible: 600.0 });
    go(&mut state, page(TableKey::PageDown, 40));
    assert_eq!(state.cursor.as_deref(), Some("imp-41"));
    let map = ScrollMap::new(total, 64.0, 600.0);
    let shown_at = map.offset(state.viewport.top);
    let bottom = 41.0 * 64.0;
    assert!(state.viewport.seek && shown_at + 600.0 >= bottom && shown_at + 600.0 < bottom + 8.0);
}

// -- column layout ------------------------------------------------------------

#[test]
fn show_hide_keeps_at_least_one_column() {
    let rows = rows(1);
    let toggle = |id: &str| TableEvent::ToggleColumn(id.into());
    let fresh = state();
    assert_eq!(
        fresh.order,
        names("id title status priority assignee updated_at")
    );
    assert_eq!(fresh.hidden, set("assignee"));
    script(
        fresh,
        &rows,
        |s| s.hidden.clone(),
        vec![
            (toggle("assignee"), set("")),
            (toggle("id"), set("id")),
            (toggle("title"), set("id title")),
            (toggle("status"), set("id status title")),
            (toggle("priority"), set("id priority status title")),
            (toggle("assignee"), set("assignee id priority status title")),
            (
                toggle("updated_at"),
                set("assignee id priority status title"),
            ),
            (toggle("nope"), set("assignee id priority status title")),
        ],
    );
}

#[test]
fn a_pinned_column_never_hides() {
    let columns = vec![
        DataColumnSpec::new("a", "a").pinned(),
        DataColumnSpec::new("b", "b"),
    ];
    let mut state = TableState::new(&columns, TableDefaults::default());
    state.toggle_column(&columns, "a");
    assert!(state.hidden.is_empty());
}

#[test]
fn order_moves_by_step_or_by_drop_target() {
    let order = |s: &TableState| s.order.join(" ");
    let mv = |id: &str, target: MoveTarget| TableEvent::MoveColumn(id.into(), target);
    script(
        state(),
        &rows(1),
        order,
        vec![
            (
                mv("status", MoveTarget::Left),
                "id status title priority assignee updated_at".into(),
            ),
            (
                mv("id", MoveTarget::Left),
                "id status title priority assignee updated_at".into(),
            ),
            (
                mv("id", MoveTarget::Right),
                "status id title priority assignee updated_at".into(),
            ),
            (
                mv("updated_at", MoveTarget::Before("status".into())),
                "updated_at status id title priority assignee".into(),
            ),
            (
                mv("updated_at", MoveTarget::After("assignee".into())),
                "status id title priority assignee updated_at".into(),
            ),
            (
                mv("zzz", MoveTarget::Left),
                "status id title priority assignee updated_at".into(),
            ),
        ],
    );
}

#[rstest]
#[case::rounds(321.6, Some(322))]
#[case::clamps_narrow(2.0, Some(MIN_WIDTH))]
#[case::clamps_wide(9_999_999.0, Some(MAX_WIDTH))]
#[case::ignores_nonsense(f64::NAN, None)]
fn widths_round_and_clamp(#[case] width: f64, #[case] want: Option<u32>) {
    let mut state = state();
    state.resize("title", width);
    state.resize("gone", 100.0);
    assert_eq!(state.widths.get("title").copied(), want);
    assert!(!state.widths.contains_key("gone"));
}

#[test]
fn density_toggles_and_reset_returns_to_the_declared_layout() {
    let rows = rows(1);
    let mut state = state();
    apply(&mut state, &rows, TableEvent::ToggleDensity);
    assert_eq!(state.density, Density::Compact);
    apply(&mut state, &rows, TableEvent::ToggleDensity);
    assert_eq!(state.density, Density::Normal);

    for event in [
        sort("id", false),
        TableEvent::Filter("title".into(), ColumnFilter::text("x")),
        TableEvent::MoveColumn("id".into(), MoveTarget::Right),
        TableEvent::ToggleColumn("assignee".into()),
        TableEvent::Resize("title".into(), 200.0),
        TableEvent::ToggleDensity,
        select("imp-1", SelectMode::Toggle),
    ] {
        apply(&mut state, &rows, event);
    }
    apply(&mut state, &rows, TableEvent::Reset);
    let fresh = self::state();
    assert_eq!(
        (
            &state.order,
            &state.hidden,
            &state.widths,
            &state.sort,
            &state.filters,
            state.density
        ),
        (
            &fresh.order,
            &fresh.hidden,
            &fresh.widths,
            &fresh.sort,
            &fresh.filters,
            fresh.density
        )
    );
}

#[test]
fn sync_keeps_the_state_in_step_with_columns_that_come_and_go() {
    let mut state = TableState {
        sort: vec![spec("title", Asc)],
        ..state()
    };
    state.set_filter("status", ColumnFilter::values(["open"]));
    state.resize("title", 100.0);
    let mut columns: Vec<DataColumnSpec> =
        columns().into_iter().filter(|c| c.id != "title").collect();
    columns.push(DataColumnSpec::new("cost", "cost").hidden());

    state.sync(&columns);
    assert_eq!(
        state.order,
        names("id status priority assignee updated_at cost")
    );
    assert_eq!(state.hidden, set("assignee cost"));
    assert!(state.sort.is_empty());
    assert!(state.widths.is_empty());
    assert_eq!(
        state.filters.keys().cloned().collect::<Vec<_>>(),
        names("status")
    );
}

// -- persistence --------------------------------------------------------------

#[test]
fn the_persisted_payload_is_the_column_layout_and_the_query_as_json() {
    let rows = rows(3);
    let mut state = state();
    for event in [
        sort("priority", false),
        sort("title", true),
        TableEvent::Filter("status".into(), ColumnFilter::values(["open"])),
        TableEvent::Quick("imp".into()),
        TableEvent::ToggleColumn("updated_at".into()),
        TableEvent::MoveColumn("title".into(), MoveTarget::Left),
        TableEvent::Resize("id".into(), 90.0),
        TableEvent::Group(Some("status".into())),
        TableEvent::ToggleDensity,
        select("imp-1", SelectMode::Toggle),
        TableEvent::Window {
            start: 5,
            count: 10,
            row_height: None,
        },
        TableEvent::Scroll {
            top: 40.0,
            visible: 600.0,
        },
    ] {
        apply(&mut state, &rows, event);
    }

    let payload = serde_json::to_value(state.persisted()).unwrap();
    assert_eq!(
        payload,
        json!({
            "v": 1,
            "sort": [["priority", "asc"], ["title", "asc"]],
            "filters": {"status": ["open"]},
            "quick": "imp",
            "filters_open": false,
            "order": ["title", "id", "status", "priority", "assignee", "updated_at"],
            "hidden": ["assignee", "updated_at"],
            "widths": {"id": 90},
            "group_by": "status",
            "density": "compact",
            "view": "rows",
        })
    );

    let mut restored = self::state();
    restored.restore(&stored(serde_json::from_str(&payload.to_string()).unwrap()));
    assert_eq!(restored.persisted(), state.persisted());
    assert!(restored.selection.is_empty());
    assert_eq!(restored.window.start, 0);
    assert_eq!(restored.viewport, Viewport::default());
}

#[test]
fn a_stale_or_hostile_payload_is_checked_field_by_field() {
    let payload = json!({
        "sort": [["gone", "asc"], ["title", "sideways"], ["id", "desc"], "junk", ["id", "asc"]],
        "filters": {
            "gone": "x",
            "title": "",
            "status": ["open", 3],
            "priority": {"min": "7", "max": "a"},
        },
        "quick": "q".repeat(500),
        "order": ["status", "gone", "status", 42],
        "hidden": ["id", "title", "status", "priority", "assignee", "updated_at"],
        "widths": {"title": 9_999_999, "gone": 10, "id": "x"},
        "group_by": "gone",
        "density": "huge",
        "view": "sideways",
        "filters_open": "yes",
    });
    let mut state = state();
    state.restore(&stored(payload));

    assert_eq!(state.sort, vec![spec("id", Desc)]);
    assert_eq!(
        state.filters.get("status"),
        Some(&ColumnFilter::Values(names("open")))
    );
    assert_eq!(
        state.filters.get("priority"),
        Some(&ColumnFilter::Range {
            min: Some(7.0),
            max: None
        })
    );
    assert_eq!(state.filters.len(), 2);
    assert_eq!(state.quick.chars().count(), QUICK_LIMIT);
    assert_eq!(
        state.order,
        names("status id title priority assignee updated_at")
    );
    assert_eq!(
        state.hidden,
        set("assignee"),
        "hiding every column is refused"
    );
    assert_eq!(
        state.widths.iter().collect::<Vec<_>>(),
        vec![(&"title".to_string(), &MAX_WIDTH)]
    );
    assert_eq!(state.group_by, None);
    assert_eq!(
        (state.density, state.view, state.filters_open),
        (Density::Normal, ViewMode::Rows, false)
    );
}

/// Whether such a payload fails to read or reads as all junk, it restores
/// nothing.
#[rstest]
#[case::text(json!("garbage"))]
#[case::a_list(json!([1, 2]))]
#[case::null(Value::Null)]
fn a_payload_that_is_not_a_map_changes_nothing(#[case] payload: Value) {
    let mut state = state();
    if let Ok(stored) = serde_json::from_value::<StoredTable>(payload) {
        state.restore(&stored);
    }
    assert_eq!(state, self::state());
}

#[test]
fn a_restore_event_applies_the_stored_layout() {
    let rows = rows(3);
    let mut state = state();
    apply(
        &mut state,
        &rows,
        TableEvent::Restore(stored(json!({
            "sort": [["id", "desc"]],
            "hidden": ["status"],
            "widths": {"title": 200},
            "density": "compact",
            "bogus": 1,
        }))),
    );
    assert_eq!(ids(&rows, &state), names("imp-3 imp-2 imp-1"));
    assert_eq!(state.hidden, set("status"));
    assert_eq!(state.widths.get("title"), Some(&200));
    assert_eq!(state.density, Density::Compact);
}

proptest! {
    /// Whatever a stored payload holds, the state stays well formed.
    #[test]
    fn restore_of_any_json_keeps_the_state_well_formed(payload in json_value()) {
        let mut state = state();
        if let Ok(stored) = serde_json::from_value::<StoredTable>(payload) {
            state.restore(&stored);
        }
        let keys: BTreeSet<String> = columns().into_iter().map(|c| c.id).collect();
        prop_assert_eq!(state.order.iter().cloned().collect::<BTreeSet<_>>(), keys.clone());
        prop_assert_eq!(state.order.len(), keys.len());
        prop_assert!(state.hidden.len() < keys.len());
        prop_assert!(state.hidden.iter().all(|id| keys.contains(id)));
        prop_assert!(state.sort.iter().all(|s| keys.contains(&s.column)));
        prop_assert!(state.widths.values().all(|w| (MIN_WIDTH..=MAX_WIDTH).contains(w)));
        prop_assert!(state.quick.chars().count() <= QUICK_LIMIT);
    }

    /// Natural order is a total order: antisymmetric and transitive.
    #[test]
    fn natural_order_is_a_total_order(a in "[a-cA-C0-9]{0,6}", b in "[a-cA-C0-9]{0,6}", c in "[a-cA-C0-9]{0,6}") {
        use std::cmp::Ordering::*;
        prop_assert_eq!(natural_cmp(&a, &b), natural_cmp(&b, &a).reverse());
        if natural_cmp(&a, &b) != Greater && natural_cmp(&b, &c) != Greater {
            prop_assert_ne!(natural_cmp(&a, &c), Greater);
        }
    }

    /// Digit runs compare as numbers.
    #[test]
    fn digit_runs_compare_as_numbers(x in 0u32..100_000, y in 0u32..100_000) {
        prop_assert_eq!(natural_cmp(&format!("imp-{x}"), &format!("IMP-{y}")), x.cmp(&y));
    }

    /// The allocation-free comparison agrees with the plain char-by-char one,
    /// ASCII or not.
    #[test]
    fn natural_order_matches_the_reference(a in "[a-cA-Cé日0-9 ]{0,8}", b in "[a-cA-Cé日0-9 ]{0,8}") {
        prop_assert_eq!(natural_cmp(&a, &b), reference_natural_cmp(&a, &b));
    }

    /// Sorting by a text column, ascending or descending, is a natural-order
    /// sort of its values with ties in row order, whatever the packed keys do.
    #[test]
    fn a_text_sort_is_a_natural_sort(
        values in prop::collection::vec("[a-cA-C0-9 é/:_-]{0,12}", 0..60),
        descending in any::<bool>(),
    ) {
        let columns = vec![DataColumnSpec::new("v", "v")];
        let rows: Vec<Row> = values.iter().enumerate().map(|(n, v)| Row::of(&n.to_string(), &[V::T(v)])).collect();
        let dir = if descending { Desc } else { Asc };
        let state = TableState {
            sort: vec![spec("v", dir)],
            ..TableState::new(&columns, TableDefaults::default())
        };
        let got: Vec<usize> = view_of(&columns, &rows, &state).rows.clone();
        let mut want: Vec<usize> = (0..values.len()).collect();
        want.sort_by(|&a, &b| {
            let order = natural_cmp(&values[a], &values[b]);
            (if descending { order.reverse() } else { order }).then(a.cmp(&b))
        });
        prop_assert_eq!(got, want);
    }
}

/// Natural order spelled out char by char: the oracle for [`natural_cmp`].
fn reference_natural_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let digits = |chars: &mut std::iter::Peekable<std::str::Chars<'_>>| {
        let mut run = String::new();
        while let Some(c) = chars.peek().copied().filter(char::is_ascii_digit) {
            run.push(c);
            chars.next();
        }
        run.trim_start_matches('0').to_string()
    };
    let (mut left, mut right) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (left.peek().copied(), right.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let (l, r) = (digits(&mut left), digits(&mut right));
                match l.len().cmp(&r.len()).then_with(|| l.cmp(&r)) {
                    Ordering::Equal => {}
                    order => return order,
                }
            }
            (Some(x), Some(y)) => {
                match x.to_lowercase().cmp(y.to_lowercase()) {
                    Ordering::Equal => {}
                    order => return order,
                }
                left.next();
                right.next();
            }
        }
    }
}

/// Arbitrary JSON biased toward the payload's own field names and the
/// table's column ids.
fn json_value() -> impl Strategy<Value = Value> {
    let word = prop_oneof![
        Just("id".to_string()),
        Just("title".to_string()),
        Just("status".to_string()),
        Just("gone".to_string()),
        Just("asc".to_string()),
        Just("desc".to_string()),
        Just("compact".to_string()),
        "[a-z]{0,4}",
    ];
    let leaf = prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::Bool),
        any::<i64>().prop_map(|n| json!(n)),
        any::<f64>().prop_map(|n| json!(n)),
        word.clone().prop_map(Value::String),
    ];
    let value = leaf.prop_recursive(3, 32, 6, move |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..6).prop_map(Value::Array),
            prop::collection::vec(("[a-z_]{1,8}", inner), 0..6)
                .prop_map(|pairs| Value::Object(pairs.into_iter().collect())),
        ]
    });
    let field = prop_oneof![
        Just("sort"),
        Just("filters"),
        Just("quick"),
        Just("filters_open"),
        Just("order"),
        Just("hidden"),
        Just("widths"),
        Just("group_by"),
        Just("density"),
        Just("view"),
    ];
    prop::collection::vec((field, value), 0..10).prop_map(|pairs| {
        Value::Object(pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
    })
}

// -- scale --------------------------------------------------------------------

#[test]
fn a_sorted_filtered_ten_thousand_row_view_builds_inside_the_budget_and_windows_a_bounded_slice() {
    let rows = rows(10_000);
    let mut state = TableState {
        sort: vec![spec("title", Desc), spec("priority", Asc)],
        ..state()
    };
    state.set_filter("status", ColumnFilter::values(["open", "review"]));
    state.set_quick("item");

    let started = std::time::Instant::now();
    let view = view(&rows, &state);
    let took = started.elapsed();
    assert_eq!((view.total, view.given), (3_333, 10_000));
    assert_eq!(view.ids()[0], "imp-9999");
    assert_eq!(tally(&view, 0), "3333 of 10000 rows");
    let mounted = state.mounted(view.entries.len(), true);
    assert_eq!(mounted.end - mounted.start, WINDOW_COUNT);
    assert!(
        took < std::time::Duration::from_secs(2),
        "view took {took:?}"
    );
}

// -- exports and the toolbar --------------------------------------------------

const HEAD: [&str; 4] = ["item", "title", "status", "priority"];

#[rstest]
#[case::one_row(vec![vec!["imp-2", "item 2", "in_progress", "2"]], "item\ttitle\tstatus\tpriority\nimp-2\titem 2\tin_progress\t2")]
#[case::tabs_and_newlines_flatten(vec![vec!["q", "say \"hi\", twice", "a\nb", "0"]], "item\ttitle\tstatus\tpriority\nq\tsay \"hi\", twice\ta b\t0")]
#[case::raw_text_not_markup(vec![vec!["imp-1", "<script>alert(1)</script>", "", "1"]], "item\ttitle\tstatus\tpriority\nimp-1\t<script>alert(1)</script>\t\t1")]
fn tsv_is_a_header_and_tab_separated_rows(#[case] rows: Vec<Vec<&str>>, #[case] want: &str) {
    assert_eq!(tsv(&HEAD, &rows), want);
}

#[rstest]
#[case::crlf_after_every_line(
    vec![vec!["imp-1", "item 1", "claimed", "1"], vec!["imp-2", "item 2", "in_progress", "2"], vec!["imp-3", "item 3", "review", "3"]],
    "item,title,status,priority\r\nimp-1,item 1,claimed,1\r\nimp-2,item 2,in_progress,2\r\nimp-3,item 3,review,3\r\n"
)]
#[case::quotes_what_needs_it(
    vec![vec!["q", "say \"hi\", twice", "a\nb", "0"]],
    "item,title,status,priority\r\nq,\"say \"\"hi\"\", twice\",\"a\nb\",0\r\n"
)]
fn csv_quotes_by_rfc_4180(#[case] rows: Vec<Vec<&str>>, #[case] want: &str) {
    assert_eq!(csv(&HEAD, &rows), want);
}

#[rstest]
#[case::all_rows(0, 0, "3 rows")]
#[case::with_a_selection(0, 1, "3 rows, 1 selected")]
#[case::narrowed(1, 0, "2 of 3 rows")]
fn the_tally_counts_rows_and_the_selection(
    #[case] narrow: usize,
    #[case] selected: usize,
    #[case] want: &str,
) {
    let mut state = state();
    if narrow > 0 {
        state.set_filter("priority", ColumnFilter::range(Some(2.0), None));
    }
    assert_eq!(tally(&view(&rows(3), &state), selected), want);
}

#[rstest]
#[case::the_table_filter(&[], "no rows match the table filter")]
#[case::the_tags(&["ui", "raft"], "no rows match tags raft, ui")]
fn the_empty_line_names_what_narrowed_the_view(#[case] tags: &[&str], #[case] want: &str) {
    let filter: BTreeSet<String> = tags.iter().map(|t| t.to_string()).collect();
    assert_eq!(empty_sentence(&filter), want);
}

#[rstest]
#[case::text(ColumnFilter::Text("raft".into()), "\u{201c}raft\u{201d}")]
#[case::values(ColumnFilter::Values(names("open done")), "open, done")]
#[case::at_least(ColumnFilter::Range { min: Some(3.0), max: None }, "\u{2265} 3")]
#[case::at_most(ColumnFilter::Range { min: None, max: Some(0.5) }, "\u{2264} 0.5")]
#[case::between(ColumnFilter::Range { min: Some(1.0), max: Some(5.0) }, "1\u{2013}5")]
fn a_filter_summarises_what_it_asks_for(#[case] filter: ColumnFilter, #[case] want: &str) {
    assert_eq!(filter.summary(), want);
}

#[test]
fn active_filters_list_quick_columns_then_tags_and_each_removes_itself() {
    let rows = rows(12);
    let mut state = state();
    state.set_quick("  item ");
    state.set_filter("priority", ColumnFilter::range(Some(1.0), None));
    state.set_filter("status", ColumnFilter::values(["open"]));
    state.toggle_tag("ui", false);
    state.toggle_tag("raft", false);

    let active = state.active_filters(&columns());
    let shown: Vec<(FilterKind, &str, &str)> = active
        .iter()
        .map(|f| (f.kind, f.label.as_str(), f.value.as_str()))
        .collect();
    assert_eq!(
        shown,
        vec![
            (FilterKind::Quick, "filter", "item"),
            (FilterKind::Column, "status", "open"),
            (FilterKind::Column, "priority", "\u{2265} 1"),
            (FilterKind::Tag, "tag", "raft"),
            (FilterKind::Tag, "tag", "ui"),
        ]
    );
    for filter in active {
        apply(&mut state, &rows, filter.remove);
    }
    assert!(state.active_filters(&columns()).is_empty());
    assert!(!state.filtering() && state.tag_filter.is_empty());
}

#[test]
fn enum_filters_offer_the_declared_options_or_the_values_present_in_natural_order() {
    let mut columns = columns();
    let view_present = view_of(&columns, &rows(12), &state());
    assert_eq!(
        view_present.options.get("status").unwrap(),
        &names("blocked claimed done in_progress open review")
    );
    assert_eq!(
        view_present.options.get("assignee").unwrap(),
        &names("agent-0 agent-1 agent-2 agent-3")
    );
    assert!(
        !view_present.options.contains_key("title"),
        "only enum columns offer values"
    );

    columns[2] = DataColumnSpec::new("status", "status").options(&["open", "done"]);
    let declared = view_of(&columns, &rows(12), &state());
    assert_eq!(declared.options.get("status").unwrap(), &names("open done"));
}

// -- tags: hashing and labels -------------------------------------------------

#[rstest]
#[case("raft", 0xA1B0_A78E)]
#[case("infra", 0x3095_8D1D)]
#[case("ui", 0x5C43_327B)]
#[case("ops", 0xB822_999B)]
#[case("gate", 0x1660_EB12)]
#[case("board", 0xEF92_FE2B)]
#[case("design", 0xFB31_C5CF)]
#[case("selfdev", 0x8F56_CB56)]
#[case("api", 0x386A_AA87)]
#[case("docs", 0x28EB_34D2)]
#[case("perf", 0xA9F0_2866)]
#[case("db", 0x571C_C6C3)]
#[case("cli", 0x0D90_3D89)]
#[case("release", 0x1036_AE7E)]
#[case::by_byte_not_codepoint("café", 0xA82B_5049)]
#[case::by_byte_accented("équipe", 0x5829_FB09)]
#[case::by_byte_cjk("日本語", 0x805F_5CE7)]
#[case::case_folded("CAFÉ", 0xA82B_5049)]
fn fnv_1a_32_pins_the_known_vectors_and_the_slot_is_the_hash_modulo_eight(
    #[case] label: &str,
    #[case] hash: u32,
) {
    assert_eq!(tag_hash(label), hash, "hash({label:?}) moved");
    assert_eq!(tag_slot(label), hash % 8);
}

#[rstest]
#[case("RAFT", "raft")]
#[case("CAFÉ", "café")]
fn labels_differing_only_in_case_share_a_slot(#[case] a: &str, #[case] b: &str) {
    assert_eq!(tag_slot(a), tag_slot(b));
}

#[test]
fn an_empty_or_overlong_label_still_maps_to_a_slot() {
    assert!(tag_slot("") < TAG_SLOTS);
    assert_eq!(canonical_tag(""), "");
    assert_eq!(
        canonical_tag(&"x".repeat(40)).chars().count(),
        MAX_TAG_LENGTH
    );
}

#[rstest]
#[case::canonical_deduplicated_sorted(&["UI", "raft", "ui", " ops "], &["ops", "raft", "ui"])]
#[case::independent_of_input_order(&["raft", "ops", "ui"], &["ops", "raft", "ui"])]
#[case::none(&[], &[])]
#[case::blanks_dropped(&["  ", ""], &[])]
fn labels_are_canonical_deduplicated_and_alphabetical(
    #[case] tags: &[&str],
    #[case] want: &[&str],
) {
    let tags: Vec<String> = tags.iter().map(|t| t.to_string()).collect();
    assert_eq!(row_labels(&tags), want);
}

#[rstest]
#[case("raft", "R")]
#[case("  ui ", "U")]
#[case("", "?")]
#[case("日本語", "日")]
fn the_letter_is_the_first_character_uppercased(#[case] tag: &str, #[case] want: &str) {
    assert_eq!(tag_letter(tag), want);
}

#[test]
fn the_ribbon_and_segment_names_carry_the_word_not_the_letter() {
    assert_eq!(ribbon_label(&names("raft infra")), "tags: raft, infra");
    assert_eq!(segment_label("raft", &set("")), "raft, filter");
    assert_eq!(segment_label("raft", &set("raft")), "raft, filtering");
}

#[rstest]
#[case::full(RibbonMode::Full, 4, "a b c d")]
#[case::flag_draws_the_first(RibbonMode::Flag, 4, "a")]
#[case::limit(RibbonMode::Tight, 2, "a b")]
fn a_ribbon_draws_up_to_its_limit_and_flag_draws_one(
    #[case] mode: RibbonMode,
    #[case] limit: usize,
    #[case] want: &str,
) {
    let labels = names("a b c d e");
    assert_eq!(mode.shown(&labels, limit), names(want).as_slice());
}

// -- tags: the filter ---------------------------------------------------------

fn tag_columns() -> Vec<DataColumnSpec> {
    vec![
        DataColumnSpec::new("id", "item"),
        DataColumnSpec::new("title", "title"),
    ]
}

fn tag_rows() -> Vec<Row> {
    vec![
        Row::of("a", &[V::T("a"), V::T("one")]).tagged(&["raft"]),
        Row::of("b", &[V::T("b"), V::T("two")]).tagged(&["ui", "raft"]),
        Row::of("c", &[V::T("c"), V::T("three")]).tagged(&["ops"]),
        Row::of("d", &[V::T("d"), V::T("four")]),
    ]
}

fn tag_state(filter: &str, mode: TagMode) -> TableState {
    TableState {
        tag_filter: set(filter),
        tag_mode: mode,
        ..TableState::new(&tag_columns(), TableDefaults::default())
    }
}

#[rstest]
#[case::tags_alone_narrow("raft", TagMode::Any, "a b")]
#[case::any_of("raft ui", TagMode::Any, "a b")]
#[case::all_of("raft ui", TagMode::All, "b")]
#[case::empty_keeps_every_row("", TagMode::Any, "a b c d")]
#[case::untagged_never_passes_all("raft", TagMode::All, "a b")]
fn a_tag_filter_narrows_by_any_or_all(
    #[case] filter: &str,
    #[case] mode: TagMode,
    #[case] want: &str,
) {
    assert_eq!(
        view_of(&tag_columns(), &tag_rows(), &tag_state(filter, mode)).ids(),
        names(want)
    );
}

#[rstest]
#[case::one("ui", TagMode::Any)]
#[case::both("ui infra", TagMode::All)]
fn the_predicate_canonicalises_the_rows_labels(#[case] filter: &str, #[case] mode: TagMode) {
    let shouty = vec![Row::of("x", &[V::T("x"), V::T("shouty")]).tagged(&["UI", " Infra "])];
    assert_eq!(
        view_of(&tag_columns(), &shouty, &tag_state(filter, mode)).ids(),
        names("x")
    );
}

#[test]
fn a_table_with_no_ribbon_is_not_narrowed_by_a_tag_filter() {
    let rows = tag_rows();
    let view = view_with(
        &tag_columns(),
        &rows,
        &tag_state("raft", TagMode::Any),
        BuildOptions {
            mode: QueryMode::Client,
            tags: false,
        },
    );
    assert_eq!(view.ids(), names("a b c d"));
}

#[test]
fn a_tag_toggles_shift_switches_to_all_and_clear_empties_the_filter() {
    let rows = tag_rows();
    let tag = |tag: &str, all: bool| TableEvent::Tag {
        tag: tag.into(),
        all,
    };
    let mut state = TableState::new(&tag_columns(), TableDefaults::default());
    let mut step = |event: TableEvent| {
        let view = view_of(&tag_columns(), &rows, &state);
        event.apply(&mut state, &tag_columns(), &view.view, &view.data);
        (state.tag_filter.clone(), state.tag_mode)
    };
    assert_eq!(step(tag(" RAFT ", false)), (set("raft"), TagMode::Any));
    assert_eq!(step(tag("raft", false)), (set(""), TagMode::Any));
    assert_eq!(step(tag("raft", true)), (set("raft"), TagMode::All));
    assert_eq!(
        step(tag("", false)),
        (set("raft"), TagMode::All),
        "a blank tag is no tag"
    );
    assert_eq!(step(TableEvent::Clear), (set(""), TagMode::Any));
}

#[test]
fn focusing_a_segment_makes_its_row_the_cursor_and_leaving_keeps_the_cursor() {
    let mut state = TableState::new(&tag_columns(), TableDefaults::default());
    state.focus_tag(Some(("b".into(), "UI".into())));
    assert_eq!(state.tag_focus, Some(("b".into(), "ui".into())));
    assert_eq!(state.cursor.as_deref(), Some("b"));
    state.focus_tag(None);
    assert_eq!(
        (state.tag_focus.clone(), state.cursor.as_deref()),
        (None, Some("b"))
    );
}

#[test]
fn the_persisted_payload_carries_no_tag_state_so_a_shared_link_wins() {
    let mut state = TableState::new(&tag_columns(), TableDefaults::default());
    state.toggle_tag("raft", true);
    state.focus_tag(Some(("a".into(), "raft".into())));
    state.tags_expand = true;
    let payload = serde_json::to_value(state.persisted()).unwrap();
    for field in ["tag_filter", "tag_mode", "tag_focus", "tags_expand"] {
        assert!(payload.get(field).is_none(), "{field} persisted");
    }
    assert!(payload.get("sort").is_some());

    let mut restored = TableState::new(&tag_columns(), TableDefaults::default());
    restored.restore(&stored(json!({"tag_filter": {"raft": true}, "sort": []})));
    assert!(restored.tag_filter.is_empty());
}

#[rstest]
#[case::select(select("a", SelectMode::Toggle), false)]
#[case::cursor(TableEvent::Cursor("a".into()), false)]
#[case::key(key(TableKey::Down), false)]
#[case::window(TableEvent::Window { start: 0, count: 10, row_height: None }, false)]
#[case::scroll(TableEvent::Scroll { top: 10.0, visible: 600.0 }, false)]
#[case::tag(TableEvent::Tag { tag: "raft".into(), all: false }, false)]
#[case::fold(TableEvent::ToggleGroup("open".into()), false)]
#[case::sort(sort("id", false), true)]
#[case::layout(TableEvent::MoveColumn("id".into(), MoveTarget::Right), true)]
#[case::density(TableEvent::ToggleDensity, true)]
fn only_layout_and_query_events_persist(#[case] event: TableEvent, #[case] want: bool) {
    assert_eq!(event.persists(), want);
}
