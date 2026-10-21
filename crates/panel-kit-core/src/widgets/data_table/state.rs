//! One table's whole user-facing state and the events that change it.
//!
//! Durable parts (sort, filters, layout, grouping, density) persist through
//! [`TableState::persisted`] / [`TableState::restore`]; session parts
//! (selection, cursor, window, tag filter) never do, so a shared link or a
//! fresh load decides them.

use std::collections::BTreeMap;
use std::collections::BTreeSet;

use serde::Deserialize;
use serde::Serialize;

use super::column::{DataColumnSpec, SortDir, SortSpec};
use super::data::TableData;
use super::stored::{Field, StoredFilter, StoredTable};
use super::tags::{canonical_tag, TagMode};
use super::view::View;

/// Rows the window renders before the client measures.
pub const WINDOW_COUNT: usize = 80;
/// Rows the window keeps rendered beyond each edge of the screen.
pub const OVERSCAN: usize = 20;
/// The most rows one window may mount.
pub const MAX_WINDOW: usize = 2_000;
/// Narrowest a dragged column may be, in pixels.
pub const MIN_WIDTH: u32 = 40;
/// Widest a dragged column may be, in pixels.
pub const MAX_WIDTH: u32 = 4_000;
/// Longest quick filter, in characters.
pub const QUICK_LIMIT: usize = 200;
/// Below this table width, in pixels, `narrow` columns fold away.
pub const NARROW_WIDTH: f64 = 1_000.0;
/// The persisted payload's version.
pub const STATE_VERSION: u32 = 1;
/// Rows a page key moves when the client has not measured a page.
pub const DEFAULT_PAGE: usize = 20;

/// Row padding and type scale.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Density {
    /// 26px rows.
    #[default]
    Normal,
    /// 20px rows for a long ledger.
    Compact,
}

impl Density {
    /// Pixel height of one row; the window's spacers use it until the client
    /// measures.
    pub fn row_height(self) -> f64 {
        match self {
            Density::Normal => 26.0,
            Density::Compact => 20.0,
        }
    }

    /// The other density.
    pub fn toggled(self) -> Density {
        match self {
            Density::Normal => Density::Compact,
            Density::Compact => Density::Normal,
        }
    }

    /// The density's name, as the toggle and `data-density` spell it.
    pub fn as_str(self) -> &'static str {
        match self {
            Density::Normal => "normal",
            Density::Compact => "compact",
        }
    }
}

/// How many rows a table may select.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Selectable {
    /// No row can be selected.
    #[default]
    None,
    /// One row at a time; a second click clears it.
    Single,
    /// Any number, with a range from the anchor.
    Multi,
}

/// How the table renders its rows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewMode {
    /// One grid row per record.
    #[default]
    Rows,
    /// One single-line card per record.
    CompactCards,
    /// One card per record: title, labelled fields, tags.
    DetailCards,
}

impl ViewMode {
    /// The next mode in the toggle's cycle.
    pub fn next(self) -> ViewMode {
        match self {
            ViewMode::Rows => ViewMode::CompactCards,
            ViewMode::CompactCards => ViewMode::DetailCards,
            ViewMode::DetailCards => ViewMode::Rows,
        }
    }

    /// The toggle's label.
    pub fn label(self) -> &'static str {
        match self {
            ViewMode::Rows => "rows",
            ViewMode::CompactCards => "cards",
            ViewMode::DetailCards => "detail",
        }
    }
}

/// The virtual window: which slice of the view's entries stays mounted.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct TableWindow {
    /// Index into the view's entries of the first one mounted.
    pub start: usize,
    /// How many entries stay mounted.
    pub count: usize,
    /// Measured row height, once the client has reported one.
    pub row_height: Option<f64>,
}

impl Default for TableWindow {
    fn default() -> Self {
        TableWindow {
            start: 0,
            count: WINDOW_COUNT,
            row_height: None,
        }
    }
}

/// One column's filter; the column's kind decides which.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ColumnFilter {
    /// Text the cell must contain, case-insensitively.
    Text(String),
    /// Values the cell must equal one of.
    Values(Vec<String>),
    /// Inclusive numeric bounds; `None` is open on that side.
    Range {
        /// Lower bound.
        min: Option<f64>,
        /// Upper bound.
        max: Option<f64>,
    },
}

impl ColumnFilter {
    /// A text filter from typed input: trimmed, and blank is no filter.
    pub fn text(input: &str) -> Option<ColumnFilter> {
        let text = input.trim();
        (!text.is_empty()).then(|| ColumnFilter::Text(text.to_string()))
    }

    /// An enum filter from ticked values: blanks and repeats dropped, and
    /// nothing ticked is no filter.
    pub fn values<'a>(input: impl IntoIterator<Item = &'a str>) -> Option<ColumnFilter> {
        let mut values: Vec<String> = Vec::new();
        for value in input {
            if !value.is_empty() && !values.iter().any(|v| v == value) {
                values.push(value.to_string());
            }
        }
        (!values.is_empty()).then_some(ColumnFilter::Values(values))
    }

    /// A range filter from typed bounds; both open is no filter.
    pub fn range(min: Option<f64>, max: Option<f64>) -> Option<ColumnFilter> {
        let min = min.filter(|n| n.is_finite());
        let max = max.filter(|n| n.is_finite());
        (min.is_some() || max.is_some()).then_some(ColumnFilter::Range { min, max })
    }
}

