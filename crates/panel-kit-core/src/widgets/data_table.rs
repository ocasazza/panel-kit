//! The one table component's state: sort, filters, column layout, grouping,
//! density, selection, cursor, window, and the tag ribbon.
//!
//! Ported from the board's LiveView data table (`ImpAgentWeb.Components.
//! DataTable.State`), which is the behaviour this replaces: multi-column sort
//! with ranks, a quick filter plus per-column filters, user column order,
//! visibility, widths, grouping with collapsed groups, density, single/multi
//! selection with an anchor for range select, a keyboard cursor, a virtual
//! window with overscan, and the tag filter with its ribbon state.
//!
//! Everything is pure and renderer-neutral: the painters read it, a host owns
//! it, and persistence is a JSON value a URL, localStorage or a server can
//! carry.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::collections::BTreeSet;

use serde::Deserialize;
use serde::Serialize;

/// Typed per-cell sort key. Rows are sorted by key, never by rendered markup.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum SortKey {
    /// Case-insensitive text (store it lowercased; see [`SortKey::text`]).
    Text(String),
    /// Numeric value (counts, scores, epoch timestamps).
    Num(f64),
    /// No value: always sorts after present values, in either direction.
    Missing,
}

impl SortKey {
    /// Lowercased text key.
    pub fn text(value: impl AsRef<str>) -> Self {
        SortKey::Text(value.as_ref().to_lowercase())
    }

    /// Numeric key; non-finite values become [`SortKey::Missing`].
    pub fn num(value: f64) -> Self {
        if value.is_finite() {
            SortKey::Num(value)
        } else {
            SortKey::Missing
        }
    }

    /// Optional numeric key.
    pub fn opt_num(value: Option<f64>) -> Self {
        value.map(SortKey::num).unwrap_or(SortKey::Missing)
    }

    /// Ascending order among present values; numbers before text.
    fn order(&self, other: &SortKey) -> Ordering {
        match (self, other) {
            (SortKey::Num(a), SortKey::Num(b)) => a.partial_cmp(b).unwrap_or(Ordering::Equal),
            (SortKey::Text(a), SortKey::Text(b)) => a.cmp(b),
            (SortKey::Num(_), SortKey::Text(_)) => Ordering::Less,
            (SortKey::Text(_), SortKey::Num(_)) => Ordering::Greater,
            (SortKey::Missing, SortKey::Missing) => Ordering::Equal,
            (SortKey::Missing, _) => Ordering::Greater,
            (_, SortKey::Missing) => Ordering::Less,
        }
    }
}

/// Sort direction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortDir {
    /// Smallest / earliest / A first.
    Asc,
    /// Largest / latest / Z first.
    Desc,
}

/// Active sort: a column id plus direction.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SortSpec {
    /// Stable column id (see [`DataColumnSpec::id`]).
    pub column: String,
    /// Direction.
    pub dir: SortDir,
}

/// Column declaration for the table.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataColumnSpec {
    /// Stable id used for sort state, filters and visibility persistence.
    pub id: String,
    /// Header text.
    pub title: String,
    /// Whether clicking the header sorts by this column.
    pub sortable: bool,
    /// Whether the user may hide this column.
    pub hideable: bool,
    /// Hidden until the user enables it.
    pub hidden_by_default: bool,
}

impl DataColumnSpec {
    /// A sortable, hideable, initially visible column.
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            sortable: true,
            hideable: true,
            hidden_by_default: false,
        }
    }

    /// Column that cannot be hidden (typically the identity column).
    pub fn pinned(mut self) -> Self {
        self.hideable = false;
        self
    }

    /// Column that does not sort (actions, checkboxes).
    pub fn unsorted(mut self) -> Self {
        self.sortable = false;
        self
    }

    /// Column that starts hidden.
    pub fn hidden(mut self) -> Self {
        self.hidden_by_default = true;
        self
    }
}

/// Row padding and type scale.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Density {
    /// Roomier padding and type.
    #[default]
    Normal,
    /// Compact padding for a long ledger.
    Compact,
}

impl Density {
    /// Pixel height of one row at this density; the window's spacers are sized
    /// by it until the client measures.
    pub fn row_height(self) -> f64 {
        match self {
            Density::Normal => 28.0,
            Density::Compact => 22.0,
        }
    }
}

/// How many rows a table may select.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Selectable {
    /// No row can be selected (the default: a plain listing).
    #[default]
    None,
    /// One row at a time.
    Single,
    /// Any number, with shift for a range.
    Multi,
}

/// How the table renders its rows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewMode {
    /// One grid row per record: the ledger.
    #[default]
    Rows,
    /// One compact card per record: a single line, chip-shaped, for a narrow
    /// column or a board.
    CompactCards,
    /// One detail card per record: the title, its meta line, and its tags.
    DetailCards,
}

impl ViewMode {
    /// The next mode in the toggle's cycle: rows, compact cards, detail cards.
    pub fn next(self) -> ViewMode {
        match self {
            ViewMode::Rows => ViewMode::CompactCards,
            ViewMode::CompactCards => ViewMode::DetailCards,
            ViewMode::DetailCards => ViewMode::Rows,
        }
    }

    /// A label for the toggle.
    pub fn label(self) -> &'static str {
        match self {
            ViewMode::Rows => "rows",
            ViewMode::CompactCards => "cards",
            ViewMode::DetailCards => "detail",
        }
    }
}

/// Whether a row must carry one of the filtered tags or all of them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TagMode {
    /// A row passes when it carries any filtered tag.
    #[default]
    Any,
    /// A row passes when it carries every filtered tag.
    All,
}

/// The virtual window: which slice of rows the client keeps mounted.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct TableWindow {
    /// Index into the visible rows of the first one mounted.
    pub start: usize,
    /// How many rows stay mounted.
    pub count: usize,
    /// Measured row height, when the client has reported one.
    pub row_height: Option<f64>,
}

impl Default for TableWindow {
    fn default() -> Self {
        TableWindow {
            start: 0,
            count: 80,
            row_height: None,
        }
    }
}

/// Per-column filter. A text filter matches the column's own key, a range
/// matches its numeric key, and a tag filter matches its text key against any
/// listed tag.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum ColumnFilter {
    /// Lowercased text the column's key must contain.
    Text(String),
    /// Inclusive numeric range; `None` is unbounded on that side.
    Range {
        /// Lower bound.
        min: Option<f64>,
        /// Upper bound.
        max: Option<f64>,
    },
    /// Column key must equal one of these tags.
    Tags(Vec<String>),
}

