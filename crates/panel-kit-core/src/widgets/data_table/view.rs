//! The rows a table shows for a state: filtered, sorted and grouped, flattened
//! into the entries a painter renders in order.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::collections::HashMap;

use super::column::{natural_cmp, ColumnKind, DataColumnSpec, SortDir, SortKey, SortSpec};
use super::state::{ColumnFilter, TableState};
use super::tags::{row_labels, TagMode};

/// Most values an enum filter offers when the column names none.
pub const OPTION_LIMIT: usize = 200;

/// One row's query data: its id, its tags, and per declared column (in
/// declaration order) a sort key and the cell's plain text.
#[derive(Clone, Copy, Debug)]
pub struct RowKeys<'a> {
    /// Stable row identity.
    pub id: &'a str,
    /// The row's tags; `None` when the table has no ribbon, which a tag
    /// filter then leaves alone.
    pub tags: Option<&'a [String]>,
    /// Sort keys, parallel to the declared columns.
    pub keys: &'a [SortKey],
    /// Cell text, parallel to the declared columns: what filters match,
    /// groups are named by, and exports write.
    pub text: &'a [String],
}

impl RowKeys<'_> {
    /// The text of the column declared at `index`; empty when absent.
    pub fn cell(&self, index: usize) -> &str {
        self.text.get(index).map(String::as_str).unwrap_or("")
    }
}

/// Who applies the query.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum QueryMode {
    /// The table filters and sorts the rows it holds.
    #[default]
    Client,
    /// The host already did; the table keeps its order and only groups.
    Server,
}

/// One rendered line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Entry {
    /// A group header: its value, its row count, and whether it is folded.
    Group {
        /// The grouping column's text for these rows.
        value: String,
        /// Rows in the group.
        count: usize,
        /// Whether its rows are hidden.
        folded: bool,
    },
    /// A row, by index into the rows the table was given.
    Row(usize),
}

/// The view of the rows under a state.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct View {
    /// Group headers and rows, in render order.
    pub entries: Vec<Entry>,
    /// Ids of the rows rendered (folded groups' rows excluded), in order.
    pub ids: Vec<String>,
    /// Each rendered row's index into `entries`.
    pub positions: HashMap<String, usize>,
    /// Rows that passed the query, folded ones included.
    pub total: usize,
    /// Rows the table was given.
    pub given: usize,
    /// The values each filterable enum column offers.
    pub options: BTreeMap<String, Vec<String>>,
}

impl View {
    /// Build the view of `rows` under `state`.
    pub fn build(
        columns: &[DataColumnSpec],
        rows: &[RowKeys<'_>],
        state: &TableState,
        mode: QueryMode,
    ) -> View {
        let index = |id: &str| columns.iter().position(|c| c.id == id);
        let kept: Vec<usize> = match mode {
            QueryMode::Server => (0..rows.len()).collect(),
            QueryMode::Client => {
                let visible: Vec<usize> = state
                    .visible(columns)
                    .iter()
                    .filter_map(|c| index(&c.id))
                    .collect();
                let mut kept: Vec<usize> = (0..rows.len())
                    .filter(|&at| {
                        let row = &rows[at];
                        quick(row, &state.quick, &visible)
                            && filters(row, state, &index)
                            && tags(row, state)
                    })
                    .collect();
                sort(&mut kept, rows, state, &index);
                kept
            }
        };

        let mut view = View {
            total: kept.len(),
            given: rows.len(),
            options: options(columns, rows),
            ..View::default()
        };
        let group = state.group_by.as_deref().and_then(index);
        let push_row = |view: &mut View, at: usize| {
            view.positions
                .insert(rows[at].id.to_string(), view.entries.len());
            view.ids.push(rows[at].id.to_string());
            view.entries.push(Entry::Row(at));
        };
        match group {
            None => kept.iter().for_each(|&at| push_row(&mut view, at)),
            Some(column) => {
                for run in kept.chunk_by(|&a, &b| rows[a].cell(column) == rows[b].cell(column)) {
                    let value = rows[run[0]].cell(column).to_string();
                    let folded = state.collapsed.contains(&value);
                    view.entries.push(Entry::Group {
                        value,
                        count: run.len(),
                        folded,
                    });
                    if !folded {
                        run.iter().for_each(|&at| push_row(&mut view, at));
                    }
                }
            }
        }
        view
    }

    /// Row indices of every rendered row, in view order.
    pub fn row_indices(&self) -> impl Iterator<Item = usize> + '_ {
        self.entries.iter().filter_map(|entry| match entry {
            Entry::Row(at) => Some(*at),
            Entry::Group { .. } => None,
        })
    }
}

/// Every word of the quick filter appears in some visible column's text.
fn quick(row: &RowKeys<'_>, quick: &str, visible: &[usize]) -> bool {
    let words: Vec<String> = quick.split_whitespace().map(str::to_lowercase).collect();
    if words.is_empty() {
        return true;
    }
    let text = visible
        .iter()
        .map(|&at| row.cell(at).to_lowercase())
        .collect::<Vec<_>>()
        .join(" ");
    words.iter().all(|word| text.contains(word.as_str()))
}