/// The defaults a table declares; `reset` returns to them.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TableDefaults {
    /// Initial sort.
    #[serde(default)]
    pub sort: Vec<SortSpec>,
    /// Initial density.
    #[serde(default)]
    pub density: Density,
    /// Initial grouping column.
    #[serde(default)]
    pub group_by: Option<String>,
    /// Selection mode.
    #[serde(default)]
    pub selectable: Selectable,
    /// Initial view mode.
    #[serde(default)]
    pub view: ViewMode,
    /// Columns hidden on top of those declared hidden.
    #[serde(default)]
    pub hidden: BTreeSet<String>,
    /// Column order; the declared order when empty.
    #[serde(default)]
    pub order: Vec<String>,
}

/// Where a moved column lands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MoveTarget {
    /// One place earlier.
    Left,
    /// One place later.
    Right,
    /// Immediately before another column (a drop on its leading half).
    Before(String),
    /// Immediately after another column (a drop on its trailing half).
    After(String),
}

/// How a click selects.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SelectMode {
    /// Flip the row and make it the anchor.
    #[default]
    Toggle,
    /// Make the row the whole selection.
    Only,
    /// Add every row from the anchor through this one.
    Range,
}

/// A key the table owns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TableKey {
    /// Arrow down.
    Down,
    /// Arrow up.
    Up,
    /// First row.
    Home,
    /// Last row.
    End,
    /// A page down.
    PageDown,
    /// A page up.
    PageUp,
    /// Space: toggle the cursor row (shift: range).
    Space,
    /// Escape: clear the selection.
    Escape,
    /// Ctrl/Cmd-A: select every visible row.
    SelectAll,
}

/// One table's whole user-facing state.
#[derive(Clone, Debug, PartialEq)]
pub struct TableState {
    /// Sort keys in priority order.
    pub sort: Vec<SortSpec>,
    /// Per-column filters, keyed by column id.
    pub filters: BTreeMap<String, ColumnFilter>,
    /// The quick filter: every word must appear in a visible column.
    pub quick: String,
    /// Whether the per-column filter row is shown.
    pub filters_open: bool,
    /// Column order.
    pub order: Vec<String>,
    /// Hidden column ids.
    pub hidden: BTreeSet<String>,
    /// Dragged column widths, in pixels.
    pub widths: BTreeMap<String, u32>,
    /// The column rows are grouped by.
    pub group_by: Option<String>,
    /// Group values the user folded.
    pub collapsed: BTreeSet<String>,
    /// Row padding.
    pub density: Density,
    /// Rows, compact cards, or detail cards.
    pub view: ViewMode,
    /// How many rows may be selected.
    pub selectable: Selectable,
    /// Selected row ids.
    pub selection: BTreeSet<String>,
    /// The row a range grows from.
    pub anchor: Option<String>,
    /// The keyboard's row: the one tab stop.
    pub cursor: Option<String>,
    /// The mounted window.
    pub window: TableWindow,
    /// The tag filter.
    pub tag_filter: BTreeSet<String>,
    /// Whether a row needs any or all filtered tags.
    pub tag_mode: TagMode,
    /// The segment in the tab order: `(row id, tag)`.
    pub tag_focus: Option<(String, String)>,
    /// Whether the cursor row spells its tags out.
    pub tags_expand: bool,
    /// What `reset` returns to.
    pub defaults: TableDefaults,
}

impl Default for TableState {
    fn default() -> Self {
        TableState::new(&[], TableDefaults::default())
    }
}

impl TableState {
    /// A fresh state for `columns` on `defaults`. Hidden columns are the
    /// declared ones plus the defaults', both limited to known columns.
    pub fn new(columns: &[DataColumnSpec], defaults: TableDefaults) -> Self {
        let keys: Vec<String> = columns.iter().map(|c| c.id.clone()).collect();
        let hidden: BTreeSet<String> = columns
            .iter()
            .filter(|c| c.hidden_by_default)
            .map(|c| c.id.clone())
            .chain(defaults.hidden.iter().cloned())
            .filter(|id| keys.contains(id))
            .collect();
        let sort = valid_sort(
            defaults
                .sort
                .iter()
                .map(|s| (s.column.as_str(), Some(s.dir))),
            &keys,
        );
        let group_by = defaults.group_by.clone().filter(|id| keys.contains(id));
        let defaults = TableDefaults {
            sort: sort.clone(),
            group_by: group_by.clone(),
            hidden: hidden.clone(),
            order: keys.clone(),
            ..defaults
        };
        TableState {
            sort,
            filters: BTreeMap::new(),
            quick: String::new(),
            filters_open: false,
            order: keys,
            hidden,
            widths: BTreeMap::new(),
            group_by,
            collapsed: BTreeSet::new(),
            density: defaults.density,
            view: defaults.view,
            selectable: defaults.selectable,
            selection: BTreeSet::new(),
            anchor: None,
            cursor: None,
            window: TableWindow::default(),
            tag_filter: BTreeSet::new(),
            tag_mode: TagMode::Any,
            tag_focus: None,
            tags_expand: false,
            defaults,
        }
    }