/// The defaults a table declares; `reset` returns to them.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TableDefaults {
    /// Initial sort.
    pub sort: Vec<SortSpec>,
    /// Initial density.
    pub density: Density,
    /// Initial grouping column.
    pub group_by: Option<String>,
    /// Initial selection mode.
    pub selectable: Selectable,
    /// Initial view mode.
    pub view: ViewMode,
    /// Columns hidden on top of those declared hidden.
    pub hidden: BTreeSet<String>,
}

/// One table's whole user-facing state.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TableState {
    /// Sort keys in priority order.
    #[serde(default)]
    pub sort: Vec<SortSpec>,
    /// Per-column filters, keyed by column id.
    #[serde(default)]
    pub filters: BTreeMap<String, ColumnFilter>,
    /// The quick filter: whitespace-separated terms, every one must match.
    #[serde(default, skip_serializing)]
    pub quick: String,
    /// Whether the per-column filter row is shown.
    #[serde(default)]
    pub filters_open: bool,
    /// Column order.
    #[serde(default)]
    pub order: Vec<String>,
    /// Column ids the user hid.
    #[serde(default)]
    pub hidden: BTreeSet<String>,
    /// Column ids explicitly shown that are hidden by default.
    #[serde(default)]
    pub shown: BTreeSet<String>,
    /// Measured column widths, in pixels.
    #[serde(default)]
    pub widths: BTreeMap<String, u32>,
    /// The column rows are grouped by, if any.
    #[serde(default)]
    pub group_by: Option<String>,
    /// Group keys the user collapsed.
    #[serde(default)]
    pub collapsed: BTreeSet<String>,
    /// Row padding and type scale.
    #[serde(default)]
    pub density: Density,
    /// Rows, compact cards, or detail cards.
    #[serde(default)]
    pub view: ViewMode,
    /// How many rows may be selected.
    #[serde(default)]
    pub selectable: Selectable,
    /// Focused row id: the keyboard's anchor and the painter's outline.
    #[serde(default)]
    pub cursor: Option<String>,
    /// Selected row ids.
    #[serde(default)]
    pub selection: BTreeSet<String>,
    /// The row a shift-click or shift-arrow range grows from.
    #[serde(default)]
    pub anchor: Option<String>,
    /// The mounted window.
    #[serde(default)]
    pub window: TableWindow,
    /// The tag filter, the ribbon's guide.
    #[serde(default)]
    pub tag_filter: BTreeSet<String>,
    /// Whether a row must carry any filtered tag or all of them.
    #[serde(default)]
    pub tag_mode: TagMode,
    /// The tag the pointer is over: `(field, value)`, for the ribbon.
    #[serde(default)]
    pub tag_focus: Option<(String, String)>,
    /// Whether the tag ribbon shows every value or only the selected ones.
    #[serde(default)]
    pub tags_expand: bool,
    /// What `reset` returns to.
    #[serde(default)]
    pub defaults: TableDefaults,
}

/// Rows the window renders by default, before the client measures.
pub const WINDOW_COUNT: usize = 80;
/// Rows the window keeps rendered beyond each edge of what is on screen.
pub const OVERSCAN: usize = 20;
/// The most rows one window may mount.
pub const MAX_WINDOW: usize = 2_000;
/// The narrowest and widest a column may be dragged to, in pixels.
pub const MIN_WIDTH: u32 = 40;
/// The widest a column may be dragged to, in pixels.
pub const MAX_WIDTH: u32 = 4_000;
/// The longest quick filter kept.
pub const QUICK_LIMIT: usize = 200;
/// The state payload version.
pub const STATE_VERSION: u32 = 1;

impl Default for TableState {
    fn default() -> Self {
        TableState::new(&[], TableDefaults::default())
    }
}

impl TableState {
    /// A fresh state for `columns`, on `defaults`.
    pub fn new(columns: &[DataColumnSpec], defaults: TableDefaults) -> Self {
        let order = columns.iter().map(|c| c.id.clone()).collect();
        let hidden = defaults.hidden.clone();
        TableState {
            sort: defaults.sort.clone(),
            filters: BTreeMap::new(),
            quick: String::new(),
            filters_open: false,
            order,
            hidden,
            shown: BTreeSet::new(),
            widths: BTreeMap::new(),
            group_by: defaults.group_by.clone(),
            collapsed: BTreeSet::new(),
            density: defaults.density,
            view: defaults.view,
            selectable: defaults.selectable,
            cursor: None,
            selection: BTreeSet::new(),
            anchor: None,
            window: TableWindow::default(),
            tag_filter: BTreeSet::new(),
            tag_mode: TagMode::default(),
            tag_focus: None,
            tags_expand: false,
            defaults,
        }
    }

    /// A state with one sort key and everything else at its zero value.
    pub fn sorted(column: impl Into<String>, dir: SortDir) -> Self {
        TableState {
            sort: vec![SortSpec {
                column: column.into(),
                dir,
            }],
            ..TableState::new(&[], TableDefaults::default())
        }
    }

    /// Drop state that names a column this table no longer has: a column that
    /// left takes its sort key, filter, width and group with it.
    pub fn sync(&mut self, columns: &[DataColumnSpec]) {
        let known: BTreeSet<&str> = columns.iter().map(|c| c.id.as_str()).collect();
        self.sort.retain(|key| known.contains(key.column.as_str()));
        self.filters.retain(|id, _| known.contains(id.as_str()));
        self.widths.retain(|id, _| known.contains(id.as_str()));
        self.order.retain(|id| known.contains(id.as_str()));
        self.hidden.retain(|id| known.contains(id.as_str()));
        self.shown.retain(|id| known.contains(id.as_str()));
        for column in columns {
            if !self.order.contains(&column.id) {
                self.order.push(column.id.clone());
            }
        }
        if let Some(group) = &self.group_by {
            if !known.contains(group.as_str()) {
                self.group_by = None;
            }
        }
    }

