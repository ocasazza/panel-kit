//! Semantic table model shared by web and terminal painters.

use serde::{Deserialize, Serialize};

use crate::badge::Rgb;

/// Horizontal text alignment for a semantic table column.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum TextAlign {
    /// Left-align text.
    Left,
    /// Center-align text.
    Center,
    /// Right-align text.
    Right,
}

/// Renderer-neutral column width request.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
pub enum ColumnWidth {
    /// Fixed width in renderer units.
    Fixed {
        /// Fixed width value.
        value: u16,
    },
    /// Flexible weighted width.
    Flex {
        /// Flexible width weight.
        weight: u16,
    },
}

/// One table column.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TableColumn {
    /// Stable column key.
    pub key: String,
    /// Display title.
    pub title: String,
    /// Requested width.
    pub width: ColumnWidth,
    /// Text alignment.
    pub align: TextAlign,
}

/// Semantic cell variants supported by authored tables.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum TableCell {
    /// Plain text cell.
    Text(String),
    /// Colored status label.
    Status {
        /// Status label.
        label: String,
        /// Status display color.
        color: Rgb,
    },
    /// Inline meter cell.
    Meter {
        /// Fill fraction.
        ratio: f64,
        /// Human-readable value text.
        text: String,
        /// Optional renderer-neutral meter color.
        color: Option<Rgb>,
    },
}

/// One table row.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TableRow {
    /// Stable identity: selection, focus and re-sorts survive a reload by id,
    /// never by position.
    pub id: String,
    /// Cells in column order.
    pub cells: Vec<TableCell>,
    /// Tags a tag filter matches; empty on a table nobody filters by tag.
    #[serde(default)]
    pub tags: Vec<String>,
}

/// Owned semantic table data.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TableModel {
    /// Column definitions.
    pub columns: Vec<TableColumn>,
    /// Table rows.
    pub rows: Vec<TableRow>,
}

/// How a table's rows are ordered, filtered and focused. Pure data: the
/// painter reads it, the host owns it, and `TableState::apply` is the only
/// mutator, so a keystroke is replayable.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TableState {
    /// Sort keys, applied in order: the first decides, the next breaks ties.
    #[serde(default)]
    pub sort: Vec<SortKey>,
    /// The active filter.
    #[serde(default)]
    pub filter: FilterState,
    /// Focused row id; keys move it, the painter marks it.
    #[serde(default)]
    pub focus: Option<String>,
    /// Selected row ids; a click or `ToggleSelected` maintains them.
    #[serde(default)]
    pub selected: Vec<String>,
    /// Row padding and type scale.
    #[serde(default)]
    pub density: Density,
    /// Index into the *ordered* rows of the first one painted.
    #[serde(default)]
    pub scroll: usize,
}

/// One column's sort, in the direction asked for.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SortKey {
    /// The `TableColumn::key` to sort on.
    pub column: String,
    /// Reverse order for this key.
    #[serde(default)]
    pub descending: bool,
}

/// A filter narrows rows by a text query over every cell, and by tags.
///
/// Tags come from `TableRow::tags`; a row with no tags is filtered out only
/// while a tag filter is active, which is what an operator asking "only this
/// tag" means.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FilterState {
    /// Text every painted row must contain in some cell.
    #[serde(default)]
    pub query: String,
    /// Tags every painted row must carry (per `tag_mode`).
    #[serde(default)]
    pub tags: Vec<String>,
    /// Whether a row must carry all filtered tags or any.
    #[serde(default)]
    pub tag_mode: TagMode,
}

/// How several filtered tags combine.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TagMode {
    /// A row passes when it carries every filtered tag.
    #[default]
    All,
    /// A row passes when it carries any filtered tag.
    Any,
}

/// Row padding and type scale.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Density {
    /// Roomier padding and type.
    #[default]
    Comfortable,
    /// Compact padding for a long ledger.
    Dense,
}

/// A keyboard/navigation intent the host translates a key into. Applying one
/// is pure and needs only the visible row ids and a page size.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TableCommand {
    /// Focus the first visible row.
    FocusFirst,
    /// Focus the last visible row.
    FocusLast,
    /// Focus the row above.
    Up,
    /// Focus the row below.
    Down,
    /// Move focus up by a page.
    PageUp(usize),
    /// Move focus down by a page.
    PageDown(usize),
    /// Add or remove the focused row from the selection.
    ToggleSelected,
    /// Select every visible row, or clear the selection when all are selected.
    ToggleSelectAll,
    /// Focus the row with this id, if the filter leaves it visible.
    Focus(String),
    /// Sort by a column: first press descending, second ascending, third
    /// clears that column's key.
    SortBy(String),
    /// Keep only rows carrying this tag, or drop it when it is present.
    ToggleTag(String),
}

