//! The table painter's markup: the rendering, ribbon and stylesheet cases of
//! the LiveView data table's suites, against real SSR output.

use dioxus::prelude::*;
use panel_kit::widgets::{Control, DataColumnSpec, DataRow, DataTable};
use panel_kit_core::widgets::data_table::{
    Align, ColumnFilter, Selectable, SortDir, SortKey, SortSpec, TableDefaults, TableState,
    TableWindow, ViewMode, WINDOW_COUNT,
};
use rstest::rstest;

// -- fixtures -----------------------------------------------------------------

const STATUSES: [&str; 6] = [
    "open",
    "claimed",
    "in_progress",
    "review",
    "blocked",
    "done",
];

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

fn item(n: usize, title: &str, status: &str) -> DataRow {
    let id = format!("imp-{n}");
    let priority = (n % 5) as f64;
    let agent = (!n.is_multiple_of(3)).then(|| format!("agent-{}", n % 4));
    DataRow::new(id.clone())
        .cell(
            rsx! { b { class: "item-id", "{id}" } },
            SortKey::text(&id),
            id.clone(),
        )
        .text(title)
        .text(status)
        .num(Some(priority), format!("{priority}"))
        .cell(
            rsx! { {agent.clone().unwrap_or_default()} },
            agent
                .as_deref()
                .map(SortKey::text)
                .unwrap_or(SortKey::Missing),
            agent.unwrap_or_default(),
        )
        .num(Some(n as f64), format!("{n}s"))
}

fn rows(count: usize) -> Vec<DataRow> {
    (1..=count)
        .map(|n| item(n, &format!("item {n}"), STATUSES[n % STATUSES.len()]))
        .collect()
}

fn state() -> TableState {
    TableState::new(&columns(), TableDefaults::default())
}

/// What a table renders, with the knobs the cases turn.
#[derive(Clone)]
struct Grid {
    rows: Vec<DataRow>,
    columns: Vec<DataColumnSpec>,
    state: TableState,
    selected: Option<Vec<String>>,
    on_row: bool,
    windowed: bool,
    ribbon: bool,
}

impl PartialEq for Grid {
    fn eq(&self, _: &Self) -> bool {
        false
    }
}

fn grid(rows: Vec<DataRow>) -> Grid {
    Grid {
        rows,
        columns: columns(),
        state: state(),
        selected: None,
        on_row: false,
        windowed: false,
        ribbon: true,
    }
}

/// Handlers need a runtime, so the table is built inside a component.
#[component]
fn Probe(grid: Grid) -> Element {
    let on_row = grid.on_row.then(|| EventHandler::new(|_: String| {}));
    rsx! {
        DataTable {
            id: "t",
            label: "things",
            columns: grid.columns,
            rows: grid.rows,
            state: grid.state,
            selected: grid.selected,
            on_row,
            windowed: grid.windowed,
            ribbon: grid.ribbon,
            empty: rsx! { "nothing here" },
        }
    }
}

impl Grid {
    fn state(mut self, state: TableState) -> Self {
        self.state = state;
        self
    }

    fn render(self) -> Html {
        let mut dom = VirtualDom::new_with_props(Probe, ProbeProps { grid: self });
        dom.rebuild_in_place();
        Html(dioxus_ssr::render(&dom))
    }
}

/// Rendered markup with the lookups the assertions share.
struct Html(String);

impl Html {
    fn has(&self, needle: &str) -> bool {
        self.0.contains(needle)
    }

    fn count(&self, needle: &str) -> usize {
        self.0.matches(needle).count()
    }

    /// The opening tag of the first `<name …>` element carrying `attr`.
    fn tag(&self, name: &str, attr: &str) -> &str {
        let open = format!("<{name} ");
        self.0
            .match_indices(&open)
            .map(|(at, _)| &self.0[at..at + self.0[at..].find('>').unwrap() + 1])
            .find(|tag| tag.contains(attr))
            .unwrap_or_else(|| panic!("no <{name}> with {attr}: {}", self.0))
    }

    /// A row's whole markup, by row id.
    fn row(&self, id: &str) -> &str {
        let at = self
            .0
            .find(&format!("id=\"t-{id}\""))
            .unwrap_or_else(|| panic!("no row {id}"));
        let start = self.0[..at].rfind("<tr").unwrap();
        let end = at + self.0[at..].find("</tr>").unwrap();
        &self.0[start..end]
    }

    /// Values of `attr="…"` in order.
    fn values(&self, attr: &str) -> Vec<String> {
        let key = format!("{attr}=\"");
        self.0
            .match_indices(&key)
            .map(|(at, _)| {
                let rest = &self.0[at + key.len()..];
                rest[..rest.find('"').unwrap()].to_string()
            })
            .collect()
    }