    /// The column at its place in the user's order.
    pub fn columns<'a>(&self, columns: &'a [DataColumnSpec]) -> Vec<&'a DataColumnSpec> {
        let mut out: Vec<&DataColumnSpec> = self
            .order
            .iter()
            .filter_map(|id| columns.iter().find(|c| &c.id == id))
            .collect();
        for column in columns {
            if !self.order.contains(&column.id) {
                out.push(column);
            }
        }
        out
    }

    /// Whether `column` is currently visible.
    pub fn is_visible(&self, column: &DataColumnSpec) -> bool {
        if !column.hideable {
            return true;
        }
        if column.hidden_by_default {
            self.shown.contains(&column.id)
        } else {
            !self.hidden.contains(&column.id)
        }
    }

    /// Flip one column's visibility. Refuses to hide the last visible column
    /// or a pinned one; returns whether anything changed.
    pub fn toggle_column(&mut self, columns: &[DataColumnSpec], id: &str) -> bool {
        let Some(column) = columns.iter().find(|c| c.id == id) else {
            return false;
        };
        if !column.hideable {
            return false;
        }
        let visible = self.is_visible(column);
        if visible && columns.iter().filter(|c| self.is_visible(c)).count() <= 1 {
            return false;
        }
        let set = if column.hidden_by_default {
            &mut self.shown
        } else {
            &mut self.hidden
        };
        if !set.remove(id) {
            set.insert(id.to_string());
        }
        true
    }

    /// Move a column one place earlier (`-1`) or later (`1`).
    pub fn move_column(&mut self, columns: &[DataColumnSpec], id: &str, delta: i32) -> bool {
        let order: Vec<String> = self.columns(columns).iter().map(|c| c.id.clone()).collect();
        let Some(from) = order.iter().position(|column| column == id) else {
            return false;
        };
        let to = (from as i32 + delta).clamp(0, order.len() as i32 - 1) as usize;
        if from == to {
            return false;
        }
        let mut next = order;
        let moved = next.remove(from);
        next.insert(to, moved);
        self.order = next;
        true
    }

    /// Record a column's dragged width, clamped to what a column may be.
    pub fn resize(&mut self, id: &str, width: u32) -> bool {
        let width = width.clamp(MIN_WIDTH, MAX_WIDTH);
        if self.widths.get(id) == Some(&width) {
            return false;
        }
        self.widths.insert(id.to_string(), width);
        true
    }

    /// The width asked for, if the user dragged this column.
    pub fn width(&self, id: &str) -> Option<u32> {
        self.widths.get(id).copied()
    }

    /// Sort by a column: a new column sorts ascending, the primary column
    /// cycles ascending, descending, off. Unsortable columns are ignored.
    pub fn click_header(&mut self, columns: &[DataColumnSpec], id: &str) {
        if !columns.iter().any(|c| c.id == id && c.sortable) {
            return;
        }
        match self.sort.first() {
            Some(spec) if spec.column == id && spec.dir == SortDir::Asc => self.sort[0].dir = SortDir::Desc,
            Some(spec) if spec.column == id && spec.dir == SortDir::Desc => {
                self.sort.remove(0);
            }
            _ => self.sort.insert(
                0,
                SortSpec {
                    column: id.to_string(),
                    dir: SortDir::Asc,
                },
            ),
        }
    }

    /// The sort direction of a column, or `None`.
    pub fn direction(&self, column: &str) -> Option<SortDir> {
        self.sort
            .iter()
            .find(|spec| spec.column == column)
            .map(|spec| spec.dir)
    }

    /// The column's place in a multi-column sort (1-based), or `None`.
    pub fn sort_rank(&self, column: &str) -> Option<usize> {
        self.sort
            .iter()
            .position(|spec| spec.column == column)
            .map(|index| index + 1)
    }

    /// Set one column's filter; an empty string clears it.
    pub fn set_filter(&mut self, column: &str, filter: ColumnFilter) {
        self.filters.insert(column.to_string(), filter);
    }

    /// Clear one column's filter.
    pub fn clear_filter(&mut self, column: &str) {
        self.filters.remove(column);
    }

    /// Clear every filter, per-column and quick.
    pub fn clear_filters(&mut self) {
        self.filters.clear();
        self.quick.clear();
    }

    /// Set the quick filter, clamped to the length a URL can carry.
    pub fn set_quick(&mut self, quick: impl Into<String>) {
        let quick = quick.into();
        self.quick = quick.chars().take(QUICK_LIMIT).collect();
    }

    /// Whether any filter narrows the rows.
    pub fn filtering(&self) -> bool {
        !self.quick.trim().is_empty()
            || !self.filters.is_empty()
            || !self.tag_filter.is_empty()
    }

    /// Group rows by a column, or clear the grouping with `None`.
    pub fn set_group(&mut self, group_by: Option<&str>) {
        self.group_by = group_by.map(str::to_string);
    }

    /// Collapse or expand one group.
    pub fn toggle_group(&mut self, group: &str) -> bool {
        if !self.collapsed.remove(group) {
            self.collapsed.insert(group.to_string());
            return true;
        }
        true
    }

    /// Whether the group's rows are hidden.
    pub fn is_collapsed(&self, group: &str) -> bool {
        self.collapsed.contains(group)
    }

    /// Set the density.
    pub fn set_density(&mut self, density: Density) {
        self.density = density;
    }

    /// Set how rows are rendered.
    pub fn set_view(&mut self, view: ViewMode) {
        self.view = view;
    }

    /// Move the cursor to a row id, if it is in view.
    pub fn set_cursor(&mut self, id: Option<&str>, ids: &[String]) {
        match id {
            Some(id) if ids.iter().any(|visible| visible == id) => {
                self.cursor = Some(id.to_string())
            }
            Some(_) => {}
            None => self.cursor = None,
        }
    }

    /// Whether the row holds the cursor: the painter outlines it.
    pub fn is_cursor(&self, id: &str) -> bool {
        self.cursor.as_deref() == Some(id)
    }

    /// Whether the row is painted as selected.
    pub fn is_selected(&self, id: &str) -> bool {
        self.selection.contains(id)
    }

    /// Select one row: `toggle` flips it, `on` adds it, `off` removes it, and
    /// `range` grows from the anchor through `ids`.
    pub fn select(&mut self, id: &str, mode: SelectMode, ids: &[String]) {
        match self.selectable {
            Selectable::None => {}
            Selectable::Single => {
                self.selection.clear();
                self.selection.insert(id.to_string());
                self.anchor = Some(id.to_string());
            }
            Selectable::Multi => match mode {
                SelectMode::Toggle => {
                    if !self.selection.remove(id) {
                        self.selection.insert(id.to_string());
                    }
                    self.anchor = Some(id.to_string());
                }
                SelectMode::On => {
                    self.selection.insert(id.to_string());
                    self.anchor = Some(id.to_string());
                }
                SelectMode::Off => {
                    self.selection.remove(id);
                    self.anchor = Some(id.to_string());
                }
                SelectMode::Range => self.select_range(id, ids),
            },
        }
    }

    /// Range grows from the anchor and leaves it in place, so a shrinking
    /// shift-click shrinks the range instead of inverting it.
    fn select_range(&mut self, id: &str, ids: &[String]) {
        let anchor = self.anchor.clone().unwrap_or_else(|| id.to_string());
        self.anchor = Some(anchor.clone());
        let from = ids.iter().position(|visible| visible == &anchor);
        let to = ids.iter().position(|visible| visible == id);
        match (from, to) {
            (Some(from), Some(to)) => {
                // The range is the selection: a shift-click that moves back
                // shrinks it rather than adding a second span.
                let (start, end) = if from <= to { (from, to) } else { (to, from) };
                self.selection = ids[start..=end].iter().cloned().collect();
            }
            _ => {
                self.selection = [id.to_string()].into_iter().collect();
            }
        }
    }

    /// Select every visible row, or clear them when all are selected.
    pub fn select_all(&mut self, ids: &[String]) {
        if self.selectable != Selectable::Multi {
            return;
        }
        let all = ids.iter().all(|id| self.selection.contains(id));
        if all && !ids.is_empty() {
            for id in ids {
                self.selection.remove(id);
            }
        } else {
            for id in ids {
                self.selection.insert(id.clone());
            }
        }
    }

    /// Clear the selection and the range anchor.
    pub fn clear_selection(&mut self) {
        self.selection.clear();
        self.anchor = None;
    }

    /// Add or remove one tag from the tag filter.
    pub fn toggle_tag(&mut self, tag: &str) {
        if !self.tag_filter.remove(tag) {
            self.tag_filter.insert(tag.to_string());
        }
    }

    /// Which tags the rows carry, in first-seen order, with their counts: what
    /// a ribbon lists.
    pub fn tag_counts(rows: &[RowKeys<'_>]) -> Vec<(String, usize)> {
        let mut counts: Vec<(String, usize)> = Vec::new();
        for row in rows {
            for tag in row.tags {
                match counts.iter_mut().find(|(name, _)| name == tag) {
                    Some((_, count)) => *count += 1,
                    None => counts.push((tag.clone(), 1)),
                }
            }
        }
        counts
    }

    /// Set which slice of rows the client keeps mounted.
    pub fn set_window(&mut self, start: usize, count: usize, row_height: Option<f64>) {
        self.window = TableWindow {
            start,
            count: count.clamp(1, MAX_WINDOW),
            row_height: row_height.filter(|height| *height > 0.0).or(self.window.row_height),
        };
    }

    /// The rows the painter mounts, and the spacer heights around them.
    pub fn mounted(&self, visible_len: usize) -> Mounted {
        let count = self.window.count.min(visible_len.max(1));
        let start = self.window.start.min(visible_len.saturating_sub(count));
        Mounted {
            start,
            end: (start + count).min(visible_len),
            before: start,
            after: visible_len.saturating_sub(start + count),
            row_height: self.window.row_height.unwrap_or(self.density.row_height()),
        }
    }

    /// A keyboard intent over the visible rows.
    pub fn key(&mut self, key: TableKey, ids: &[String], page: usize) {
        if ids.is_empty() {
            return;
        }
        let page = page.clamp(1, 1_000);
        let last = ids.len() - 1;
        let at = self
            .cursor
            .as_ref()
            .and_then(|cursor| ids.iter().position(|id| id == cursor));
        let to = match (&key, at) {
            (TableKey::ExtendDown, None) => 0,
            (TableKey::ExtendDown, Some(at)) => (at + 1).min(last),
            (TableKey::ExtendUp, None) => 0,
            (TableKey::ExtendUp, Some(at)) => at.saturating_sub(1),
            (TableKey::Extend(_), _) => at.unwrap_or(0),
            (TableKey::Toggle, _) => at.unwrap_or(0),
            (TableKey::ClearSelection, _) => at.unwrap_or(0),
            (TableKey::SelectAll, _) => 0,
            (TableKey::Down, None) => 0,
            (TableKey::Down, Some(at)) => (at + 1).min(last),
            (TableKey::Up, None) => 0,
            (TableKey::Up, Some(at)) => at.saturating_sub(1),
            (TableKey::Home, _) => 0,
            (TableKey::End, _) => last,
            (TableKey::PageDown, None) => page.min(last),
            (TableKey::PageDown, Some(at)) => (at + page).min(last),
            (TableKey::PageUp, None) => 0,
            (TableKey::PageUp, Some(at)) => at.saturating_sub(page),
        };
        let id = ids[to].clone();
        self.cursor = Some(id.clone());
        let extending = matches!(
            key,
            TableKey::ExtendUp | TableKey::ExtendDown | TableKey::Extend(_)
        );
        if extending && self.selectable == Selectable::Multi {
            if self.anchor.is_none() {
                self.anchor = self.cursor.clone();
            }
            self.select(&id, SelectMode::Range, ids);
        }
        self.follow(&id, ids);
    }

    /// Keep the cursor inside the mounted window, so the row exists to focus.
    fn follow(&mut self, id: &str, ids: &[String]) {
        let Some(at) = ids.iter().position(|visible| visible == id) else {
            return;
        };
        let count = self.window.count;
        let edge = OVERSCAN.min(count / 4);
        let start = self.window.start;
        if at < start + edge || at >= start + count - edge {
            self.window.start = at.saturating_sub(count / 2);
        }
    }

    /// What a host keeps per table id.
    pub fn persisted(&self) -> PersistedTable {
        PersistedTable {
            v: STATE_VERSION,
            sort: self.sort.clone(),
            filters: self.filters.clone(),
            quick: self.quick.clone(),
            filters_open: self.filters_open,
            order: self.order.clone(),
            hidden: self.hidden.iter().cloned().collect(),
            shown: self.shown.iter().cloned().collect(),
            widths: self.widths.clone(),
            group_by: self.group_by.clone(),
            density: self.density,
            view: self.view,
        }
    }

    /// Apply a `persisted` payload. Untrusted: a field that does not fit the
    /// columns this table has now keeps its current value.
    pub fn restore(&mut self, payload: &PersistedTable, columns: &[DataColumnSpec]) {
        let known: BTreeSet<&str> = columns.iter().map(|c| c.id.as_str()).collect();
        if payload.sort.iter().all(|spec| known.contains(spec.column.as_str())) {
            self.sort = payload.sort.clone();
        }
        if payload.filters.keys().all(|id| known.contains(id.as_str())) {
            self.filters = payload.filters.clone();
        }
        self.set_quick(payload.quick.clone());
        self.filters_open = payload.filters_open;
        if !payload.order.is_empty() && payload.order.iter().all(|id| known.contains(id.as_str())) {
            self.order = payload.order.clone();
        }
        let kept = |ids: &[String]| -> BTreeSet<String> {
            ids.iter()
                .filter(|id| known.contains(id.as_str()))
                .cloned()
                .collect()
        };
        self.hidden = kept(&payload.hidden);
        self.shown = kept(&payload.shown);
        self.widths = payload
            .widths
            .iter()
            .filter(|(id, _)| known.contains(id.as_str()))
            .map(|(id, width)| (id.clone(), *width))
            .collect();
        match payload.group_by.as_deref() {
            Some(id) if known.contains(id) => self.group_by = Some(id.to_string()),
            Some(_) => {}
            None => self.group_by = None,
        }
        self.density = payload.density;
        self.view = payload.view;
    }

    /// Back to the declared defaults.
    pub fn reset(&mut self, columns: &[DataColumnSpec]) {
        let mut fresh = TableState::new(columns, self.defaults.clone());
        fresh.window = self.window;
        *self = fresh;
    }
}