    fn known(&self, id: &str) -> bool {
        self.order.iter().any(|known| known == id)
    }

    /// Reconcile with the table's current columns: a new column joins the end
    /// of the order (hidden if declared so); one that left takes its sort key,
    /// filter, width and group with it.
    pub fn sync(&mut self, columns: &[DataColumnSpec]) {
        let keys: Vec<&str> = columns.iter().map(|c| c.id.as_str()).collect();
        let fresh: Vec<&DataColumnSpec> = columns.iter().filter(|c| !self.known(&c.id)).collect();
        let fresh_hidden = fresh
            .iter()
            .filter(|c| c.hidden_by_default)
            .map(|c| c.id.clone());
        let keep = |id: &String| keys.contains(&id.as_str());

        self.order.retain(keep);
        self.order.extend(fresh.iter().map(|c| c.id.clone()));
        self.hidden.extend(fresh_hidden.clone());
        self.hidden.retain(keep);
        self.sort.retain(|spec| keep(&spec.column));
        self.filters.retain(|id, _| keep(id));
        self.widths.retain(|id, _| keep(id));
        self.group_by = self.group_by.take().filter(keep);
        self.defaults.order.retain(keep);
        self.defaults
            .order
            .extend(fresh.iter().map(|c| c.id.clone()));
        self.defaults.hidden.extend(fresh_hidden);
        self.defaults.hidden.retain(keep);
    }