    /// Row ids in render order.
    fn order(&self) -> Vec<String> {
        self.values("data-id")
    }

    /// Header column ids in order (the filter row repeats them).
    fn headers(&self) -> Vec<String> {
        let head = &self.0[self.0.find("<thead").unwrap()..];
        let first = &head[..head.find("</tr>").unwrap()];
        Html(first.to_string()).values("data-col")
    }
}

fn imp(range: std::ops::RangeInclusive<usize>) -> Vec<String> {
    range.map(|n| format!("imp-{n}")).collect()
}

// -- rendering ----------------------------------------------------------------

#[test]
fn a_grid_with_aria_semantics_one_header_per_visible_column_and_stable_row_ids() {
    let html = grid(rows(3)).render();

    assert!(html
        .tag("div", "id=\"t\"")
        .contains("class=\"pk-widget pk-dt\""));
    let table = html.tag("table", "id=\"t-grid\"");
    for attr in [
        "class=\"pk-dt-grid\"",
        "role=\"grid\"",
        "aria-label=\"things\"",
        "aria-rowcount=\"4\"",
        "aria-colcount=\"5\"",
        "aria-multiselectable=\"false\"",
    ] {
        assert!(table.contains(attr), "{attr} missing: {table}");
    }

    assert_eq!(
        html.headers(),
        ["id", "title", "status", "priority", "updated_at"]
    );
    assert!(html
        .tag("th", "data-col=\"id\"")
        .contains("aria-sort=\"none\""));
    assert!(!html
        .tag("th", "data-col=\"updated_at\"")
        .contains("aria-sort"));
    assert!(html.has("data-dt-sort=\"title\""));
    assert!(!html.has("data-dt-sort=\"updated_at\""));

    assert_eq!(html.order(), imp(1..=3));
    let row = html.row("imp-2");
    for attr in [
        "<b class=\"item-id\">imp-2</b>",
        "aria-rowindex=\"3\"",
        "role=\"row\"",
        "aria-selected=\"false\"",
        ">in_progress</td>",
    ] {
        assert!(row.contains(attr), "{attr} missing: {row}");
    }
    assert!(
        html.row("imp-1").contains("tabindex=\"0\""),
        "the first row is the one tab stop"
    );
    assert!(html.row("imp-2").contains("tabindex=\"-1\""));
    assert!(html.has(">3 rows<"));
}

#[test]
fn sticky_and_aligned_columns_carry_their_classes_and_widths_go_on_the_colgroup() {
    let html = grid(rows(1)).render();
    assert!(html.has("<col style=\"width: 12ch\""));
    assert!(html
        .tag("th", "data-col=\"id\"")
        .contains("class=\"pk-dt-th pk-dt-align-left pk-dt-sticky pk-dt-first\""));
    assert!(html.has("class=\"pk-dt-td pk-dt-align-right\""));
}

#[rstest]
#[case::no_rows_shows_the_empty_slot(0, "", "nothing here", "no rows match")]
#[case::a_filter_that_hides_every_row(2, "zzz", "no rows match the table filter", "nothing here")]
fn the_empty_slot_shows_for_no_rows_and_the_filter_line_for_hidden_ones(
    #[case] count: usize,
    #[case] quick: &str,
    #[case] shown: &str,
    #[case] absent: &str,
) {
    let mut state = state();
    state.set_quick(quick);
    let html = grid(rows(count)).state(state).render();
    assert!(html.has(shown), "{}", html.0);
    assert!(!html.has(absent), "{}", html.0);
    assert!(html.order().is_empty());
}

/// Text nodes only: dioxus-ssr 0.6 writes attribute values unescaped, while
/// the web renderer sets attributes through the DOM, so SSR cannot speak for
/// attribute escaping.
#[test]
fn cell_text_and_group_values_are_html_escaped() {
    let sneaky = item(
        1,
        "<script>alert(1)</script>",
        "<img src=x onerror=alert(2)>",
    );
    let state = TableState {
        group_by: Some("status".into()),
        ..state()
    };
    let html = grid(vec![sneaky]).state(state).render();
    assert!(
        html.has(">&lt;script&gt;alert(1)&lt;/script&gt;</td>"),
        "the cell's text node is escaped"
    );
    assert!(html.has("<span class=\"pk-dt-group-value\">&lt;img src=x onerror=alert(2)&gt;</span>"));
    assert_eq!(html.order(), ["imp-1"]);
}