/// How a click selects.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectMode {
    /// Flip the row.
    Toggle,
    /// Select the row.
    On,
    /// Deselect the row.
    Off,
    /// Select from the anchor through the row.
    Range,
}

/// A keyboard intent, translated by the host from its own event names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TableKey {
    /// One row down.
    Down,
    /// One row up.
    Up,
    /// The first row.
    Home,
    /// The last row.
    End,
    /// A page down.
    PageDown,
    /// A page up.
    PageUp,
    /// Extend the selection down (shift + Down).
    ExtendDown,
    /// Extend the selection up (shift + Up).
    ExtendUp,
    /// Extend the selection to a row (shift + a row id).
    Extend(String),
    /// Toggle the cursor row's selection.
    Toggle,
    /// Clear the selection.
    ClearSelection,
    /// Select every visible row.
    SelectAll,
}

/// Which rows the painter mounts, and how many it leaves out.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mounted {
    /// First mounted row, as an index into the visible rows.
    pub start: usize,
    /// One past the last mounted row.
    pub end: usize,
    /// Rows above the window (the top spacer's height is this times a row).
    pub before: usize,
    /// Rows below the window.
    pub after: usize,
    /// The row height the spacers are sized by, measured or from density.
    pub row_height: f64,
}

/// One row's query-relevant data: its id, a lowercase search haystack, its
/// tags, and one sort key per declared column (in column-declaration order).
pub struct RowKeys<'a> {
    /// Stable row identity: cursor and selection survive a re-sort by id.
    pub id: &'a str,
    /// Lowercased text the quick filter matches against.
    pub search: &'a str,
    /// The row's tags.
    pub tags: &'a [String],
    /// Sort keys, parallel to the declared column list.
    pub keys: &'a [SortKey],
}