    /// The columns in the user's order, hidden ones included.
    pub fn ordered<'a>(&self, columns: &'a [DataColumnSpec]) -> Vec<&'a DataColumnSpec> {
        self.order
            .iter()
            .filter_map(|id| columns.iter().find(|c| &c.id == id))
            .collect()
    }

    /// The columns in the user's order, hidden ones left out.
    pub fn visible<'a>(&self, columns: &'a [DataColumnSpec]) -> Vec<&'a DataColumnSpec> {
        self.ordered(columns)
            .into_iter()
            .filter(|c| self.is_visible(c))
            .collect()
    }

    /// Whether `column` is shown. A pinned column always is.
    pub fn is_visible(&self, column: &DataColumnSpec) -> bool {
        !column.hideable || !self.hidden.contains(&column.id)
    }

    /// Show a hidden column or hide a shown one. The last visible column and
    /// a pinned column stay.
    pub fn toggle_column(&mut self, columns: &[DataColumnSpec], id: &str) {
        let Some(column) = columns.iter().find(|c| c.id == id) else {
            return;
        };
        if self.hidden.remove(id) || !column.hideable {
            return;
        }
        if self.visible(columns).len() > 1 {
            self.hidden.insert(id.to_string());
        }
    }

    /// Move a column by one step or to a drop target.
    pub fn move_column(&mut self, id: &str, target: &MoveTarget) {
        let Some(from) = self.order.iter().position(|c| c == id) else {
            return;
        };
        let mut rest = self.order.clone();
        let moved = rest.remove(from);
        let index_in = |list: &[String], other: &str| list.iter().position(|c| c == other);
        let at = match target {
            MoveTarget::Left => from.saturating_sub(1),
            MoveTarget::Right => (from + 1).min(rest.len()),
            MoveTarget::Before(other) => index_in(&rest, other).unwrap_or(rest.len()),
            MoveTarget::After(other) => index_in(&rest, other)
                .map(|at| at + 1)
                .unwrap_or(rest.len()),
        };
        rest.insert(at, moved);
        self.order = rest;
    }

    /// Record a dragged width: rounded, clamped; a non-number changes nothing.
    pub fn resize(&mut self, id: &str, width: f64) {
        if self.known(id) && width.is_finite() {
            self.widths.insert(id.to_string(), clamp_width(width));
        }
    }

    /// Sort by a column. A click cycles it ascending, descending, off and
    /// replaces the sort; `append` (shift) cycles it in its own place, so a
    /// second column refines the first.
    pub fn click_header(&mut self, id: &str, append: bool) {
        if !self.known(id) {
            return;
        }
        let at = self.sort.iter().position(|spec| spec.column == id);
        let next = match at.map(|at| self.sort[at].dir) {
            None => Some(SortDir::Asc),
            Some(SortDir::Asc) => Some(SortDir::Desc),
            Some(SortDir::Desc) => None,
        };
        match (append, next, at) {
            (false, Some(dir), _) => self.sort = vec![SortSpec::new(id, dir)],
            (false, None, _) => self.sort.clear(),
            (true, Some(dir), Some(at)) => self.sort[at].dir = dir,
            (true, Some(dir), None) => self.sort.push(SortSpec::new(id, dir)),
            (true, None, Some(at)) => {
                self.sort.remove(at);
            }
            (true, None, None) => {}
        }
    }

    /// A column's sort direction.
    pub fn direction(&self, column: &str) -> Option<SortDir> {
        self.sort.iter().find(|s| s.column == column).map(|s| s.dir)
    }

    /// A column's place in a multi-column sort, 1-based.
    pub fn sort_rank(&self, column: &str) -> Option<usize> {
        self.sort
            .iter()
            .position(|s| s.column == column)
            .map(|at| at + 1)
    }

    /// Set or clear one column's filter.
    pub fn set_filter(&mut self, column: &str, filter: Option<ColumnFilter>) {
        if !self.known(column) {
            return;
        }
        match filter {
            Some(filter) => {
                self.filters.insert(column.to_string(), filter);
            }
            None => {
                self.filters.remove(column);
            }
        }
    }

    /// Set the quick filter, clamped to [`QUICK_LIMIT`] characters.
    pub fn set_quick(&mut self, quick: &str) {
        self.quick = quick.chars().take(QUICK_LIMIT).collect();
    }

    /// Clear every filter: columns, quick, and the tag filter with its mode.
    pub fn clear_filters(&mut self) {
        self.filters.clear();
        self.quick.clear();
        self.clear_tags();
    }

    /// Whether a column or quick filter narrows the rows.
    pub fn filtering(&self) -> bool {
        !self.filters.is_empty() || !self.quick.trim().is_empty()
    }

    /// Group by a known column, or ungroup; every group unfolds.
    pub fn set_group(&mut self, column: Option<&str>) {
        self.group_by = column.filter(|id| self.known(id)).map(str::to_string);
        self.collapsed.clear();
    }

    /// Fold or unfold one group.
    pub fn toggle_group(&mut self, group: &str) {
        if !self.collapsed.remove(group) {
            self.collapsed.insert(group.to_string());
        }
    }

    /// Select a row. `none` selects nothing; `single` holds one row and a
    /// second click clears it; `multi` toggles, takes only, or adds the span
    /// of rendered rows from the anchor through `id` (an anchor out of view
    /// spans just `id`).
    pub fn select(&mut self, id: &str, mode: SelectMode, view: &View, data: &TableData) {
        match self.selectable {
            Selectable::None => {}
            Selectable::Single => {
                if self.selection.contains(id) {
                    self.selection.clear();
                    self.anchor = None;
                } else {
                    self.selection = BTreeSet::from([id.to_string()]);
                    self.anchor = Some(id.to_string());
                }
            }
            Selectable::Multi => match (mode, self.anchor.clone()) {
                (SelectMode::Only, _) => {
                    self.selection = BTreeSet::from([id.to_string()]);
                    self.anchor = Some(id.to_string());
                }
                (SelectMode::Range, Some(anchor)) => {
                    match (view.position(data, &anchor), view.position(data, id)) {
                        (Some(a), Some(b)) => {
                            let (lo, hi) = (a.min(b), a.max(b));
                            let span = view.rows[lo..=hi]
                                .iter()
                                .map(|&row| data.id(row).to_string());
                            self.selection.extend(span);
                        }
                        _ => {
                            self.selection.insert(id.to_string());
                        }
                    }
                }
                _ => {
                    if !self.selection.remove(id) {
                        self.selection.insert(id.to_string());
                    }
                    self.anchor = Some(id.to_string());
                }
            },
        }
    }

    /// Select every rendered row, or deselect them when all already are.
    pub fn select_all(&mut self, view: &View, data: &TableData) {
        if self.selectable != Selectable::Multi {
            return;
        }
        if view.ids(data).all(|id| self.selection.contains(id)) {
            for id in view.ids(data) {
                self.selection.remove(id);
            }
        } else {
            self.selection.extend(view.ids(data).map(str::to_string));
        }
    }

    /// Clear the selection and the anchor.
    pub fn clear_selection(&mut self) {
        self.selection.clear();
        self.anchor = None;
    }

    /// A key over the view. Navigation moves the cursor through the rendered
    /// rows and pulls the window along; shift extends a multi selection from
    /// the anchor (or the row the cursor left).
    pub fn key(&mut self, key: TableKey, shift: bool, view: &View, data: &TableData, page: usize) {
        match key {
            TableKey::Space => {
                if let Some(id) = self.cursor.clone() {
                    let mode = if shift {
                        SelectMode::Range
                    } else {
                        SelectMode::Toggle
                    };
                    self.select(&id, mode, view, data);
                }
            }
            TableKey::Escape => self.clear_selection(),
            TableKey::SelectAll => {
                if self.selectable == Selectable::Multi {
                    self.selection.extend(view.ids(data).map(str::to_string));
                }
            }
            nav => {
                if view.rows.is_empty() {
                    return;
                }
                let page = page.clamp(1, 1_000);
                let last = view.rows.len() - 1;
                let from = self.cursor.as_deref().and_then(|c| view.position(data, c));
                let to = match (nav, from) {
                    (TableKey::Down, Some(at)) => (at + 1).min(last),
                    (TableKey::Up, Some(at)) => at.saturating_sub(1),
                    (TableKey::End, _) => last,
                    (TableKey::PageDown, None) => page.min(last),
                    (TableKey::PageDown, Some(at)) => (at + page).min(last),
                    (TableKey::PageUp, Some(at)) => at.saturating_sub(page),
                    _ => 0,
                };
                let id = view.id(data, to).to_string();
                let left = self.cursor.replace(id.clone());
                if shift && self.selectable == Selectable::Multi {
                    if self.anchor.is_none() {
                        self.anchor = left.or_else(|| Some(id.clone()));
                    }
                    self.select(&id, SelectMode::Range, view, data);
                }
                self.follow(view.row_entries[to]);
            }
        }
    }

    /// A cursor that left the mounted window moves the window with it, so the
    /// row exists to focus. `at` is the cursor's entry index.
    fn follow(&mut self, at: usize) {
        let TableWindow { start, count, .. } = self.window;
        let edge = OVERSCAN.min(count / 4);
        if at < start + edge || at >= (start + count).saturating_sub(edge) {
            self.window.start = at.saturating_sub(count / 2);
        }
    }

    /// The client's scroll position: start and count clamp, and a row height
    /// that is not positive keeps the measured one.
    pub fn set_window(&mut self, start: usize, count: usize, row_height: Option<f64>) {
        self.window = TableWindow {
            start: start.min(10_000_000),
            count: count.clamp(1, MAX_WINDOW),
            row_height: row_height
                .filter(|h| h.is_finite() && *h > 0.0)
                .or(self.window.row_height),
        };
    }

    /// Which of `total` entries the body mounts. Unwindowed, all of them;
    /// windowed, the window's slice clamped into range, with the entries above
    /// and below it left to spacers.
    pub fn mounted(&self, total: usize, windowed: bool) -> Mounted {
        if !windowed {
            return Mounted {
                start: 0,
                end: total,
                above: 0,
                below: 0,
            };
        }
        let count = self.window.count;
        let start = self.window.start.min(total.saturating_sub(count));
        let end = (start + count).min(total);
        Mounted {
            start,
            end,
            above: start,
            below: total - end,
        }
    }

    /// The row height the window and spacers use.
    pub fn row_height(&self) -> f64 {
        self.window.row_height.unwrap_or(self.density.row_height())
    }

    /// Toggle a tag in the filter. `all` (shift) switches to all-of first, so
    /// the gesture that adds a tag also says how it combines.
    pub fn toggle_tag(&mut self, tag: &str, all: bool) {
        let tag = canonical_tag(tag);
        if tag.is_empty() {
            return;
        }
        if !self.tag_filter.remove(&tag) {
            self.tag_filter.insert(tag);
        }
        if all {
            self.tag_mode = TagMode::All;
        }
    }

    /// Empty the tag filter and leave tag focus.
    pub fn clear_tags(&mut self) {
        self.tag_filter.clear();
        self.tag_mode = TagMode::Any;
        self.tag_focus = None;
        self.tags_expand = false;
    }

    /// Put one segment in the tab order; its row becomes the cursor, since a
    /// segment is only reachable on the cursor row. `None` leaves tag focus.
    pub fn focus_tag(&mut self, focus: Option<(String, String)>) {
        match focus {
            Some((row, tag)) => {
                self.cursor = Some(row.clone());
                self.tag_focus = Some((row, canonical_tag(&tag)));
            }
            None => self.tag_focus = None,
        }
    }

    /// The durable part of the state, as a host stores it.
    pub fn persisted(&self) -> PersistedTable {
        PersistedTable {
            v: STATE_VERSION,
            sort: self
                .sort
                .iter()
                .map(|s| (s.column.clone(), s.dir))
                .collect(),
            filters: self.filters.clone(),
            quick: self.quick.clone(),
            filters_open: self.filters_open,
            order: self.order.clone(),
            hidden: self.hidden.iter().cloned().collect(),
            widths: self.widths.clone(),
            group_by: self.group_by.clone(),
            density: self.density,
            view: self.view,
        }
    }

    /// Apply a stored payload. Untrusted: each field is checked against the
    /// columns the table has now, and a field that does not fit keeps its
    /// current value.
    pub fn restore(&mut self, stored: &StoredTable) {
        let keys = self.order.clone();
        let known = |id: &str| keys.iter().any(|k| k == id);
        let texts = |list: &[Field<String>]| -> Vec<String> {
            list.iter().filter_map(Field::value).cloned().collect()
        };

        if let Some(sort) = stored.sort.value() {
            let pairs = sort.iter().filter_map(Field::value);
            self.sort = valid_sort(
                pairs.map(|(key, dir)| (key.as_str(), parse_dir(dir))),
                &keys,
            );
        }
        if let Some(filters) = stored.filters.value() {
            self.filters = filters
                .iter()
                .filter(|(id, _)| known(id))
                .filter_map(|(id, filter)| {
                    let filter = match filter.value()? {
                        StoredFilter::Text(text) => {
                            (!text.is_empty()).then(|| ColumnFilter::Text(text.clone()))
                        }
                        StoredFilter::Values(values) => {
                            ColumnFilter::values(texts(values).iter().map(String::as_str))
                        }
                        StoredFilter::Range { min, max } => ColumnFilter::range(
                            min.value().and_then(|n| n.get()),
                            max.value().and_then(|n| n.get()),
                        ),
                    };
                    Some((id.clone(), filter?))
                })
                .collect();
        }
        if let Some(quick) = stored.quick.value() {
            self.set_quick(quick);
        }
        if let Some(open) = stored.filters_open.value() {
            self.filters_open = *open;
        }
        if let Some(order) = stored.order.value() {
            let mut next: Vec<String> = Vec::new();
            for id in texts(order) {
                if known(&id) && !next.contains(&id) {
                    next.push(id);
                }
            }
            let missing: Vec<String> = keys.iter().filter(|k| !next.contains(k)).cloned().collect();
            next.extend(missing);
            self.order = next;
        }
        if let Some(hidden) = stored.hidden.value() {
            let set: BTreeSet<String> = texts(hidden).into_iter().filter(|id| known(id)).collect();
            if set.len() < keys.len() {
                self.hidden = set;
            }
        }
        if let Some(widths) = stored.widths.value() {
            self.widths = widths
                .iter()
                .filter(|(id, _)| known(id))
                .filter_map(|(id, width)| Some((id.clone(), clamp_width(width.value()?.get()?))))
                .collect();
        }
        match stored.group_by.value() {
            Some(None) => self.group_by = None,
            Some(Some(id)) if known(id) => self.group_by = Some(id.clone()),
            _ => {}
        }
        if let Some(density) = stored.density.value() {
            self.density = *density;
        }
        if let Some(view) = stored.view.value() {
            self.view = *view;
        }
    }

    /// Back to the declared layout and query: sort, filters, order, hidden
    /// columns, widths, grouping, density and view. Selection, cursor and the
    /// tag filter are the session's and stay.
    pub fn reset(&mut self) {
        let defaults = &self.defaults;
        self.sort = defaults.sort.clone();
        self.filters.clear();
        self.quick.clear();
        self.order = defaults.order.clone();
        self.hidden = defaults.hidden.clone();
        self.widths.clear();
        self.group_by = defaults.group_by.clone();
        self.collapsed.clear();
        self.density = defaults.density;
        self.view = defaults.view;
    }
}