#[test]
fn a_host_owned_selection_renders_on_the_rows_and_the_tally() {
    let mut grid = grid(rows(2));
    grid.selected = Some(vec!["imp-2".into()]);
    grid.on_row = true;
    let html = grid.render();
    assert!(html.row("imp-2").contains("aria-selected=\"true\""));
    assert!(html.row("imp-1").contains("aria-selected=\"false\""));
    assert!(html.has(">2 rows, 1 selected<"));
}

#[test]
fn multi_select_adds_the_checkbox_column_and_marks_the_grid_multiselectable() {
    let state = TableState {
        selectable: Selectable::Multi,
        selection: ["imp-1".to_string()].into(),
        ..state()
    };
    let html = grid(rows(2)).state(state).render();
    assert!(html.has("aria-multiselectable=\"true\""));
    assert!(html.has("aria-colcount=\"6\""));
    assert!(html.has("data-dt-check-all"));
    let check = html.tag("input", "data-dt-check=\"imp-1\"");
    assert!(
        check.contains("aria-label=\"select imp-1\"") && check.contains("checked"),
        "{check}"
    );
}

#[test]
fn the_filter_row_renders_one_control_per_filterable_column_by_kind() {
    let mut state = TableState {
        filters_open: true,
        ..state()
    };
    state.set_filter("priority", ColumnFilter::range(Some(1.0), None));
    let html = grid(rows(2)).state(state).render();

    assert!(html.has("id=\"t-filters\""));
    assert!(html.has("aria-rowcount=\"4\""));
    assert!(html.has("id=\"t-filter-title\""), "text: a search box");
    assert!(
        html.tag("input", "name=\"min\"").contains("value=\"1\""),
        "number: a range"
    );
    assert!(html.has("name=\"max\""));
    assert!(
        html.has("data-dt-menu=\"filter-status\""),
        "enum: a checklist"
    );
    assert!(html.has("value=\"claimed\""));
    assert!(
        !html.has("aria-label=\"filter since\""),
        "unfilterable columns get none"
    );
}

#[test]
fn a_multi_column_sort_marks_each_header_with_its_direction_and_rank() {
    let state = TableState {
        sort: vec![
            SortSpec::new("priority", SortDir::Asc),
            SortSpec::new("id", SortDir::Desc),
        ],
        ..state()
    };
    let html = grid(rows(10)).state(state).render();
    assert!(html
        .tag("th", "data-col=\"priority\"")
        .contains("aria-sort=\"ascending\""));
    assert!(html
        .tag("th", "data-col=\"id\"")
        .contains("aria-sort=\"descending\""));
    assert!(html.has("<span class=\"pk-dt-mark\" aria-hidden=\"true\">\u{25b2}</span>"));
    assert!(html.has("<span class=\"pk-dt-mark\" aria-hidden=\"true\">\u{25bc}</span>"));
    assert!(html.has("<span class=\"pk-dt-rank\" aria-hidden=\"true\">2</span>"));
}

#[test]
fn a_single_key_sort_draws_no_rank_and_missing_values_sort_last() {
    let columns = vec![
        DataColumnSpec::new("name", "Name").pinned(),
        DataColumnSpec::new("score", "Score").number(),
        DataColumnSpec::new("cost", "Cost").hidden(),
    ];
    let rows = vec![
        DataRow::new("a")
            .text("alpha")
            .num(Some(1.0), "1")
            .num(Some(9.0), "cost-a"),
        DataRow::new("b")
            .text("beta")
            .num(Some(3.0), "3")
            .num(Some(8.0), "cost-b"),
        DataRow::new("c")
            .text("gamma")
            .num(None, "-")
            .num(None, "cost-c"),
    ];
    let state = TableState {
        sort: vec![SortSpec::new("score", SortDir::Desc)],
        ..TableState::new(&columns, TableDefaults::default())
    };
    let mut grid = grid(rows);
    grid.columns = columns;
    let html = grid.state(state).render();
    assert_eq!(html.order(), ["b", "a", "c"]);
    assert!(
        !html.has("cost-a") && !html.has("data-col=\"cost\""),
        "a default-hidden column is not painted"
    );
    assert!(!html.has("pk-dt-rank"));
}

#[test]
fn a_folded_group_renders_its_header_and_hides_its_rows() {
    let state = TableState {
        group_by: Some("status".into()),
        collapsed: ["open".to_string()].into(),
        ..state()
    };
    let html = grid(rows(6)).state(state).render();
    let header = html.tag("tr", "data-group=\"open\"");
    assert!(
        header.contains("class=\"pk-dt-group-row\"") && header.contains("aria-expanded=\"false\""),
        "{header}"
    );
    assert!(html.has("<span class=\"pk-dt-group-count\">1</span>"));
    assert!(!html.has("id=\"t-imp-6\""));
    assert!(html.has("id=\"t-imp-3\""));
}