/// Borrowed semantic table data for runtime providers: the ordered, filtered
/// window the painter draws, plus the counts an empty state needs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TableView<'a> {
    /// Borrowed column definitions.
    pub columns: &'a [TableColumn],
    /// The rows to paint, in order.
    pub rows: &'a [&'a TableRow],
    /// What the host asked for; the painter reads focus, selection, density.
    pub state: &'a TableState,
    /// Rows the model holds before filtering.
    pub total: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_model_carries_columns_and_tagged_cells() {
        let table = TableModel {
            columns: vec![TableColumn {
                key: "service".into(),
                title: "Service".into(),
                width: ColumnWidth::Flex { weight: 2 },
                align: TextAlign::Left,
            }],
            rows: vec![TableRow {
                id: "api".into(),
                tags: Vec::new(),
                cells: vec![TableCell::Text("api".into())],
            }],
        };

        assert_eq!(table.columns[0].key, "service");
        assert_eq!(table.rows[0].cells[0], TableCell::Text("api".into()));
    }
}

/// What the visible order is: the row indices in paint order.
///
/// Filling a caller-owned buffer keeps a per-episode repaint from allocating;
/// the painter hands its scratch in and reads the slice back.
pub fn visible(model: &TableModel, state: &TableState, out: &mut Vec<usize>) {
    out.clear();
    out.extend(
        model
            .rows
            .iter()
            .enumerate()
            .filter(|(_, row)| passes(row, &state.filter))
            .map(|(index, _)| index),
    );

    for key in state.sort.iter().rev() {
        let Some(column) = model.columns.iter().position(|c| c.key == key.column) else {
            continue;
        };
        out.sort_by(|a, b| {
            let left = model.rows[*a].cells.get(column);
            let right = model.rows[*b].cells.get(column);
            let order = compare_cells(left, right);
            if key.descending {
                order.reverse()
            } else {
                order
            }
        });
    }
}

fn passes(row: &TableRow, filter: &FilterState) -> bool {
    let query = filter.query.trim().to_lowercase();
    if !query.is_empty()
        && !row
            .cells
            .iter()
            .any(|cell| cell_text(cell).to_lowercase().contains(&query))
    {
        return false;
    }

    match filter.tags.as_slice() {
        [] => true,
        tags if filter.tag_mode == TagMode::Any => tags.iter().any(|t| row.tags.contains(t)),
        tags => tags.iter().all(|t| row.tags.contains(t)),
    }
}

/// A cell's text as a person reads it: the meter's label, not its fraction.
pub fn cell_text(cell: &TableCell) -> String {
    match cell {
        TableCell::Text(text) => text.clone(),
        TableCell::Status { label, .. } => label.clone(),
        TableCell::Meter { text, .. } => text.clone(),
    }
}

/// Numeric cells compare numerically, the rest by text, case-insensitively;
/// an absent cell sorts first. Numbers beat text so a column of counts does
/// not read `10 < 9`.
fn compare_cells(left: Option<&TableCell>, right: Option<&TableCell>) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    match (left, right) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Less,
        (Some(_), None) => Ordering::Greater,
        (Some(a), Some(b)) => match (numeric(a), numeric(b)) {
            (Some(a), Some(b)) => a.partial_cmp(&b).unwrap_or(Ordering::Equal),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => cell_text(a).to_lowercase().cmp(&cell_text(b).to_lowercase()),
        },
    }
}

fn numeric(cell: &TableCell) -> Option<f64> {
    match cell {
        TableCell::Meter { ratio, .. } => Some(*ratio),
        TableCell::Text(text) => text.trim().parse().ok(),
        TableCell::Status { .. } => None,
    }
}

impl TableState {
    /// Apply one intent. `ids` are the visible rows' ids in order; returns
    /// whether anything changed, so a host can skip a save when a key did
    /// nothing.
    pub fn apply(&mut self, command: &TableCommand, ids: &[String]) -> bool {
        let before = self.clone();
        match command {
            TableCommand::FocusFirst => self.focus_on(ids.first()),
            TableCommand::FocusLast => self.focus_on(ids.last()),
            TableCommand::Up => self.step(ids, -1),
            TableCommand::Down => self.step(ids, 1),
            TableCommand::PageUp(page) => self.step(ids, -(*page as isize)),
            TableCommand::PageDown(page) => self.step(ids, *page as isize),
            TableCommand::ToggleSelected => {
                if let Some(id) = self.focus.clone() {
                    toggle(&mut self.selected, id);
                }
            }
            TableCommand::ToggleSelectAll => {
                if self.selected.len() == ids.len() && !ids.is_empty() {
                    self.selected.clear();
                } else {
                    self.selected = ids.to_vec();
                }
            }
            TableCommand::Focus(id) => {
                if ids.iter().any(|visible| visible == id) {
                    self.focus = Some(id.clone());
                }
            }
            TableCommand::SortBy(column) => self.toggle_sort(column),
            TableCommand::ToggleTag(tag) => toggle(&mut self.filter.tags, tag.clone()),
        }
        *self != before
    }