/// The entries a body mounts and the spacer rows around them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mounted {
    /// First mounted entry.
    pub start: usize,
    /// One past the last mounted entry.
    pub end: usize,
    /// Entries above the window (the top spacer is this many rows tall).
    pub above: usize,
    /// Entries below the window.
    pub below: usize,
}

/// The durable state a host keeps per table, JSON-shaped: sort pairs are
/// `[column, "asc"|"desc"]` and filters are a string, a list or `{min, max}`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PersistedTable {
    /// Payload version.
    pub v: u32,
    /// Sort keys in priority order.
    pub sort: Vec<(String, SortDir)>,
    /// Per-column filters.
    pub filters: BTreeMap<String, ColumnFilter>,
    /// The quick filter.
    pub quick: String,
    /// Whether the filter row was open.
    pub filters_open: bool,
    /// Column order.
    pub order: Vec<String>,
    /// Hidden column ids, sorted.
    pub hidden: Vec<String>,
    /// Dragged widths.
    pub widths: BTreeMap<String, u32>,
    /// Grouping column.
    pub group_by: Option<String>,
    /// Density.
    pub density: Density,
    /// Rows or cards.
    pub view: ViewMode,
}

/// One interaction a painter raises; [`TableEvent::apply`] is the one door.
#[derive(Clone, Debug, PartialEq)]
pub enum TableEvent {
    /// A header click; `append` when shift was held.
    Sort {
        /// The column.
        column: String,
        /// Refine the sort instead of replacing it.
        append: bool,
    },
    /// A column moved.
    MoveColumn(String, MoveTarget),
    /// A column's dragged width, in pixels.
    Resize(String, f64),
    /// A column shown or hidden.
    ToggleColumn(String),
    /// The quick filter's text.
    Quick(String),
    /// One column's filter set or cleared.
    Filter(String, Option<ColumnFilter>),
    /// The filter row shown or hidden.
    ToggleFilters,
    /// Every filter cleared, tags included.
    Clear,
    /// Grouped by a column, or ungrouped.
    Group(Option<String>),
    /// A group folded or unfolded.
    ToggleGroup(String),
    /// The density toggled.
    ToggleDensity,
    /// The view mode set.
    View(ViewMode),
    /// A row selected.
    Select {
        /// The row.
        id: String,
        /// How.
        mode: SelectMode,
    },
    /// Every visible row selected, or deselected when all were.
    SelectAll,
    /// The cursor put on a row.
    Cursor(String),
    /// A key the table owns.
    Key {
        /// Which.
        key: TableKey,
        /// Shift held.
        shift: bool,
        /// Rows a page moves.
        page: usize,
    },
    /// The client's mounted window.
    Window {
        /// First mounted entry.
        start: usize,
        /// Entries mounted.
        count: usize,
        /// Measured row height.
        row_height: Option<f64>,
    },
    /// A tag toggled in the filter; `all` when shift was held.
    Tag {
        /// The tag.
        tag: String,
        /// Switch to all-of.
        all: bool,
    },
    /// The tag combinator set.
    TagMode(TagMode),
    /// The tag filter emptied.
    ClearTags,
    /// A segment focused, or tag focus left.
    TagFocus(Option<(String, String)>),
    /// The cursor row's tags spelled out, or folded back.
    TagsExpand(bool),
    /// A stored payload applied.
    Restore(StoredTable),
    /// Back to the defaults.
    Reset,
}