#[test]
fn the_cursor_row_is_the_tab_stop_and_names_itself() {
    let state = TableState {
        cursor: Some("imp-2".into()),
        ..state()
    };
    let html = grid(rows(3)).state(state).render();
    assert!(html
        .tag("div", "id=\"t\"")
        .contains("data-cursor=\"imp-2\""));
    assert!(
        html.row("imp-2").contains("data-cursor=\"true\"")
            && html.row("imp-2").contains("tabindex=\"0\"")
    );
    assert!(html.row("imp-1").contains("tabindex=\"-1\""));
}

#[rstest]
#[case::the_first_window(1_000, TableWindow { start: 0, count: 60, row_height: Some(26.0) }, imp(1..=60), None, Some("24440px"))]
#[case::a_scrolled_window(200, TableWindow { start: 120, count: 40, row_height: Some(20.5) }, imp(121..=160), Some("2460px"), Some("820px"))]
fn a_virtual_window_mounts_its_slice_between_spacers(
    #[case] count: usize,
    #[case] window: TableWindow,
    #[case] mounted: Vec<String>,
    #[case] above: Option<&str>,
    #[case] below: Option<&str>,
) {
    let mut grid = grid(rows(count));
    grid.windowed = true;
    let html = grid.state(TableState { window, ..state() }).render();
    let root = html.tag("div", "id=\"t\"");
    assert!(
        root.contains("data-virtual=\"true\"") && root.contains(&format!("data-total=\"{count}\"")),
        "{root}"
    );
    assert_eq!(html.order(), mounted);
    let spacers: Vec<String> = html
        .values("style")
        .into_iter()
        .filter(|s| s.starts_with("height:"))
        .collect();
    let want: Vec<String> = [above, below]
        .into_iter()
        .flatten()
        .map(|h| format!("height: {h}"))
        .collect();
    assert_eq!(spacers, want);
}

#[test]
fn the_user_order_and_a_dragged_width_reach_the_headers_and_the_colgroup() {
    let mut state = state();
    state.order = [
        "status",
        "id",
        "title",
        "priority",
        "assignee",
        "updated_at",
    ]
    .map(String::from)
    .to_vec();
    state.resize("title", 321.6);
    let html = grid(rows(1)).state(state).render();
    assert_eq!(
        html.headers(),
        ["status", "id", "title", "priority", "updated_at"]
    );
    let row = html.row("imp-1");
    let first = &row[row.find("role=\"gridcell\"").unwrap()..];
    assert!(
        first[first.find('>').unwrap() + 1..].starts_with("claimed</td>"),
        "cells follow the header order: {row}"
    );
    assert!(html.has("<col style=\"width: 322px\""));
}

#[test]
fn ten_thousand_rows_render_one_bounded_window() {
    let mut state = TableState {
        sort: vec![
            SortSpec::new("title", SortDir::Desc),
            SortSpec::new("priority", SortDir::Asc),
        ],
        ..state()
    };
    state.set_filter("status", ColumnFilter::values(["open", "review"]));
    state.set_quick("item");
    let mut grid = grid(rows(10_000));
    grid.windowed = true;
    let html = grid.state(state).render();
    assert_eq!(html.order().len(), WINDOW_COUNT);
    assert!(html.count("<tr") <= WINDOW_COUNT + 4);
    assert!(html.has("data-total=\"3333\""));
    assert!(html.has(">3333 of 10000 rows<"));
}

#[rstest]
#[case::a_toolbar_subset(Some(vec![Control::Quick, Control::Tally]), &["t-quick", "pk-dt-tally"], &["data-dt-menu=\"columns\"", ">csv<"])]
#[case::every_control(None, &["t-quick", "data-dt-menu=\"columns\"", "group rows by", ">csv<", ">copy<"], &[])]
fn the_toolbar_draws_the_controls_asked_for(
    #[case] toolbar: Option<Vec<Control>>,
    #[case] present: &[&str],
    #[case] absent: &[&str],
) {
    let html = Html(dioxus_ssr::render_element(rsx! {
        DataTable { id: "t", columns: columns(), rows: rows(2), toolbar }
    }));
    for needle in present {
        assert!(html.has(needle), "{needle} missing");
    }
    for needle in absent {
        assert!(!html.has(needle), "{needle} drawn");
    }
}