/// Indices of rows that pass every filter, in sorted order. Stable: rows with
/// equal keys keep their input order.
pub fn visible_rows(
    columns: &[DataColumnSpec],
    rows: &[RowKeys<'_>],
    state: &TableState,
) -> Vec<usize> {
    let terms: Vec<String> = state
        .quick
        .split_whitespace()
        .map(str::to_lowercase)
        .collect();

    let mut out: Vec<usize> = rows
        .iter()
        .enumerate()
        .filter(|(_, row)| terms.iter().all(|term| row.search.contains(term.as_str())))
        .filter(|(_, row)| passes_columns(columns, row, state))
        .filter(|(_, row)| passes_tags(row, state))
        .map(|(index, _)| index)
        .collect();

    // Keys apply in priority order, the last one first so the first decides.
    for spec in state.sort.iter().rev() {
        let Some(column) = columns
            .iter()
            .position(|c| c.id == spec.column && c.sortable)
        else {
            continue;
        };
        let dir = spec.dir;
        let missing = SortKey::Missing;
        out.sort_by(|&a, &b| {
            let left = rows[a].keys.get(column).unwrap_or(&missing);
            let right = rows[b].keys.get(column).unwrap_or(&missing);
            match (left, right) {
                (SortKey::Missing, SortKey::Missing) => Ordering::Equal,
                (SortKey::Missing, _) => Ordering::Greater,
                (_, SortKey::Missing) => Ordering::Less,
                _ if dir == SortDir::Desc => right.order(left),
                _ => left.order(right),
            }
        });
    }
    out
}

fn passes_columns(columns: &[DataColumnSpec], row: &RowKeys<'_>, state: &TableState) -> bool {
    state.filters.iter().all(|(id, filter)| {
        let Some(index) = columns.iter().position(|c| &c.id == id) else {
            return true;
        };
        let key = row.keys.get(index).unwrap_or(&SortKey::Missing);
        match (filter, key) {
            (ColumnFilter::Text(text), SortKey::Text(value)) => value.contains(&text.to_lowercase()),
            (ColumnFilter::Text(text), SortKey::Num(value)) => {
                format!("{value}").contains(&text.to_lowercase())
            }
            (ColumnFilter::Range { min, max }, SortKey::Num(value)) => {
                min.is_none_or(|min| *value >= min) && max.is_none_or(|max| *value <= max)
            }
            (ColumnFilter::Tags(tags), SortKey::Text(value)) => {
                tags.iter().any(|tag| tag.to_lowercase() == *value)
            }
            // A filter on a column the row has no value for excludes it: the
            // operator asked for rows that have that value.
            _ => false,
        }
    })
}

fn passes_tags(row: &RowKeys<'_>, state: &TableState) -> bool {
    match state.tag_filter.len() {
        0 => true,
        _ if state.tag_mode == TagMode::All => state
            .tag_filter
            .iter()
            .all(|tag| row.tags.contains(tag)),
        _ => state.tag_filter.iter().any(|tag| row.tags.contains(tag)),
    }
}

/// Group visible rows by a column's text key, in visible order: the painter's
/// group headers, each with the rows under it.
pub fn grouped(
    columns: &[DataColumnSpec],
    rows: &[RowKeys<'_>],
    state: &TableState,
    visible: &[usize],
) -> Vec<(String, Vec<usize>)> {
    let Some(group_by) = state.group_by.as_ref() else {
        return vec![(String::new(), visible.to_vec())];
    };
    let Some(column) = columns.iter().position(|c| &c.id == group_by) else {
        return vec![(String::new(), visible.to_vec())];
    };
    let mut groups: Vec<(String, Vec<usize>)> = Vec::new();
    for index in visible {
        let label = match rows[*index].keys.get(column) {
            Some(SortKey::Text(text)) => text.clone(),
            Some(SortKey::Num(value)) => format!("{value}"),
            _ => "—".to_string(),
        };
        match groups.iter_mut().find(|(name, _)| name == &label) {
            Some((_, indices)) => indices.push(*index),
            None => groups.push((label, vec![*index])),
        }
    }
    groups
}

/// What a host keeps per table id: the state fields a URL, localStorage or a
/// server carries, typed so restoring costs no JSON juggling. The quick
/// filter's text is included because a reload should keep the view the reader
/// left; a host that wants it out clears it before persisting.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PersistedTable {
    /// Payload version.
    #[serde(default)]
    pub v: u32,
    /// Sort keys, in priority order.
    #[serde(default)]
    pub sort: Vec<SortSpec>,
    /// Per-column filters.
    #[serde(default)]
    pub filters: BTreeMap<String, ColumnFilter>,
    /// The quick filter's text.
    #[serde(default)]
    pub quick: String,
    /// Whether the filter row was open.
    #[serde(default)]
    pub filters_open: bool,
    /// Column order.
    #[serde(default)]
    pub order: Vec<String>,
    /// Hidden column ids.
    #[serde(default)]
    pub hidden: Vec<String>,
    /// Shown hidden-by-default column ids.
    #[serde(default)]
    pub shown: Vec<String>,
    /// Measured column widths.
    #[serde(default)]
    pub widths: BTreeMap<String, u32>,
    /// Grouping column.
    #[serde(default)]
    pub group_by: Option<String>,
    /// Density.
    #[serde(default)]
    pub density: Density,
    /// Rows, compact cards, or detail cards.
    #[serde(default)]
    pub view: ViewMode,
}

/// One interaction a painter raises; the host applies it to its own state.
#[derive(Clone, Debug, PartialEq)]
pub enum TableEvent {
    /// A header click: sort by that column.
    Sort(String),
    /// A column moved one place earlier (`-1`) or later (`1`).
    MoveColumn(String, i32),
    /// A column's dragged width, in pixels.
    Resize(String, u32),
    /// A column's visibility toggled.
    ToggleColumn(String),
    /// The quick filter's text.
    Quick(String),
    /// One column's filter text; empty clears it.
    Filter(String, String),
    /// The per-column filter row shown or hidden.
    FiltersOpen(bool),
    /// Every filter cleared.
    ClearFilters,
    /// Grouped by a column, or ungrouped.
    Group(Option<String>),
    /// A group collapsed or expanded.
    ToggleGroup(String),
    /// Row padding.
    Density(Density),
    /// Rows, compact cards, or detail cards.
    View(ViewMode),
    /// A row clicked: `shift` extends from the anchor.
    Select {
        /// The row clicked.
        id: String,
        /// Whether shift was held: extend from the anchor instead of flipping.
        shift: bool,
    },
    /// Every visible row selected, or none.
    SelectAll,
    /// A keyboard intent.
    Key(TableKey),
    /// The client's mounted window.
    Window {
        /// First mounted row.
        start: usize,
        /// How many rows stay mounted.
        count: usize,
        /// Measured row height, when the client reported one.
        row_height: Option<f64>,
    },
    /// A tag added to or removed from the ribbon's filter.
    Tag(String),
    /// Whether a row must carry any or all filtered tags.
    TagMode(TagMode),
    /// The ribbon's hovered tag.
    TagFocus(Option<(String, String)>),
    /// Whether the ribbon shows every value.
    TagsExpand(bool),
    /// The cursor moved to a row id.
    Cursor(String),
    /// The state restored from a persisted payload.
    Restore(PersistedTable),
    /// Back to the table's defaults.
    Reset,
}

impl TableEvent {
    /// Apply this event. `ids` are the visible rows this frame, in order.
    pub fn apply(&self, state: &mut TableState, columns: &[DataColumnSpec], ids: &[String]) {
        match self {
            TableEvent::Sort(column) => state.click_header(columns, column),
            TableEvent::MoveColumn(column, delta) => {
                state.move_column(columns, column, *delta);
            }
            TableEvent::Resize(column, width) => {
                state.resize(column, *width);
            }
            TableEvent::ToggleColumn(column) => {
                state.toggle_column(columns, column);
            }
            TableEvent::Quick(text) => state.set_quick(text),
            TableEvent::Filter(column, text) => {
                if text.trim().is_empty() {
                    state.clear_filter(column);
                } else {
                    state.set_filter(column, ColumnFilter::Text(text.to_lowercase()));
                }
            }
            TableEvent::FiltersOpen(open) => state.filters_open = *open,
            TableEvent::ClearFilters => state.clear_filters(),
            TableEvent::Group(column) => state.set_group(column.as_deref()),
            TableEvent::ToggleGroup(group) => {
                state.toggle_group(group);
            }
            TableEvent::Density(density) => state.set_density(*density),
            TableEvent::View(view) => state.set_view(*view),
            TableEvent::Select { id, shift } => {
                let mode = if *shift && state.selectable == Selectable::Multi {
                    SelectMode::Range
                } else {
                    SelectMode::Toggle
                };
                state.select(id, mode, ids);
            }
            TableEvent::SelectAll => state.select_all(ids),
            TableEvent::Key(key) => {
                let key = match key {
                    TableKey::ExtendDown => TableKey::Down,
                    TableKey::ExtendUp => TableKey::Up,
                    other => other.clone(),
                };
                match key {
                    TableKey::Toggle => {
                        if let Some(cursor) = state.cursor.clone() {
                            state.select(&cursor, SelectMode::Toggle, ids);
                        }
                    }
                    TableKey::ClearSelection => state.clear_selection(),
                    TableKey::SelectAll => state.select_all(ids),
                    TableKey::Extend(id) => state.select(&id, SelectMode::Range, ids),
                    other => state.key(other, ids, 20),
                }
            }
            TableEvent::Window {
                start,
                count,
                row_height,
            } => state.set_window(*start, *count, *row_height),
            TableEvent::Tag(tag) => state.toggle_tag(tag),
            TableEvent::TagMode(mode) => state.tag_mode = *mode,
            TableEvent::TagFocus(focus) => state.tag_focus = focus.clone(),
            TableEvent::TagsExpand(expand) => state.tags_expand = *expand,
            TableEvent::Cursor(id) => state.set_cursor(Some(id), ids),
            TableEvent::Restore(payload) => state.restore(payload, columns),
            TableEvent::Reset => state.reset(columns),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cols() -> Vec<DataColumnSpec> {
        vec![
            DataColumnSpec::new("name", "Name").pinned(),
            DataColumnSpec::new("created", "Created"),
            DataColumnSpec::new("gpu", "GPU-h").hidden(),
            DataColumnSpec::new("act", "").unsorted(),
        ]
    }

    fn keys(name: &str, created: Option<f64>) -> (String, Vec<SortKey>) {
        (
            name.to_lowercase(),
            vec![
                SortKey::text(name),
                SortKey::opt_num(created),
                SortKey::Missing,
                SortKey::Missing,
            ],
        )
    }

    fn no_tags() -> Vec<String> {
        Vec::new()
    }

    fn run(data: &[(String, Vec<SortKey>)], state: &TableState) -> Vec<usize> {
        let tags = no_tags();
        let rows: Vec<RowKeys> = data
            .iter()
            .map(|(search, keys)| RowKeys {
                id: search,
                search,
                tags: &tags,
                keys,
            })
            .collect();
        visible_rows(&cols(), &rows, state)
    }

    fn state() -> TableState {
        TableState::new(&cols(), TableDefaults::default())
    }

    #[test]
    fn the_quick_filter_requires_every_term() {
        let data = [
            keys("humanoid-recover", None),
            keys("spider-recover", None),
            keys("spot-walk", None),
        ];
        let mut s = state();
        s.set_quick("RECOVER spi");
        assert_eq!(run(&data, &s), vec![1]);
    }

    #[test]
    fn missing_values_sort_last_in_both_directions() {
        let data = [
            keys("a", Some(900.0)),
            keys("b", None),
            keys("c", Some(10_000.0)),
        ];
        let mut s = state();
        s.sort = vec![SortSpec {
            column: "created".into(),
            dir: SortDir::Desc,
        }];
        assert_eq!(run(&data, &s), vec![2, 0, 1]);
        s.sort[0].dir = SortDir::Asc;
        assert_eq!(run(&data, &s), vec![0, 2, 1]);
    }

    #[test]
    fn a_second_key_breaks_ties_and_ranks_are_one_based() {
        let data = [
            (String::from("b"), vec![SortKey::text("same"), SortKey::num(2.0)]),
            (String::from("a"), vec![SortKey::text("same"), SortKey::num(1.0)]),
            (String::from("c"), vec![SortKey::text("other"), SortKey::num(0.0)]),
        ];
        let mut s = state();
        s.sort = vec![
            SortSpec {
                column: "name".into(),
                dir: SortDir::Asc,
            },
            SortSpec {
                column: "created".into(),
                dir: SortDir::Asc,
            },
        ];
        assert_eq!(run(&data, &s), vec![2, 1, 0]);
        assert_eq!(s.sort_rank("name"), Some(1));
        assert_eq!(s.sort_rank("created"), Some(2));
        assert_eq!(s.direction("created"), Some(SortDir::Asc));
    }

    #[test]
    fn a_header_click_cycles_ascending_descending_off() {
        let c = cols();
        let mut s = state();
        s.click_header(&c, "created");
        assert_eq!(s.direction("created"), Some(SortDir::Asc));
        s.click_header(&c, "created");
        assert_eq!(s.direction("created"), Some(SortDir::Desc));
        s.click_header(&c, "created");
        assert_eq!(s.direction("created"), None);
        assert!(s.sort.is_empty());
        s.click_header(&c, "act");
        assert_eq!(s.direction("act"), None, "unsortable column is ignored");
    }

    #[test]
    fn visibility_respects_defaults_pins_and_last_column() {
        let c = cols();
        let mut s = state();
        assert!(!s.is_visible(&c[2]), "hidden-by-default column starts hidden");
        assert!(s.toggle_column(&c, "gpu"));
        assert!(s.is_visible(&c[2]));
        assert!(!s.toggle_column(&c, "name"), "pinned column cannot hide");

        let only = vec![DataColumnSpec::new("x", "X"), DataColumnSpec::new("y", "Y")];
        let mut s = TableState::new(&only, TableDefaults::default());
        assert!(s.toggle_column(&only, "x"));
        assert!(!s.toggle_column(&only, "y"), "last visible column cannot hide");
    }

    #[test]
    fn columns_follow_the_users_order_and_widths_clamp() {
        let c = cols();
        let mut s = state();
        assert_eq!(
            s.columns(&c).iter().map(|c| c.id.as_str()).collect::<Vec<_>>(),
            ["name", "created", "gpu", "act"]
        );
        assert!(s.move_column(&c, "created", -1));
        assert_eq!(s.columns(&c)[0].id, "created");
        assert!(s.resize("name", 9_999));
        assert_eq!(s.width("name"), Some(MAX_WIDTH));
    }

    #[test]
    fn a_column_filter_matches_text_range_and_excludes_missing() {
        let data = [
            keys("a", Some(900.0)),
            keys("b", Some(10.0)),
            keys("c", None),
        ];
        let mut s = state();
        s.set_filter(
            "created",
            ColumnFilter::Range {
                min: Some(100.0),
                max: None,
            },
        );
        assert_eq!(run(&data, &s), vec![0]);

        let mut s = state();
        s.set_filter("name", ColumnFilter::Text("b".into()));
        assert_eq!(run(&data, &s), vec![1]);
    }

    #[test]
    fn the_tag_filter_combines_by_mode() {
        let one: Vec<String> = vec!["p1".into(), "seat".into()];
        let two: Vec<String> = vec!["p1".into()];
        let all: Vec<SortKey> = vec![SortKey::text("x"), SortKey::Missing, SortKey::Missing, SortKey::Missing];
        let rows = vec![
            RowKeys { id: "one", search: "x", tags: &one, keys: &all },
            RowKeys { id: "two", search: "x", tags: &two, keys: &all },
        ];
        let mut s = state();
        s.toggle_tag("seat");
        assert_eq!(visible_rows(&cols(), &rows, &s), vec![0]);

        s.tag_mode = TagMode::All;
        s.toggle_tag("p1");
        assert_eq!(visible_rows(&cols(), &rows, &s), vec![0]);
    }

    #[test]
    fn selection_modes_range_from_the_anchor_and_select_all_toggles() {
        let ids: Vec<String> = ["a", "b", "c", "d"].iter().map(|s| s.to_string()).collect();
        let mut s = state();
        assert_eq!(s.selectable, Selectable::None);
        s.select("a", SelectMode::Toggle, &ids);
        assert!(s.selection.is_empty(), "a table nobody may select selects nothing");

        s.selectable = Selectable::Multi;
        s.select("b", SelectMode::Toggle, &ids);
        assert!(s.is_selected("b"));
        s.select("d", SelectMode::Range, &ids);
        assert_eq!(s.selection.len(), 3, "b..d");
        s.select("a", SelectMode::Range, &ids);
        assert_eq!(s.selection.len(), 2, "a..b");

        s.select_all(&ids);
        assert_eq!(s.selection.len(), 4);
        s.select_all(&ids);
        assert!(s.selection.is_empty());

        s.selectable = Selectable::Single;
        s.select("a", SelectMode::Toggle, &ids);
        s.select("b", SelectMode::Toggle, &ids);
        assert_eq!(s.selection.len(), 1);
        assert!(s.is_selected("b"));
    }

    #[test]
    fn keys_move_the_cursor_and_keep_it_inside_the_window() {
        let ids: Vec<String> = (0..100).map(|i| format!("r{i}")).collect();
        let mut s = state();
        s.set_window(0, 20, None);
        s.key(TableKey::Down, &ids, 20);
        assert_eq!(s.cursor.as_deref(), Some("r0"));
        s.key(TableKey::PageDown, &ids, 20);
        assert_eq!(s.cursor.as_deref(), Some("r20"));
        assert!(s.window.start >= 10, "the window follows the cursor");
        s.key(TableKey::End, &ids, 20);
        assert_eq!(s.cursor.as_deref(), Some("r99"));
        s.key(TableKey::Home, &ids, 20);
        assert_eq!(s.cursor.as_deref(), Some("r0"));
    }

    #[test]
    fn the_window_mounts_a_slice_and_counts_the_spacers() {
        let mut s = state();
        s.set_window(40, 20, Some(24.0));
        let mounted = s.mounted(100);
        assert_eq!((mounted.start, mounted.end), (40, 60));
        assert_eq!((mounted.before, mounted.after), (40, 40));

        s.set_window(90, 20, None);
        let mounted = s.mounted(100);
        assert_eq!((mounted.start, mounted.end), (80, 100), "clamped to the end");
        assert_eq!(mounted.row_height, 24.0, "the measured height stands");
    }

    #[test]
    fn grouping_labels_rows_by_the_group_columns_key() {
        let data = [
            keys("a", Some(1.0)),
            keys("b", Some(2.0)),
            keys("c", Some(1.0)),
        ];
        let tags = no_tags();
        let rows: Vec<RowKeys> = data
            .iter()
            .map(|(search, keys)| RowKeys {
                id: search,
                search,
                tags: &tags,
                keys,
            })
            .collect();
        let mut s = state();
        s.set_group(Some("name"));
        let visible = visible_rows(&cols(), &rows, &s);
        let groups = grouped(&cols(), &rows, &s, &visible);
        assert_eq!(groups.len(), 3, "one group per distinct name");
        s.toggle_group("b");
        assert!(s.is_collapsed("b"));
    }

    #[test]
    fn persisted_state_round_trips_and_restore_rejects_unknown_columns() {
        let c = cols();
        let mut s = state();
        s.click_header(&c, "created");
        s.toggle_column(&c, "gpu");
        s.resize("name", 160);
        s.set_group(Some("name"));
        s.set_quick("recover");

        let payload = s.persisted();
        let mut back = state();
        back.restore(&payload, &c);
        assert_eq!(back.sort, s.sort);
        assert_eq!(back.width("name"), Some(160));
        assert_eq!(back.group_by, s.group_by);
        assert_eq!(back.quick, "recover");

        let mut stranger = payload.clone();
        stranger.order = vec!["nope".into()];
        stranger.group_by = Some("gone".into());
        let mut guarded = state();
        guarded.restore(&stranger, &c);
        assert_eq!(guarded.order, state().order, "an unknown order keeps the old one");
        assert_eq!(guarded.group_by, None, "an unknown group clears");
    }

    #[test]
    fn the_view_cycles_and_persists_with_the_state() {
        let c = cols();
        let mut s = state();
        assert_eq!(s.view, ViewMode::Rows);
        s.set_view(s.view.next());
        assert_eq!(s.view, ViewMode::CompactCards);
        s.set_view(s.view.next());
        assert_eq!(s.view, ViewMode::DetailCards);
        s.set_view(s.view.next());
        assert_eq!(s.view, ViewMode::Rows);

        s.set_view(ViewMode::DetailCards);
        let payload = s.persisted();
        let mut back = state();
        back.restore(&payload, &c);
        assert_eq!(back.view, ViewMode::DetailCards);

        TableEvent::View(ViewMode::CompactCards).apply(&mut s, &c, &[]);
        assert_eq!(s.view, ViewMode::CompactCards);
        assert_eq!(ViewMode::CompactCards.label(), "cards");
    }

    #[test]
    fn events_apply_through_one_door() {
        let c = cols();
        let ids: Vec<String> = ["a", "b"].iter().map(|s| s.to_string()).collect();
        let mut s = state();
        s.selectable = Selectable::Multi;

        TableEvent::Sort("created".into()).apply(&mut s, &c, &ids);
        assert_eq!(s.direction("created"), Some(SortDir::Asc));
        TableEvent::Filter("name".into(), "api".into()).apply(&mut s, &c, &ids);
        assert!(s.filters.contains_key("name"));
        TableEvent::Filter("name".into(), String::new()).apply(&mut s, &c, &ids);
        assert!(!s.filters.contains_key("name"));
        TableEvent::Tag("p1".into()).apply(&mut s, &c, &ids);
        assert!(s.tag_filter.contains("p1"));
        TableEvent::Select {
            id: "a".into(),
            shift: false,
        }
        .apply(&mut s, &c, &ids);
        assert!(s.is_selected("a"));
        TableEvent::Density(Density::Compact).apply(&mut s, &c, &ids);
        assert_eq!(s.density, Density::Compact);
        TableEvent::Reset.apply(&mut s, &c, &ids);
        assert!(s.selection.is_empty(), "reset drops the selection too");
        assert_eq!(s.density, Density::Normal);
    }

    #[test]
    fn sync_drops_state_for_a_column_that_left() {
        let c = cols();
        let mut s = state();
        s.click_header(&c, "created");
        s.set_filter("created", ColumnFilter::Text("x".into()));
        s.sync(&["name".to_string(), "gpu".to_string(), "act".to_string()].iter().map(|id| DataColumnSpec::new(id.clone(), id.clone())).collect::<Vec<_>>());
        assert!(s.sort.is_empty());
        assert!(s.filters.is_empty());
    }
}