impl TableEvent {
    /// Apply to `state`; `view` and `data` are what selection and keys work
    /// over.
    pub fn apply(
        &self,
        state: &mut TableState,
        columns: &[DataColumnSpec],
        view: &View,
        data: &TableData,
    ) {
        match self {
            TableEvent::Sort { column, append } => state.click_header(column, *append),
            TableEvent::MoveColumn(column, target) => state.move_column(column, target),
            TableEvent::Resize(column, width) => state.resize(column, *width),
            TableEvent::ToggleColumn(column) => state.toggle_column(columns, column),
            TableEvent::Quick(text) => state.set_quick(text),
            TableEvent::Filter(column, filter) => state.set_filter(column, filter.clone()),
            TableEvent::ToggleFilters => state.filters_open = !state.filters_open,
            TableEvent::Clear => state.clear_filters(),
            TableEvent::Group(column) => state.set_group(column.as_deref()),
            TableEvent::ToggleGroup(group) => state.toggle_group(group),
            TableEvent::ToggleDensity => state.density = state.density.toggled(),
            TableEvent::View(mode) => state.view = *mode,
            TableEvent::Select { id, mode } => state.select(id, *mode, view, data),
            TableEvent::SelectAll => state.select_all(view, data),
            TableEvent::Cursor(id) => state.cursor = Some(id.clone()),
            TableEvent::Key { key, shift, page } => state.key(*key, *shift, view, data, *page),
            TableEvent::Window {
                start,
                count,
                row_height,
            } => state.set_window(*start, *count, *row_height),
            TableEvent::Tag { tag, all } => state.toggle_tag(tag, *all),
            TableEvent::TagMode(mode) => state.tag_mode = *mode,
            TableEvent::ClearTags => state.clear_tags(),
            TableEvent::TagFocus(focus) => state.focus_tag(focus.clone()),
            TableEvent::TagsExpand(on) => state.tags_expand = *on,
            TableEvent::Restore(payload) => state.restore(payload),
            TableEvent::Reset => state.reset(),
        }
    }