#[rstest]
#[case::compact_cards(ViewMode::CompactCards, "pk-dt-card compact")]
#[case::detail_cards(ViewMode::DetailCards, "pk-dt-card detail")]
fn card_views_label_each_field_by_its_column(#[case] view: ViewMode, #[case] class: &str) {
    let html = grid(rows(2)).state(TableState { view, ..state() }).render();
    assert_eq!(html.count(class), 2);
    assert!(html.has("data-label=\"status\""));
    assert!(
        html.has("<b class=\"item-id\">imp-1</b>"),
        "cards carry the cells as rendered"
    );
}

// -- the ribbon ---------------------------------------------------------------

fn tag_columns() -> Vec<DataColumnSpec> {
    vec![
        DataColumnSpec::new("id", "item"),
        DataColumnSpec::new("title", "title"),
    ]
}

fn tag_rows() -> Vec<DataRow> {
    [
        ("a", "one", &["raft"][..]),
        ("b", "two", &["ui", "raft"]),
        ("c", "three", &["ops"]),
        ("d", "four", &[]),
    ]
    .into_iter()
    .map(|(id, title, tags)| DataRow::new(id).text(id).text(title).tags(tags))
    .collect()
}

fn ribbon_grid(state: impl FnOnce(&mut TableState), ribbon: bool) -> Html {
    let mut table = TableState::new(&tag_columns(), TableDefaults::default());
    state(&mut table);
    let mut grid = grid(tag_rows());
    grid.columns = tag_columns();
    grid.ribbon = ribbon;
    grid.state(table).render()
}

#[rstest]
#[case::with_a_ribbon(true, "aria-colcount=\"3\"", 3)]
#[case::without_one(false, "aria-colcount=\"2\"", 2)]
fn the_gutter_is_a_counted_column_only_when_there_is_a_ribbon(
    #[case] ribbon: bool,
    #[case] colcount: &str,
    #[case] headers: usize,
) {
    let html = ribbon_grid(|_| {}, ribbon);
    assert!(html.has(colcount));
    assert_eq!(html.count("<th "), headers);
    assert_eq!(html.has("pk-dt-td-tags"), ribbon);
    assert_eq!(html.has("class=\"pk-dt-col-tags\""), ribbon);
    if ribbon {
        assert!(html
            .tag("th", "pk-dt-th-tags")
            .contains("aria-label=\"tags\""));
    }
}

#[test]
fn one_segment_per_label_with_letter_title_and_pressed_state() {
    let html = ribbon_grid(|_| {}, true);
    assert!(html.has("data-dt-tag=\"ops\"") && html.has("data-dt-tag=\"raft\""));
    let raft = html.tag("button", "data-dt-tag=\"raft\"");
    for attr in [
        "aria-pressed=\"false\"",
        "title=\"raft, filter\"",
        "aria-label=\"raft, filter\"",
        "class=\"pk-dt-tag pk-dt-tag-",
    ] {
        assert!(raft.contains(attr), "{attr} missing: {raft}");
    }
    assert!(html.has("<span class=\"pk-dt-tag-letter\" aria-hidden=\"true\">R</span>"));
    assert_eq!(
        html.count("class=\"pk-dt-ribbon\""),
        3,
        "the untagged row draws no ribbon"
    );
}

#[test]
fn an_active_tags_segment_is_marked_pressed() {
    let html = ribbon_grid(|s| s.toggle_tag("raft", false), true);
    let raft = html.tag("button", "data-dt-tag=\"raft\"");
    assert!(
        raft.contains("aria-pressed=\"true\"") && raft.contains("title=\"raft, filtering\""),
        "{raft}"
    );
    assert!(raft.contains(" on\""), "{raft}");
}

#[test]
fn only_the_focused_segment_is_in_the_tab_order() {
    let html = ribbon_grid(|s| s.focus_tag(Some(("a".into(), "raft".into()))), true);
    let focused: Vec<&str> = html
        .0
        .match_indices("<button ")
        .map(|(at, _)| &html.0[at..at + html.0[at..].find('>').unwrap()])
        .filter(|tag| tag.contains("pk-dt-tag") && tag.contains("tabindex=\"0\""))
        .collect();
    assert_eq!(focused.len(), 1, "{focused:?}");
}

#[test]
fn every_tag_slot_is_declared_in_the_stylesheet() {
    let css = include_str!("../assets/panel-kit.css");
    for slot in 0..8 {
        assert!(
            css.contains(&format!("--tag-{slot}:")),
            "--tag-{slot} missing"
        );
        assert!(
            css.contains(&format!("--tag-ink-{slot}:")),
            "--tag-ink-{slot} missing"
        );
        let rule = format!(".pk-dt-tag-{slot} {{");
        assert!(
            css.lines().any(|line| line.starts_with(&rule)),
            "{rule} missing"
        );
    }
}