    /// Whether the row is painted as selected.
    pub fn is_selected(&self, id: &str) -> bool {
        self.selected.iter().any(|selected| selected == id)
    }

    /// Whether the row holds focus: the painter outlines it.
    pub fn is_focused(&self, id: &str) -> bool {
        self.focus.as_deref() == Some(id)
    }

    /// The direction this column is sorted by, if it is.
    pub fn sort_direction(&self, column: &str) -> Option<bool> {
        self.sort
            .iter()
            .find(|key| key.column == column)
            .map(|key| key.descending)
    }

    fn focus_on(&mut self, id: Option<&String>) {
        if let Some(id) = id {
            self.focus = Some(id.clone());
        }
    }

    fn step(&mut self, ids: &[String], delta: isize) {
        if ids.is_empty() {
            self.focus = None;
            return;
        }
        let current = self
            .focus
            .as_ref()
            .and_then(|focus| ids.iter().position(|id| id == focus));
        let next = match (current, delta) {
            (None, delta) if delta < 0 => ids.len() - 1,
            (None, _) => 0,
            (Some(index), delta) => (index as isize + delta).clamp(0, ids.len() as isize - 1) as usize,
        };
        self.focus = Some(ids[next].clone());
        self.scroll = next;
    }

    /// Descending, then ascending, then not sorted by this column.
    fn toggle_sort(&mut self, column: &str) {
        match self
            .sort
            .iter()
            .position(|key| key.column == column)
            .map(|index| (index, self.sort[index].descending))
        {
            None => self.sort.insert(
                0,
                SortKey {
                    column: column.to_string(),
                    descending: true,
                },
            ),
            Some((index, true)) => self.sort[index].descending = false,
            Some((index, false)) => {
                self.sort.remove(index);
            }
        }
    }
}

fn toggle(list: &mut Vec<String>, value: String) {
    match list.iter().position(|item| item == &value) {
        Some(index) => {
            list.remove(index);
        }
        None => list.push(value),
    }
}

#[cfg(test)]
mod state_tests {
    use super::*;

    fn model() -> TableModel {
        TableModel {
            columns: vec![
                TableColumn {
                    key: "id".into(),
                    title: "Id".into(),
                    width: ColumnWidth::Fixed { value: 8 },
                    align: TextAlign::Left,
                },
                TableColumn {
                    key: "count".into(),
                    title: "Count".into(),
                    width: ColumnWidth::Fixed { value: 8 },
                    align: TextAlign::Right,
                },
                TableColumn {
                    key: "state".into(),
                    title: "State".into(),
                    width: ColumnWidth::Fixed { value: 8 },
                    align: TextAlign::Left,
                },
            ],
            rows: vec![
                row("imp-9", "9", "open", &["p1"]),
                row("imp-10", "10", "claimed", &["p1", "seat"]),
                row("imp-3", "3", "review", &["seat"]),
                row("imp-1", "1", "done", &[]),
            ],
        }
    }

    fn row(id: &str, count: &str, state: &str, tags: &[&str]) -> TableRow {
        TableRow {
            id: id.into(),
            tags: tags.iter().map(|t| t.to_string()).collect(),
            cells: vec![
                TableCell::Text(id.into()),
                TableCell::Text(count.into()),
                TableCell::Status {
                    label: state.into(),
                    color: (39, 201, 63),
                },
            ],
        }
    }

    fn ids(model: &TableModel, state: &TableState) -> Vec<String> {
        let mut out = Vec::new();
        visible(model, state, &mut out);
        out.iter().map(|i| model.rows[*i].id.clone()).collect()
    }

    #[test]
    fn numbers_sort_as_numbers_and_text_as_text() {
        let model = model();
        let ascending = TableState {
            sort: vec![SortKey {
                column: "count".into(),
                descending: false,
            }],
            ..TableState::default()
        };
        assert_eq!(ids(&model, &ascending), ["imp-1", "imp-3", "imp-9", "imp-10"]);

        let descending = TableState {
            sort: vec![SortKey {
                column: "count".into(),
                descending: true,
            }],
            ..TableState::default()
        };
        assert_eq!(ids(&model, &descending), ["imp-10", "imp-9", "imp-3", "imp-1"]);
    }

