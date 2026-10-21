//! One table's whole user-facing state and the events that change it.
//!
//! Durable parts (sort, filters, layout, grouping, density) persist through
//! [`TableState::persisted`] / [`TableState::restore`]; session parts
//! (selection, cursor, window, tag filter) never do, so a shared link or a
//! fresh load decides them.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::HashSet;
use std::rc::{Rc, Weak};

use serde::Deserialize;
use serde::Serialize;

use super::column::{DataColumnSpec, SortDir, SortSpec};
use super::data::TableData;
use super::scan::RowSet;
use super::scroll::{clean, ScrollMap};
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

    /// Pixel height of every entry (row, group header or card) in this mode.
    /// The painter's CSS fixes the box to this height, so a virtual window's
    /// spacers are `entries × row_height` with nothing measured.
    pub fn row_height(self, density: Density) -> f64 {
        match (self, density) {
            (ViewMode::Rows, density) => density.row_height(),
            (ViewMode::CompactCards, Density::Normal) => 32.0,
            (ViewMode::CompactCards, Density::Compact) => 26.0,
            (ViewMode::DetailCards, Density::Normal) => 64.0,
            (ViewMode::DetailCards, Density::Compact) => 52.0,
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

/// What the client's scroller shows, which a scaled table windows by.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Viewport {
    /// Pixels scrolled.
    pub top: f64,
    /// Height of the rows' area under the header.
    pub visible: f64,
    /// `top` was set by the table, not reported by the client: the client scrolls to it.
    pub seek: bool,
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

/// Where the cursor and anchor rows last rendered in the view. A hint only:
/// lookups verify the id at the position, so a stale one costs a scan, not a
/// wrong answer. Never affects equality.
#[derive(Clone, Copy, Debug, Default)]
pub struct PositionHint {
    cursor: usize,
    anchor: usize,
}

impl PartialEq for PositionHint {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

/// The selection `select_all` last made hold every row of a view, and which
/// rows those were, so [`TableState::all_selected`] needs no lookup per row.
/// It holds the selection weakly: any change to the selection ends the claim.
/// Never affects equality.
#[derive(Clone, Default)]
pub struct Covered(Option<Cover>);

#[derive(Clone)]
struct Cover {
    selection: Weak<BTreeSet<String>>,
    data: TableData,
    rows: Rc<RowSet>,
}

impl Covered {
    /// A record that `selection` holds the id of every row of `view`.
    fn of(selection: &Rc<BTreeSet<String>>, view: &View, data: &TableData) -> Covered {
        let mut rows = RowSet::empty(data.len());
        for &row in view.rows.iter().filter(|&&row| row < data.len()) {
            rows.insert(row);
        }
        Covered(Some(Cover {
            selection: Rc::downgrade(selection),
            data: data.clone(),
            rows: Rc::new(rows),
        }))
    }

    /// Whether the record shows that `selection` holds every row of `view`:
    /// the same selection, untouched since, over the same data.
    fn holds(&self, selection: &Rc<BTreeSet<String>>, view: &View, data: &TableData) -> bool {
        let Some(cover) = &self.0 else {
            return false;
        };
        cover
            .selection
            .upgrade()
            .is_some_and(|held| Rc::ptr_eq(&held, selection))
            && cover.data == *data
            && view
                .rows
                .iter()
                .all(|&row| row < data.len() && cover.rows.contains(row))
    }
}

impl PartialEq for Covered {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl std::fmt::Debug for Covered {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(if self.0.is_some() {
            "Covered(..)"
        } else {
            "Covered(-)"
        })
    }
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
    /// Selected row ids. Shared between clones; a change copies on write.
    pub selection: Rc<BTreeSet<String>>,
    /// The row a range grows from.
    pub anchor: Option<String>,
    /// The keyboard's row: the one tab stop.
    pub cursor: Option<String>,
    /// Where the cursor and anchor last rendered. Never trusted unchecked.
    pub pos: PositionHint,
    /// Whether the selection still holds the rows `select_all` selected.
    pub covered: Covered,
    /// What the client's scroller shows.
    pub viewport: Viewport,
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
            selection: Rc::default(),
            anchor: None,
            cursor: None,
            pos: PositionHint::default(),
            covered: Covered::default(),
            viewport: Viewport::default(),
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
                    self.clear_selection();
                } else {
                    self.selection = Rc::new(BTreeSet::from([id.to_string()]));
                    self.anchor = Some(id.to_string());
                }
            }
            Selectable::Multi => match (mode, self.anchor.clone()) {
                (SelectMode::Only, _) => {
                    self.selection = Rc::new(BTreeSet::from([id.to_string()]));
                    self.anchor = Some(id.to_string());
                }
                (SelectMode::Range, Some(anchor)) => {
                    match (self.find(view, data, &anchor), self.find(view, data, id)) {
                        (Some(a), Some(b)) => {
                            self.pos.anchor = a;
                            let (lo, hi) = (a.min(b), a.max(b));
                            self.add_ids(view.rows[lo..=hi].iter().map(|&row| data.id(row)));
                        }
                        _ => self.add_ids(std::iter::once(id)),
                    }
                }
                _ => {
                    let set = Rc::make_mut(&mut self.selection);
                    if !set.remove(id) {
                        set.insert(id.to_string());
                    }
                    self.anchor = Some(id.to_string());
                }
            },
        }
    }

    /// Whether every rendered row is selected. Right after `select_all` made
    /// it so this reads one bit per row; otherwise it looks each id up.
    pub fn all_selected(&self, view: &View, data: &TableData) -> bool {
        // A selection smaller than the view cannot hold every row of it.
        !view.rows.is_empty()
            && self.selection.len() >= view.rows.len()
            && (self.covered.holds(&self.selection, view, data)
                || view.ids(data).all(|id| self.selection.contains(id)))
    }

    /// Select every rendered row, or deselect them when all already are.
    pub fn select_all(&mut self, view: &View, data: &TableData) {
        if self.selectable != Selectable::Multi || view.rows.is_empty() {
            return;
        }
        if !self.all_selected(view, data) {
            self.add_ids(view.ids(data));
            self.cover(view, data);
        } else if self.selection.len() == view.rows.len() {
            self.selection = Rc::default();
        } else {
            let set = Rc::make_mut(&mut self.selection);
            for id in view.ids(data) {
                set.remove(id);
            }
        }
    }

    /// Record that the selection holds every row of `view`.
    fn cover(&mut self, view: &View, data: &TableData) {
        if !self.covered.holds(&self.selection, view, data) {
            self.covered = Covered::of(&self.selection, view, data);
        }
    }

    /// Add ids to the selection, allocating only those not already in it.
    /// A shared selection is copied only when something is added.
    fn add_ids<'a>(&mut self, ids: impl Iterator<Item = &'a str>) {
        let fresh: Vec<String> = ids
            .filter(|id| !self.selection.contains(*id))
            .map(str::to_string)
            .collect();
        if fresh.is_empty() {
            return;
        }
        let set = Rc::make_mut(&mut self.selection);
        // `append` bulk-builds in linear time but walks the whole set.
        if set.is_empty() || fresh.len() > set.len() / 8 {
            set.append(&mut fresh.into_iter().collect());
        } else {
            set.extend(fresh);
        }
    }

    /// Where the row `id` renders: a remembered position when it still holds
    /// that row, else a scan.
    fn find(&self, view: &View, data: &TableData, id: &str) -> Option<usize> {
        [self.pos.cursor, self.pos.anchor]
            .into_iter()
            .find(|&at| at < view.rows.len() && view.id(data, at) == id)
            .or_else(|| view.position(data, id))
    }

    /// Clear the selection and the anchor.
    pub fn clear_selection(&mut self) {
        if !self.selection.is_empty() {
            self.selection = Rc::default();
        }
        self.anchor = None;
    }

    /// Drop what names a row `data` does not hold: selected ids, the anchor,
    /// the cursor and the tag focus. A selection that loses nothing stays
    /// shared. One pass over `data`, however the state shrinks.
    pub fn retain_rows(&mut self, data: &TableData) {
        self.covered = Covered::default();
        let (wanted, found) = {
            let wanted: HashSet<&str> = self
                .selection
                .iter()
                .map(String::as_str)
                .chain(self.cursor.as_deref())
                .chain(self.anchor.as_deref())
                .chain(self.tag_focus.as_ref().map(|(row, _)| row.as_str()))
                .collect();
            if wanted.is_empty() {
                return;
            }
            let mut found: HashSet<&str> = HashSet::with_capacity(wanted.len());
            for row in 0..data.len() {
                let id = data.id(row);
                if wanted.contains(id) && found.insert(id) && found.len() == wanted.len() {
                    break;
                }
            }
            (wanted.len(), found)
        };
        if found.len() == wanted {
            return;
        }
        let keep = |id: &str| found.contains(id);
        if self.selection.iter().any(|id| !keep(id)) {
            Rc::make_mut(&mut self.selection).retain(|id| keep(id));
        }
        self.anchor = self.anchor.take().filter(|id| keep(id));
        self.cursor = self.cursor.take().filter(|id| keep(id));
        self.tag_focus = self.tag_focus.take().filter(|(row, _)| keep(row));
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
                    self.add_ids(view.ids(data));
                    if !view.rows.is_empty() {
                        self.cover(view, data);
                    }
                }
            }
            nav => {
                if view.rows.is_empty() {
                    return;
                }
                let page = page.clamp(1, 1_000);
                let last = view.rows.len() - 1;
                let from = self
                    .cursor
                    .as_deref()
                    .and_then(|c| self.find(view, data, c));
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
                self.pos.cursor = to;
                if shift && self.selectable == Selectable::Multi {
                    if self.anchor.is_none() {
                        self.anchor = left.or_else(|| Some(id.clone()));
                        self.pos.anchor = from.unwrap_or(to);
                    }
                    self.select(&id, SelectMode::Range, view, data);
                }
                self.follow(view.row_entries[to], view.entries.len());
            }
        }
    }

    /// A cursor that left the mounted window moves the window with it, so the
    /// row exists to focus; a scaled table scrolls the least that shows it.
    /// `at` is the cursor's entry index among `total`.
    fn follow(&mut self, at: usize, total: usize) {
        let map = self.scroll_map(total);
        if map.scaled() {
            if let Some(top) = map.reveal(self.viewport.top, at) {
                self.viewport.top = top;
                self.viewport.seek = true;
            }
            return;
        }
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

    /// The client's scroller: pixels scrolled and the height of the rows'
    /// area. A value that is not a length counts as 0.
    pub fn set_viewport(&mut self, top: f64, visible: f64) {
        self.viewport = Viewport {
            top: clean(top),
            visible: clean(visible),
            seek: false,
        };
    }

    /// The scroll map of `total` entries at the row height and viewport now.
    fn scroll_map(&self, total: usize) -> ScrollMap {
        ScrollMap::new(
            total,
            self.view.row_height(self.density),
            self.viewport.visible,
        )
    }

    /// Which of `total` entries the body mounts. Unwindowed, all of them;
    /// windowed, the window's slice clamped into range (a scaled table's is
    /// the viewport's), with the entries above and below it left to spacers.
    pub fn mounted(&self, total: usize, windowed: bool) -> Mounted {
        if !windowed {
            return Mounted {
                start: 0,
                end: total,
                above: 0,
                below: 0,
                above_px: 0.0,
                below_px: 0.0,
            };
        }
        let map = self.scroll_map(total);
        let (start, end) = if map.scaled() {
            map.window(self.viewport.top)
        } else {
            let count = self.window.count;
            let start = self.window.start.min(total.saturating_sub(count));
            (start, (start + count).min(total))
        };
        let (above_px, below_px) = map.spacers(self.viewport.top, start, end);
        Mounted {
            start,
            end,
            above: start,
            below: total - end,
            above_px,
            below_px,
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
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mounted {
    /// First mounted entry.
    pub start: usize,
    /// One past the last mounted entry.
    pub end: usize,
    /// Entries above the window.
    pub above: usize,
    /// Entries below the window.
    pub below: usize,
    /// Height of the top spacer, in pixels.
    pub above_px: f64,
    /// Height of the bottom spacer, in pixels.
    pub below_px: f64,
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
    /// The client's scroller, reported on each scroll of a scaled table.
    Scroll {
        /// Pixels scrolled.
        top: f64,
        /// Height of the rows' area under the header.
        visible: f64,
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
    /// The table's rows were replaced by `data`: selected ids, the anchor, the
    /// cursor and the tag focus that name no row of it are dropped.
    RowsReplaced,
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
            TableEvent::Scroll { top, visible } => state.set_viewport(*top, *visible),
            TableEvent::Tag { tag, all } => state.toggle_tag(tag, *all),
            TableEvent::TagMode(mode) => state.tag_mode = *mode,
            TableEvent::ClearTags => state.clear_tags(),
            TableEvent::TagFocus(focus) => state.focus_tag(focus.clone()),
            TableEvent::TagsExpand(on) => state.tags_expand = *on,
            TableEvent::Restore(payload) => state.restore(payload),
            TableEvent::Reset => state.reset(),
            TableEvent::RowsReplaced => state.retain_rows(data),
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
                | TableEvent::Scroll { .. }
                | TableEvent::Tag { .. }
                | TableEvent::TagMode(_)
                | TableEvent::ClearTags
                | TableEvent::TagFocus(_)
                | TableEvent::TagsExpand(_)
                | TableEvent::ToggleGroup(_)
                | TableEvent::RowsReplaced
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::data_table::{BuildOptions, TableDataBuilder, Value};
    use proptest::prelude::*;

    fn columns() -> Vec<DataColumnSpec> {
        vec![
            DataColumnSpec::new("item", "item"),
            DataColumnSpec::new("since", "since").number(),
        ]
    }

    fn data(n: usize) -> TableData {
        let columns = columns();
        let mut builder = TableDataBuilder::with_capacity(&columns, n);
        for i in 0..n {
            let id = format!("r{i}");
            builder.row(
                id.as_str(),
                [Value::Text(&id), Value::Number(((i * 7) % 11) as f64)],
            );
        }
        builder.build()
    }

    fn multi() -> TableState {
        TableState {
            selectable: Selectable::Multi,
            ..TableState::new(&columns(), TableDefaults::default())
        }
    }

    /// Insertion order, newest-first, or only the rows matching "1".
    fn view_of(kind: u8, data: &TableData) -> View {
        let mut query = multi();
        match kind % 3 {
            0 => {}
            1 => query.sort = vec![SortSpec::new("since", SortDir::Desc)],
            _ => query.set_quick("1"),
        }
        View::build(&columns(), data, &query, BuildOptions::default())
    }

    const KEYS: [TableKey; 6] = [
        TableKey::Down,
        TableKey::Up,
        TableKey::PageDown,
        TableKey::PageUp,
        TableKey::Home,
        TableKey::End,
    ];

    proptest! {
        // A hint left over from another view order must give the scan's answer.
        #[test]
        fn keys_over_changing_views_match_the_scan(
            n in 1usize..60,
            steps in prop::collection::vec((0u8..3, 0usize..6, any::<bool>()), 1..30),
        ) {
            let data = data(n);
            let mut hinted = multi();
            let mut scanned = multi();
            for (kind, key, shift) in steps {
                let view = view_of(kind, &data);
                scanned.pos = PositionHint { cursor: usize::MAX, anchor: usize::MAX };
                hinted.key(KEYS[key], shift, &view, &data, 7);
                scanned.key(KEYS[key], shift, &view, &data, 7);
                prop_assert_eq!(&hinted.cursor, &scanned.cursor);
                prop_assert_eq!(&hinted.anchor, &scanned.anchor);
                prop_assert_eq!(&hinted.selection, &scanned.selection);
                prop_assert_eq!(hinted.window, scanned.window);
            }
        }

        // Select-all is the set union, or the set difference once every row is in.
        #[test]
        fn select_all_matches_set_algebra(
            n in 1usize..80,
            picked in prop::collection::vec(0usize..100, 0..90),
            kind in 0u8..3,
        ) {
            let data = data(n);
            let view = view_of(kind, &data);
            let mut state = multi();
            state.selection = Rc::new(picked.iter().map(|i| format!("r{i}")).collect());
            let before = state.selection.clone();
            let rows: BTreeSet<String> = view.ids(&data).map(str::to_string).collect();
            let want: BTreeSet<String> = if rows.is_subset(&before) {
                before.difference(&rows).cloned().collect()
            } else {
                before.union(&rows).cloned().collect()
            };
            state.select_all(&view, &data);
            prop_assert_eq!(&*state.selection, &want);
        }
    }

    #[test]
    fn a_clone_shares_the_selection_until_either_side_changes_it() {
        let data = data(50);
        let view = view_of(0, &data);
        let mut state = multi();
        state.select_all(&view, &data);
        let mut copy = state.clone();
        assert!(Rc::ptr_eq(&state.selection, &copy.selection));
        copy.clear_selection();
        assert_eq!(state.selection.len(), 50);
        assert!(copy.selection.is_empty());
    }

    #[test]
    fn selecting_rows_already_selected_leaves_a_shared_selection_shared() {
        let data = data(20);
        let view = view_of(0, &data);
        let mut state = multi();
        state.select_all(&view, &data);
        let copy = state.clone();
        state.key(TableKey::SelectAll, false, &view, &data, 7);
        assert!(Rc::ptr_eq(&state.selection, &copy.selection));
    }

    #[test]
    fn a_cursor_off_the_view_starts_navigation_from_the_edge() {
        let data = data(30);
        let all = view_of(0, &data);
        let narrowed = view_of(2, &data);
        let mut state = multi();
        state.cursor = Some("r5".into());
        state.key(TableKey::Down, false, &all, &data, 7);
        assert_eq!(state.cursor.as_deref(), Some("r6"));
        // r6 is filtered out: the stale hint must not resolve it.
        state.key(TableKey::Down, false, &narrowed, &data, 7);
        assert_eq!(state.cursor.as_deref(), Some(narrowed.id(&data, 0)));
    }

    #[test]
    fn retaining_rows_drops_what_names_a_row_the_data_lacks() {
        let rows = data(10);
        let mut state = multi();
        state.selection = Rc::new(["r1", "gone", "r8"].map(String::from).into());
        state.anchor = Some("r8".into());
        state.cursor = Some("lost".into());
        state.tag_focus = Some(("lost".into(), "a".into()));
        state.retain_rows(&rows);
        assert_eq!(
            *state.selection,
            BTreeSet::from(["r1".to_string(), "r8".to_string()])
        );
        assert_eq!(state.anchor.as_deref(), Some("r8"));
        assert_eq!((state.cursor.as_deref(), &state.tag_focus), (None, &None));
        // Nothing lost: the selection stays the one allocation.
        let before = state.selection.clone();
        state.retain_rows(&rows);
        assert!(Rc::ptr_eq(&state.selection, &before));
        state.retain_rows(&data(0));
        assert!(state.selection.is_empty() && state.anchor.is_none());
    }

    #[test]
    fn select_all_records_the_rows_it_covered_until_the_selection_or_the_data_changes() {
        let rows = data(30);
        let view = view_of(0, &rows);
        let mut state = multi();
        state.select_all(&view, &rows);
        assert!(state.covered.holds(&state.selection, &view, &rows));
        // Any view of rows it covered is covered: another order, fewer rows.
        let narrowed = view_of(2, &rows);
        let sorted = view_of(1, &rows);
        assert!(state.covered.holds(&state.selection, &narrowed, &rows));
        assert!(state.covered.holds(&state.selection, &sorted, &rows));
        // A clone that changes its selection ends the claim for itself alone.
        let mut copy = state.clone();
        copy.select("r3", SelectMode::Toggle, &view, &rows);
        assert!(!copy.covered.holds(&copy.selection, &view, &rows));
        assert!(state.covered.holds(&state.selection, &view, &rows));
        // Rows the selection never saw are not covered.
        let grown = rows.append(|builder| {
            builder.row("r30", [Value::Text("r30"), Value::Number(0.0)]);
        });
        let longer = view_of(0, &grown);
        assert!(!state.covered.holds(&state.selection, &longer, &grown));
        assert!(!state.all_selected(&longer, &grown));
    }

    // -- laws over the events ------------------------------------------------

    fn law_columns() -> Vec<DataColumnSpec> {
        vec![
            DataColumnSpec::new("item", "item").pinned(),
            DataColumnSpec::new("kind", "kind").enumerated(),
            DataColumnSpec::new("since", "since").number(),
            DataColumnSpec::new("note", "note"),
        ]
    }

    /// Rows `r{n}` for each `n`, in that order.
    fn law_rows(builder: &mut TableDataBuilder, ids: &[usize]) {
        for &n in ids {
            let id = format!("r{n}");
            let tags: &[&str] = match n % 4 {
                0 => &[],
                1 => &["a"],
                2 => &["b"],
                _ => &["a", "b"],
            };
            builder
                .row(
                    id.as_str(),
                    [
                        Value::Text(&id),
                        Value::Text(["open", "done", "hold"][n % 3]),
                        Value::Number((n % 7) as f64),
                        Value::Text(if n % 2 == 0 { "even" } else { "odd" }),
                    ],
                )
                .tags(tags);
        }
    }

    fn law_data(ids: &[usize]) -> TableData {
        let mut builder = TableDataBuilder::with_capacity(&law_columns(), ids.len());
        law_rows(&mut builder, ids);
        builder.build()
    }

    fn law_state() -> TableState {
        TableState {
            selectable: Selectable::Multi,
            ..TableState::new(&law_columns(), TableDefaults::default())
        }
    }

    fn distinct(ids: &[usize]) -> Vec<usize> {
        let mut seen = BTreeSet::new();
        ids.iter().copied().filter(|n| seen.insert(*n)).collect()
    }

    /// `event` over the view the state gives now, as a painter raises it.
    fn run(event: &TableEvent, state: &mut TableState, data: &TableData) {
        let view = View::build(&law_columns(), data, state, BuildOptions::default());
        event.apply(state, &law_columns(), &view, data);
    }

    fn column() -> impl Strategy<Value = String> {
        prop::sample::select(vec!["item", "kind", "since", "note", "nope"]).prop_map(String::from)
    }

    fn word() -> impl Strategy<Value = String> {
        prop::sample::select(vec![
            "", " ", "1", "a", "b", " r1 ", "R2", "open", "even", "zz",
        ])
        .prop_map(String::from)
    }

    /// The tags rows hold, one that folds to one of them, and a blank.
    fn tag() -> impl Strategy<Value = String> {
        prop::sample::select(vec!["a", "b", " A ", ""]).prop_map(String::from)
    }

    fn column_filter() -> impl Strategy<Value = Option<ColumnFilter>> {
        prop_oneof![
            Just(None),
            word().prop_map(|w| ColumnFilter::text(&w)),
            prop::collection::vec(word(), 0..3)
                .prop_map(|v| ColumnFilter::values(v.iter().map(String::as_str))),
            (prop::option::of(-2i32..8), prop::option::of(-2i32..8))
                .prop_map(|(lo, hi)| ColumnFilter::range(lo.map(f64::from), hi.map(f64::from))),
        ]
    }

    fn stored() -> impl Strategy<Value = StoredTable> {
        prop::sample::select(vec![
            r#"{"sort":[["since","desc"]],"quick":"r1"}"#,
            r#"{"filters":{"kind":["open"]},"hidden":["note"],"density":"compact"}"#,
            r#"{"order":["note","item"],"group_by":"kind","view":"detail_cards"}"#,
            r#"{"sort":[["nope","asc"],["item","asc"]],"widths":{"item":120},"filters_open":true}"#,
            r#"{"hidden":["item","kind","since","note"],"quick":"  "}"#,
            "{}",
        ])
        .prop_map(|json| serde_json::from_str(json).expect("a stored table"))
    }

    fn key() -> impl Strategy<Value = TableKey> {
        prop::sample::select(vec![
            TableKey::Down,
            TableKey::Up,
            TableKey::Home,
            TableKey::End,
            TableKey::PageDown,
            TableKey::PageUp,
            TableKey::Space,
            TableKey::Escape,
            TableKey::SelectAll,
        ])
    }

    fn select_mode() -> impl Strategy<Value = SelectMode> {
        prop::sample::select(vec![
            SelectMode::Toggle,
            SelectMode::Only,
            SelectMode::Range,
        ])
    }

    /// Every event that names no row.
    fn plain_event() -> impl Strategy<Value = TableEvent> {
        let target = prop_oneof![
            Just(MoveTarget::Left),
            Just(MoveTarget::Right),
            column().prop_map(MoveTarget::Before),
            column().prop_map(MoveTarget::After),
        ];
        let modes = prop::sample::select(vec![
            ViewMode::Rows,
            ViewMode::CompactCards,
            ViewMode::DetailCards,
        ]);
        let groups = prop::sample::select(vec!["open", "done", "hold", "x"]).prop_map(String::from);
        prop_oneof![
            (column(), any::<bool>())
                .prop_map(|(column, append)| TableEvent::Sort { column, append }),
            (column(), target).prop_map(|(column, to)| TableEvent::MoveColumn(column, to)),
            (column(), -50.0..5000.0f64)
                .prop_map(|(column, width)| TableEvent::Resize(column, width)),
            column().prop_map(TableEvent::ToggleColumn),
            word().prop_map(TableEvent::Quick),
            (column(), column_filter())
                .prop_map(|(column, filter)| TableEvent::Filter(column, filter)),
            Just(TableEvent::ToggleFilters),
            Just(TableEvent::Clear),
            prop::option::of(column()).prop_map(TableEvent::Group),
            groups.prop_map(TableEvent::ToggleGroup),
            Just(TableEvent::ToggleDensity),
            modes.prop_map(TableEvent::View),
            (0usize..200, 0usize..100, prop::option::of(-5.0..40.0f64)).prop_map(
                |(start, count, row_height)| TableEvent::Window {
                    start,
                    count,
                    row_height
                }
            ),
            (-10.0..5000.0f64, -10.0..800.0f64)
                .prop_map(|(top, visible)| TableEvent::Scroll { top, visible }),
            (tag(), any::<bool>()).prop_map(|(tag, all)| TableEvent::Tag { tag, all }),
            prop::sample::select(vec![TagMode::Any, TagMode::All]).prop_map(TableEvent::TagMode),
            Just(TableEvent::ClearTags),
            any::<bool>().prop_map(TableEvent::TagsExpand),
            stored().prop_map(TableEvent::Restore),
            Just(TableEvent::Reset),
            Just(TableEvent::RowsReplaced),
        ]
    }

    /// Rows the pool may or may not hold: the ids events name.
    fn row_id() -> impl Strategy<Value = String> {
        prop::sample::select(vec!["r0", "r1", "r2", "r3", "r4", "r5", "ghost"])
            .prop_map(String::from)
    }

    fn row_event() -> impl Strategy<Value = TableEvent> {
        prop_oneof![
            (row_id(), select_mode()).prop_map(|(id, mode)| TableEvent::Select { id, mode }),
            Just(TableEvent::SelectAll),
            row_id().prop_map(TableEvent::Cursor),
            (key(), any::<bool>(), 0usize..30).prop_map(|(key, shift, page)| TableEvent::Key {
                key,
                shift,
                page
            }),
            prop::option::of((row_id(), tag())).prop_map(TableEvent::TagFocus),
        ]
    }

    fn event() -> impl Strategy<Value = TableEvent> {
        prop_oneof![3 => plain_event(), 2 => row_event()]
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(1_000))]

        /// A false flag is a promise: the event leaves the query, or the state a
        /// host stores, as it found it.
        #[test]
        fn an_event_that_does_not_requery_or_persist_leaves_those_alone(
            rows in 1usize..12,
            before in prop::collection::vec(event(), 0..12),
            event in event(),
        ) {
            let data = law_data(&(0..rows).collect::<Vec<_>>());
            let mut state = law_state();
            for earlier in &before {
                run(earlier, &mut state, &data);
            }
            let (query, persisted) = (state.query(), state.persisted());
            run(&event, &mut state, &data);
            if !event.requeries() {
                prop_assert_eq!(state.query(), query, "{:?} changed the query", event);
            }
            if !event.persists() {
                prop_assert_eq!(state.persisted(), persisted, "{:?} changed what persists", event);
            }
        }

        /// The painter keeps a view while the order key and the folds hold: an
        /// event applied over that view does what it does over a fresh one.
        #[test]
        fn a_reused_view_serves_events_as_a_fresh_one_does(
            rows in 1usize..16,
            events in prop::collection::vec(event(), 1..40),
        ) {
            let columns = law_columns();
            let data = law_data(&(0..rows).collect::<Vec<_>>());
            let options = BuildOptions::default();
            let (mut fresh, mut reused) = (law_state(), law_state());
            let mut held: Option<(OrderKey, BTreeSet<String>, View)> = None;
            for event in &events {
                let now = View::build(&columns, &data, &fresh, options);
                let (key, folds) = (reused.order_key(), reused.collapsed.clone());
                if held.as_ref().is_none_or(|(k, f, _)| *k != key || *f != folds) {
                    held = Some((key, folds, View::build(&columns, &data, &reused, options)));
                }
                let (_, _, kept) = held.as_ref().expect("a view is held");
                prop_assert_eq!(kept, &now, "the view held for {:?}", event);
                event.apply(&mut fresh, &columns, &now, &data);
                event.apply(&mut reused, &columns, kept, &data);
                prop_assert_eq!(&fresh, &reused, "after {:?}", event);
            }
        }
    }

    /// One of each event, each with a parameter that changes something.
    fn every_event() -> Vec<TableEvent> {
        let stored = serde_json::from_str(
            r#"{"sort":[["since","desc"]],"quick":"r1","hidden":["note"],"density":"compact"}"#,
        )
        .expect("a stored table");
        vec![
            TableEvent::Sort {
                column: "item".into(),
                append: false,
            },
            TableEvent::MoveColumn("item".into(), MoveTarget::Right),
            TableEvent::Resize("item".into(), 140.0),
            TableEvent::ToggleColumn("note".into()),
            TableEvent::Quick("zz".into()),
            TableEvent::Filter("kind".into(), ColumnFilter::values(["open"])),
            TableEvent::ToggleFilters,
            TableEvent::Clear,
            TableEvent::Group(Some("kind".into())),
            TableEvent::ToggleGroup("open".into()),
            TableEvent::ToggleDensity,
            TableEvent::View(ViewMode::DetailCards),
            TableEvent::Select {
                id: "r1".into(),
                mode: SelectMode::Toggle,
            },
            TableEvent::SelectAll,
            TableEvent::Cursor("r2".into()),
            TableEvent::Key {
                key: TableKey::Down,
                shift: false,
                page: 5,
            },
            TableEvent::Window {
                start: 10,
                count: 30,
                row_height: Some(30.0),
            },
            TableEvent::Scroll {
                top: 100.0,
                visible: 400.0,
            },
            TableEvent::Tag {
                tag: "a".into(),
                all: true,
            },
            TableEvent::TagMode(TagMode::All),
            TableEvent::ClearTags,
            TableEvent::TagFocus(Some(("r1".into(), "a".into()))),
            TableEvent::TagsExpand(true),
            TableEvent::Restore(stored),
            TableEvent::Reset,
            TableEvent::RowsReplaced,
        ]
    }

    /// A flag that is true is true because some state it changes names it; a
    /// flag that is false leaves every state alone.
    #[test]
    fn a_flag_is_true_exactly_when_some_state_can_change_what_it_names() {
        // Adding an event breaks this match until the list above holds it.
        fn slot(event: &TableEvent) -> usize {
            match event {
                TableEvent::Sort { .. } => 0,
                TableEvent::MoveColumn(..) => 1,
                TableEvent::Resize(..) => 2,
                TableEvent::ToggleColumn(_) => 3,
                TableEvent::Quick(_) => 4,
                TableEvent::Filter(..) => 5,
                TableEvent::ToggleFilters => 6,
                TableEvent::Clear => 7,
                TableEvent::Group(_) => 8,
                TableEvent::ToggleGroup(_) => 9,
                TableEvent::ToggleDensity => 10,
                TableEvent::View(_) => 11,
                TableEvent::Select { .. } => 12,
                TableEvent::SelectAll => 13,
                TableEvent::Cursor(_) => 14,
                TableEvent::Key { .. } => 15,
                TableEvent::Window { .. } => 16,
                TableEvent::Scroll { .. } => 17,
                TableEvent::Tag { .. } => 18,
                TableEvent::TagMode(_) => 19,
                TableEvent::ClearTags => 20,
                TableEvent::TagFocus(_) => 21,
                TableEvent::TagsExpand(_) => 22,
                TableEvent::Restore(_) => 23,
                TableEvent::Reset => 24,
                TableEvent::RowsReplaced => 25,
            }
        }
        let events = every_event();
        let slots: Vec<usize> = events.iter().map(slot).collect();
        assert_eq!(
            slots,
            (0..events.len()).collect::<Vec<_>>(),
            "one of each event"
        );

        let data = law_data(&(0..8).collect::<Vec<_>>());
        let worked = [
            TableEvent::Sort {
                column: "since".into(),
                append: false,
            },
            TableEvent::Sort {
                column: "note".into(),
                append: true,
            },
            TableEvent::Quick("e".into()),
            TableEvent::Filter("kind".into(), ColumnFilter::values(["open", "done"])),
            TableEvent::Group(Some("kind".into())),
            TableEvent::ToggleGroup("open".into()),
            TableEvent::Tag {
                tag: "a".into(),
                all: false,
            },
            TableEvent::ToggleColumn("note".into()),
            TableEvent::Resize("item".into(), 90.0),
            TableEvent::ToggleDensity,
            TableEvent::ToggleFilters,
            TableEvent::View(ViewMode::CompactCards),
            TableEvent::MoveColumn("note".into(), MoveTarget::Left),
        ];
        let touched = [
            TableEvent::Select {
                id: "r1".into(),
                mode: SelectMode::Toggle,
            },
            TableEvent::Cursor("r2".into()),
            TableEvent::TagFocus(Some(("r1".into(), "a".into()))),
            TableEvent::TagsExpand(true),
        ];
        let mut states = vec![law_state()];
        for setup in [&worked[..], &touched[..]] {
            let mut state = law_state();
            for event in setup {
                run(event, &mut state, &data);
            }
            states.push(state);
        }
        for event in &events {
            let (mut queries, mut persists) = (false, false);
            for state in &states {
                let mut next = state.clone();
                run(event, &mut next, &data);
                queries |= next.query() != state.query();
                persists |= next.persisted() != state.persisted();
            }
            assert_eq!(event.requeries(), queries, "{event:?}: requeries");
            assert_eq!(event.persists(), persists, "{event:?}: persists");
        }
    }

    #[derive(Clone, Debug)]
    enum Step {
        Select(prop::sample::Index, SelectMode),
        SelectAll,
        Key(TableKey, bool),
        Cursor(prop::sample::Index),
        Focus(prop::sample::Index),
        Plain(TableEvent),
        /// Ids the host put in the selection, held by the data or not.
        Foreign(Vec<usize>),
        Append(usize),
        /// New data built apart, and the state pruned to it.
        Replace(Vec<usize>),
        /// New data built apart that the state is not told of.
        Swap(Vec<usize>),
    }

    fn step() -> impl Strategy<Value = Step> {
        prop_oneof![
            3 => (any::<prop::sample::Index>(), select_mode()).prop_map(|(at, mode)| Step::Select(at, mode)),
            3 => Just(Step::SelectAll),
            2 => (key(), any::<bool>()).prop_map(|(key, shift)| Step::Key(key, shift)),
            1 => any::<prop::sample::Index>().prop_map(Step::Cursor),
            1 => any::<prop::sample::Index>().prop_map(Step::Focus),
            3 => plain_event().prop_map(Step::Plain),
            1 => prop::collection::vec(0usize..40, 1..4).prop_map(Step::Foreign),
            2 => (1usize..6).prop_map(Step::Append),
            2 => prop::collection::vec(0usize..40, 0..20).prop_map(Step::Replace),
            1 => prop::collection::vec(0usize..40, 0..20).prop_map(Step::Swap),
        ]
    }

    proptest! {
        /// Replacing the data keeps of the selection, the anchor, the cursor and
        /// the tag focus what the new rows hold; appending touches none of them,
        /// so ids the host selected ahead of their rows wait for them. Whatever
        /// happened, `all_selected` is what looking each row up says.
        #[test]
        fn selection_follows_the_rows_through_replacements_and_appends(
            first in prop::collection::vec(0usize..40, 1..20),
            steps in prop::collection::vec(step(), 1..40),
        ) {
            let columns = law_columns();
            let mut data = law_data(&distinct(&first));
            let mut fresh = 40;
            let mut state = law_state();
            for step in steps {
                let view = View::build(&columns, &data, &state, BuildOptions::default());
                let ids: Vec<String> = view.ids(&data).map(str::to_string).collect();
                let row = |at: &prop::sample::Index| ids.get(at.index(ids.len().max(1))).cloned();
                let before = state.clone();
                match &step {
                    Step::Select(at, mode) => {
                        if let Some(id) = row(at) {
                            TableEvent::Select { id, mode: *mode }.apply(&mut state, &columns, &view, &data);
                        }
                    }
                    Step::SelectAll => TableEvent::SelectAll.apply(&mut state, &columns, &view, &data),
                    Step::Key(key, shift) => {
                        TableEvent::Key { key: *key, shift: *shift, page: 7 }
                            .apply(&mut state, &columns, &view, &data);
                    }
                    Step::Cursor(at) => {
                        if let Some(id) = row(at) {
                            TableEvent::Cursor(id).apply(&mut state, &columns, &view, &data);
                        }
                    }
                    Step::Focus(at) => {
                        if let Some(id) = row(at) {
                            TableEvent::TagFocus(Some((id, "a".into()))).apply(&mut state, &columns, &view, &data);
                        }
                    }
                    Step::Plain(event) => event.apply(&mut state, &columns, &view, &data),
                    Step::Foreign(ids) => {
                        let mut set = (*state.selection).clone();
                        set.extend(ids.iter().map(|n| format!("r{n}")));
                        state.selection = Rc::new(set);
                    }
                    Step::Append(count) => {
                        let earlier = data.clone();
                        let added: Vec<usize> = (fresh..fresh + count).collect();
                        fresh += count;
                        data = data.append(|builder| law_rows(builder, &added));
                        prop_assert!(data.extends(&earlier));
                        prop_assert_eq!(&state, &before, "appending changed the state");
                    }
                    Step::Replace(ids) => {
                        let earlier = data.clone();
                        data = law_data(&distinct(ids));
                        prop_assert!(!data.extends(&earlier));
                        state.retain_rows(&data);
                        let present: BTreeSet<String> =
                            (0..data.len()).map(|row| data.id(row).to_string()).collect();
                        let mut want = before.clone();
                        want.selection = Rc::new(before.selection.intersection(&present).cloned().collect());
                        want.anchor = before.anchor.clone().filter(|id| present.contains(id));
                        want.cursor = before.cursor.clone().filter(|id| present.contains(id));
                        want.tag_focus = before.tag_focus.clone().filter(|(row, _)| present.contains(row));
                        prop_assert_eq!(&state, &want, "replacing the rows");
                    }
                    Step::Swap(ids) => data = law_data(&distinct(ids)),
                }
                let view = View::build(&columns, &data, &state, BuildOptions::default());
                let every = !view.rows.is_empty() && view.ids(&data).all(|id| state.selection.contains(id));
                prop_assert_eq!(state.all_selected(&view, &data), every, "after {:?}", step);
            }
        }
    }
}