/// Every column filter passes; a hidden column's filter still applies.
fn filters(row: &RowKeys<'_>, state: &TableState, index: &dyn Fn(&str) -> Option<usize>) -> bool {
    state.filters.iter().all(|(id, filter)| {
        let Some(at) = index(id) else { return true };
        let text = row.cell(at);
        match filter {
            ColumnFilter::Text(want) => text.to_lowercase().contains(&want.to_lowercase()),
            ColumnFilter::Values(values) => values.iter().any(|value| value == text),
            ColumnFilter::Range { min, max } => {
                let number = match row.keys.get(at) {
                    Some(SortKey::Num(n)) => Some(*n),
                    _ => text.trim().parse::<f64>().ok(),
                };
                number.is_some_and(|n| min.is_none_or(|m| n >= m) && max.is_none_or(|m| n <= m))
            }
        }
    })
}

/// The tag filter, over the row's canonical labels. A table with no ribbon is
/// not narrowed by it.
fn tags(row: &RowKeys<'_>, state: &TableState) -> bool {
    let Some(tags) = row.tags.filter(|_| !state.tag_filter.is_empty()) else {
        return true;
    };
    let have = row_labels(tags);
    match state.tag_mode {
        TagMode::Any => state.tag_filter.iter().any(|tag| have.contains(tag)),
        TagMode::All => state.tag_filter.iter().all(|tag| have.contains(tag)),
    }
}

/// Grouping sorts by the group column first, so groups come out contiguous;
/// the user's sort orders rows inside each. A missing value sinks whichever
/// way its column sorts. Stable.
fn sort(
    kept: &mut [usize],
    rows: &[RowKeys<'_>],
    state: &TableState,
    index: &dyn Fn(&str) -> Option<usize>,
) {
    let mut sort: Vec<SortSpec> = state.sort.clone();
    if let Some(group) = &state.group_by {
        sort.retain(|spec| &spec.column != group);
        sort.insert(0, SortSpec::new(group.clone(), SortDir::Asc));
    }
    let keys: Vec<(usize, SortDir)> = sort
        .iter()
        .filter_map(|spec| Some((index(&spec.column)?, spec.dir)))
        .collect();
    if keys.is_empty() {
        return;
    }
    let missing = SortKey::Missing;
    kept.sort_by(|&a, &b| {
        keys.iter()
            .map(|&(column, dir)| {
                let left = rows[a].keys.get(column).unwrap_or(&missing);
                let right = rows[b].keys.get(column).unwrap_or(&missing);
                match (left, right) {
                    (SortKey::Missing, SortKey::Missing) => Ordering::Equal,
                    (SortKey::Missing, _) => Ordering::Greater,
                    (_, SortKey::Missing) => Ordering::Less,
                    _ if dir == SortDir::Desc => right.order(left),
                    _ => left.order(right),
                }
            })
            .find(|order| *order != Ordering::Equal)
            .unwrap_or(Ordering::Equal)
    });
}

/// The values each filterable enum column offers: its declared options, else
/// the distinct non-empty values present (at most [`OPTION_LIMIT`]), in
/// natural order.
fn options(columns: &[DataColumnSpec], rows: &[RowKeys<'_>]) -> BTreeMap<String, Vec<String>> {
    columns
        .iter()
        .enumerate()
        .filter(|(_, c)| c.kind == ColumnKind::Enum && c.filterable)
        .map(|(at, column)| {
            let values = column.options.clone().unwrap_or_else(|| {
                let mut values: Vec<String> = Vec::new();
                for row in rows {
                    let text = row.cell(at);
                    if !text.is_empty() && !values.iter().any(|v| v == text) {
                        values.push(text.to_string());
                        if values.len() == OPTION_LIMIT {
                            break;
                        }
                    }
                }
                values.sort_by(|a, b| natural_cmp(a, b));
                values
            });
            (column.id.clone(), values)
        })
        .collect()
}

/// The toolbar's count: `N rows`, `N of M rows` when the query narrowed them,
/// and the selection when there is one.
pub fn tally(view: &View, selected: usize) -> String {
    let rows = if view.total == view.given {
        format!("{} rows", view.total)
    } else {
        format!("{} of {} rows", view.total, view.given)
    };
    match selected {
        0 => rows,
        n => format!("{rows}, {n} selected"),
    }
}

/// What the body says when rows exist but none pass: it names what narrowed
/// them.
pub fn empty_sentence(tag_filter: &std::collections::BTreeSet<String>) -> String {
    if tag_filter.is_empty() {
        "no rows match the table filter".to_string()
    } else {
        let tags: Vec<&str> = tag_filter.iter().map(String::as_str).collect();
        format!("no rows match tags {}", tags.join(", "))
    }
}