    #[test]
    fn an_unknown_sort_column_leaves_the_order_alone() {
        let model = model();
        let state = TableState {
            sort: vec![
                SortKey {
                    column: "tags".into(),
                    descending: false,
                },
                SortKey {
                    column: "id".into(),
                    descending: false,
                },
            ],
            ..TableState::default()
        };
        assert_eq!(ids(&model, &state), ["imp-1", "imp-10", "imp-3", "imp-9"]);
    }

    #[test]
    fn the_query_matches_any_cell_and_the_tag_filter_combines_by_mode() {
        let model = model();
        let query = TableState {
            filter: FilterState {
                query: "claim".into(),
                ..FilterState::default()
            },
            ..TableState::default()
        };
        assert_eq!(ids(&model, &query), ["imp-10"]);

        let all = TableState {
            filter: FilterState {
                tags: vec!["p1".into(), "seat".into()],
                tag_mode: TagMode::All,
                ..FilterState::default()
            },
            ..TableState::default()
        };
        assert_eq!(ids(&model, &all), ["imp-10"]);

        let any = TableState {
            filter: FilterState {
                tags: vec!["p1".into(), "seat".into()],
                tag_mode: TagMode::Any,
                ..FilterState::default()
            },
            ..TableState::default()
        };
        assert_eq!(ids(&model, &any), ["imp-9", "imp-10", "imp-3"]);
    }

    #[test]
    fn keys_move_focus_and_selection_by_id_through_the_visible_window() {
        let model = model();
        let mut state = TableState::default();
        let ids = ids(&model, &state);

        assert!(state.apply(&TableCommand::Down, &ids));
        assert_eq!(state.focus.as_deref(), Some("imp-9"));
        state.apply(&TableCommand::PageDown(2), &ids);
        assert_eq!(state.focus.as_deref(), Some("imp-3"));
        state.apply(&TableCommand::Down, &ids);
        assert_eq!(state.focus.as_deref(), Some("imp-1"));
        state.apply(&TableCommand::Down, &ids);
        assert_eq!(state.focus.as_deref(), Some("imp-1"), "clamped at the end");
        state.apply(&TableCommand::FocusFirst, &ids);
        assert_eq!(state.focus.as_deref(), Some("imp-9"));

        state.apply(&TableCommand::ToggleSelected, &ids);
        assert!(state.is_selected("imp-9"));
        state.apply(&TableCommand::ToggleSelected, &ids);
        assert!(!state.is_selected("imp-9"));

        state.apply(&TableCommand::ToggleSelectAll, &ids);
        assert_eq!(state.selected.len(), 4);
        state.apply(&TableCommand::ToggleSelectAll, &ids);
        assert!(state.selected.is_empty());

        assert!(!state.apply(&TableCommand::Focus("imp-99".into()), &ids));
    }

    #[test]
    fn a_sort_cycles_descending_then_ascending_then_off() {
        let mut state = TableState::default();
        let ids: Vec<String> = vec!["a".into()];

        assert!(state.apply(&TableCommand::SortBy("count".into()), &ids));
        assert_eq!(state.sort_direction("count"), Some(true));
        state.apply(&TableCommand::SortBy("count".into()), &ids);
        assert_eq!(state.sort_direction("count"), Some(false));
        state.apply(&TableCommand::SortBy("count".into()), &ids);
        assert_eq!(state.sort_direction("count"), None);
        assert!(state.sort.is_empty());
    }

    #[test]
    fn a_tag_toggles_in_and_out_of_the_filter() {
        let mut state = TableState::default();
        state.apply(&TableCommand::ToggleTag("seat".into()), &[]);
        assert_eq!(state.filter.tags, ["seat"]);
        state.apply(&TableCommand::ToggleTag("seat".into()), &[]);
        assert!(state.filter.tags.is_empty());
    }

    #[test]
    fn the_state_round_trips_through_json_for_a_url_or_local_storage() {
        let state = TableState {
            sort: vec![SortKey {
                column: "count".into(),
                descending: true,
            }],
            filter: FilterState {
                query: "seat".into(),
                tags: vec!["p1".into()],
                tag_mode: TagMode::Any,
            },
            focus: Some("imp-10".into()),
            selected: vec!["imp-9".into()],
            density: Density::Dense,
            scroll: 2,
        };
        let json = serde_json::to_string(&state).expect("serialize");
        assert_eq!(
            serde_json::from_str::<TableState>(&json).expect("deserialize"),
            state
        );
    }
}