    /// Whether the event changes durable state a host should persist.
    pub fn persists(&self) -> bool {
        !matches!(
            self,
            TableEvent::Select { .. }
                | TableEvent::SelectAll
                | TableEvent::Cursor(_)
                | TableEvent::Key { .. }
                | TableEvent::Window { .. }
                | TableEvent::Tag { .. }
                | TableEvent::TagMode(_)
                | TableEvent::ClearTags
                | TableEvent::TagFocus(_)
                | TableEvent::TagsExpand(_)
                | TableEvent::ToggleGroup(_)
        )
    }

    /// Whether the event changes the query a server-mode host runs.
    pub fn requeries(&self) -> bool {
        matches!(
            self,
            TableEvent::Sort { .. }
                | TableEvent::Quick(_)
                | TableEvent::Filter(..)
                | TableEvent::Clear
                | TableEvent::Restore(_)
                | TableEvent::Reset
        )
    }
}

/// The query a server-mode host runs: sort, filters and the trimmed quick
/// filter.
#[derive(Clone, Debug, PartialEq)]
pub struct TableQuery {
    /// Sort keys in priority order.
    pub sort: Vec<SortSpec>,
    /// Per-column filters.
    pub filters: BTreeMap<String, ColumnFilter>,
    /// The quick filter, trimmed.
    pub quick: String,
}

impl TableState {
    /// The query a server-mode host re-runs.
    pub fn query(&self) -> TableQuery {
        TableQuery {
            sort: self.sort.clone(),
            filters: self.filters.clone(),
            quick: self.quick.trim().to_string(),
        }
    }

    /// What the row order depends on: the query, the visible columns the quick
    /// filter reads, and grouping. Equal keys over the same data order rows
    /// the same way; folding is not part of it.
    pub fn order_key(&self) -> OrderKey {
        OrderKey {
            sort: self.sort.clone(),
            filters: self.filters.clone(),
            quick: self.quick.clone(),
            hidden: self.hidden.clone(),
            group_by: self.group_by.clone(),
            tag_filter: self.tag_filter.clone(),
            tag_mode: self.tag_mode,
        }
    }
}

/// What narrows a filter's rows, for a chip that names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilterKind {
    /// The quick filter.
    Quick,
    /// One column's filter.
    Column,
    /// One tag of the tag filter.
    Tag,
}

/// One thing narrowing the rows, as a removable chip shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct ActiveFilter {
    /// What kind of filter it is.
    pub kind: FilterKind,
    /// What it filters: `"filter"`, a column's title, or `"tag"`.
    pub label: String,
    /// What it asks for, as text.
    pub value: String,
    /// The event that removes it.
    pub remove: TableEvent,
}

impl TableState {
    /// Every active filter: the quick filter, then column filters in column
    /// order, then tags alphabetically.
    pub fn active_filters(&self, columns: &[DataColumnSpec]) -> Vec<ActiveFilter> {
        let quick = self.quick.trim();
        let quick = (!quick.is_empty()).then(|| ActiveFilter {
            kind: FilterKind::Quick,
            label: "filter".to_string(),
            value: quick.to_string(),
            remove: TableEvent::Quick(String::new()),
        });
        let columns = columns.iter().filter_map(|column| {
            let filter = self.filters.get(&column.id)?;
            Some(ActiveFilter {
                kind: FilterKind::Column,
                label: column.title.clone(),
                value: filter.summary(),
                remove: TableEvent::Filter(column.id.clone(), None),
            })
        });
        let tags = self.tag_filter.iter().map(|tag| ActiveFilter {
            kind: FilterKind::Tag,
            label: "tag".to_string(),
            value: tag.clone(),
            remove: TableEvent::Tag {
                tag: tag.clone(),
                all: false,
            },
        });
        quick.into_iter().chain(columns).chain(tags).collect()
    }
}

impl ColumnFilter {
    /// What the filter asks for, as a chip reads it: the text, the values,
    /// or the bounds.
    pub fn summary(&self) -> String {
        let number = |n: f64| {
            let mut out = String::new();
            super::data::write_number(&mut out, n);
            out
        };
        match self {
            ColumnFilter::Text(text) => format!("\u{201c}{text}\u{201d}"),
            ColumnFilter::Values(values) => values.join(", "),
            ColumnFilter::Range {
                min: Some(min),
                max: Some(max),
            } => format!("{}\u{2013}{}", number(*min), number(*max)),
            ColumnFilter::Range {
                min: Some(min),
                max: None,
            } => format!("\u{2265} {}", number(*min)),
            ColumnFilter::Range {
                min: None,
                max: Some(max),
            } => format!("\u{2264} {}", number(*max)),
            ColumnFilter::Range {
                min: None,
                max: None,
            } => String::new(),
        }
    }
}

/// What a view's row order depends on; see [`TableState::order_key`].
#[derive(Clone, Debug, PartialEq)]
pub struct OrderKey {
    sort: Vec<SortSpec>,
    filters: BTreeMap<String, ColumnFilter>,
    quick: String,
    hidden: BTreeSet<String>,
    group_by: Option<String>,
    tag_filter: BTreeSet<String>,
    tag_mode: TagMode,
}

impl OrderKey {
    /// Whether rows under `self` are some of the rows under `earlier`, in the
    /// same order: everything but the quick text is equal, and each earlier
    /// word is part of some word here, so a row matching here matches there.
    pub fn narrows(&self, earlier: &OrderKey) -> bool {
        let words = |key: &OrderKey| -> Vec<String> {
            key.quick
                .split_whitespace()
                .map(super::view::fold)
                .collect()
        };
        let now = words(self);
        self.sort == earlier.sort
            && self.filters == earlier.filters
            && self.hidden == earlier.hidden
            && self.group_by == earlier.group_by
            && self.tag_filter == earlier.tag_filter
            && self.tag_mode == earlier.tag_mode
            && words(earlier)
                .iter()
                .all(|word| now.iter().any(|new| new.contains(word.as_str())))
    }
}

fn clamp_width(width: f64) -> u32 {
    width
        .round()
        .clamp(f64::from(MIN_WIDTH), f64::from(MAX_WIDTH)) as u32
}

fn parse_dir(dir: &str) -> Option<SortDir> {
    match dir {
        "asc" => Some(SortDir::Asc),
        "desc" => Some(SortDir::Desc),
        _ => None,
    }
}

/// Sort keys naming known columns with a direction, first occurrence of each.
fn valid_sort<'a>(
    pairs: impl IntoIterator<Item = (&'a str, Option<SortDir>)>,
    keys: &[String],
) -> Vec<SortSpec> {
    let mut sort: Vec<SortSpec> = Vec::new();
    for (key, dir) in pairs {
        let Some(dir) = dir else { continue };
        if keys.iter().any(|k| k == key) && !sort.iter().any(|s| s.column == key) {
            sort.push(SortSpec::new(key, dir));
        }
    }
    sort
}
