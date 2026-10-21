//! The rows a table shows for a state: filtered, sorted and grouped, flattened
//! into the entries a painter renders in order.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::collections::BTreeSet;

use super::column::{natural_cmp, ColumnKind, DataColumnSpec, SortDir, SortSpec};
use super::data::{Order, TableData};
use super::state::{ColumnFilter, TableState};
use super::tags::TagMode;

/// Most values an enum filter offers when the column names none.
pub const OPTION_LIMIT: usize = 200;

/// Who applies the query.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum QueryMode {
    /// The table filters and sorts the rows it holds.
    #[default]
    Client,
    /// The host already did; the table keeps its order and only groups.
    Server,
}

/// How a view is built besides the state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BuildOptions {
    /// Who applies the query.
    pub mode: QueryMode,
    /// Whether the table draws a ribbon; without one, a tag filter does not
    /// narrow.
    pub tags: bool,
}

impl Default for BuildOptions {
    fn default() -> Self {
        BuildOptions {
            mode: QueryMode::Client,
            tags: true,
        }
    }
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
    /// A row, by index into the table's data.
    Row(usize),
}

/// The view of the rows under a state.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct View {
    /// Group headers and rows, in render order.
    pub entries: Vec<Entry>,
    /// The rendered rows (folded groups' rows excluded), in order.
    pub rows: Vec<usize>,
    /// Each rendered row's index into `entries`.
    pub row_entries: Vec<usize>,
    /// Rows that passed the query, folded ones included.
    pub total: usize,
    /// Rows the table holds.
    pub given: usize,
    /// The values each filterable enum column offers.
    pub options: BTreeMap<String, Vec<String>>,
}

/// The rows that pass the query, in order: the expensive half of a view,
/// reused while only folding changes.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Ordered {
    /// Passing rows, sorted (the group column first when grouping).
    pub rows: Vec<usize>,
    /// Rows the table holds.
    pub given: usize,
}

impl Ordered {
    /// Filter and sort `data` under `state`.
    pub fn build(
        columns: &[DataColumnSpec],
        data: &TableData,
        state: &TableState,
        options: BuildOptions,
    ) -> Ordered {
        let index = |id: &str| columns.iter().position(|c| c.id == id);
        let mut rows: Vec<usize> = match options.mode {
            QueryMode::Server => (0..data.len()).collect(),
            QueryMode::Client => {
                let filter = Filter::new(columns, data, state, options.tags, &index);
                (0..data.len())
                    .filter(|&row| filter.passes(data, row))
                    .collect()
            }
        };
        if options.mode == QueryMode::Client {
            sort(&mut rows, data, state, &index);
        }
        Ordered {
            rows,
            given: data.len(),
        }
    }
}

impl View {
    /// Build the view of `data` under `state`.
    pub fn build(
        columns: &[DataColumnSpec],
        data: &TableData,
        state: &TableState,
        options: BuildOptions,
    ) -> View {
        View::group(
            &Ordered::build(columns, data, state, options),
            columns,
            data,
            state,
        )
    }

    /// Group ordered rows into entries, leaving folded groups' rows out.
    pub fn group(
        ordered: &Ordered,
        columns: &[DataColumnSpec],
        data: &TableData,
        state: &TableState,
    ) -> View {
        let kept = &ordered.rows;
        let mut view = View {
            total: kept.len(),
            given: ordered.given,
            options: enum_options(columns, data),
            ..View::default()
        };
        let group = state
            .group_by
            .as_deref()
            .and_then(|id| columns.iter().position(|c| c.id == id));
        match group {
            None => {
                view.row_entries = (0..kept.len()).collect();
                view.entries = kept.iter().map(|&row| Entry::Row(row)).collect();
                view.rows = kept.clone();
            }
            Some(column) => {
                let same = |a: usize, b: usize| match data.code(a, column) {
                    Some(code) => data.code(b, column) == Some(code),
                    None => data.text(a, column) == data.text(b, column),
                };
                for run in kept.chunk_by(|&a, &b| same(a, b)) {
                    let value = data.text(run[0], column).into_owned();
                    let folded = state.collapsed.contains(&value);
                    view.entries.push(Entry::Group {
                        value,
                        count: run.len(),
                        folded,
                    });
                    if !folded {
                        for &row in run {
                            view.row_entries.push(view.entries.len());
                            view.rows.push(row);
                            view.entries.push(Entry::Row(row));
                        }
                    }
                }
            }
        }
        view
    }

    /// The id of the rendered row at `at`.
    pub fn id<'a>(&self, data: &'a TableData, at: usize) -> &'a str {
        data.id(self.rows[at])
    }

    /// Where the row `id` renders, among the rendered rows.
    pub fn position(&self, data: &TableData, id: &str) -> Option<usize> {
        self.rows.iter().position(|&row| data.id(row) == id)
    }

    /// The rendered rows' ids, in order.
    pub fn ids<'a>(&'a self, data: &'a TableData) -> impl Iterator<Item = &'a str> + 'a {
        self.rows.iter().map(move |&row| data.id(row))
    }
}

/// A query resolved against the data once, so each row is checked without
/// allocating.
struct Filter<'a> {
    words: Vec<String>,
    /// Per visible column, how each word is looked for in it.
    visible: Vec<Quick>,
    /// Column filters.
    columns: Vec<(usize, Check<'a>)>,
    tags: Option<(Vec<Option<u32>>, TagMode)>,
}

/// How the quick filter reads one visible column.
enum Quick {
    /// Search the text.
    Text(usize),
    /// An enum: per word, whether each code's value contains it.
    Codes(usize, Vec<Vec<bool>>),
    /// A number: per word, whether it could appear in a number's text at all.
    Number(usize, Vec<bool>),
}

enum Check<'a> {
    Text(String),
    Codes(Vec<u32>),
    Values(&'a [String]),
    Range(Option<f64>, Option<f64>),
}

impl<'a> Filter<'a> {
    fn new(
        columns: &[DataColumnSpec],
        data: &TableData,
        state: &'a TableState,
        tags: bool,
        index: &dyn Fn(&str) -> Option<usize>,
    ) -> Self {
        let words: Vec<String> = state
            .quick
            .split_whitespace()
            .map(str::to_lowercase)
            .collect();
        let visible = state
            .visible(columns)
            .iter()
            .filter_map(|c| {
                let at = index(&c.id)?;
                Some(match (c.kind, data.dictionary(at)) {
                    (_, Some(dict)) => Quick::Codes(
                        at,
                        words
                            .iter()
                            .map(|word| {
                                dict.iter()
                                    .map(|value| contains_folded(value, word))
                                    .collect()
                            })
                            .collect(),
                    ),
                    (ColumnKind::Number, None) => Quick::Number(
                        at,
                        words
                            .iter()
                            .map(|word| {
                                word.chars().all(|c| {
                                    c.is_ascii_digit()
                                        || matches!(c, '.' | '-' | 'e' | '+' | 'i' | 'n' | 'f')
                                })
                            })
                            .collect(),
                    ),
                    _ => Quick::Text(at),
                })
            })
            .collect();
        let checks = state
            .filters
            .iter()
            .filter_map(|(id, filter)| {
                let at = index(id)?;
                let check = match filter {
                    ColumnFilter::Text(text) => Check::Text(text.to_lowercase()),
                    ColumnFilter::Values(values) => match data.dictionary(at) {
                        Some(dict) => Check::Codes(
                            (0..dict.len() as u32)
                                .filter(|&code| values.iter().any(|v| **v == *dict[code as usize]))
                                .collect(),
                        ),
                        None => Check::Values(values),
                    },
                    ColumnFilter::Range { min, max } => Check::Range(*min, *max),
                };
                Some((at, check))
            })
            .collect();
        let tags = (tags && !state.tag_filter.is_empty()).then(|| {
            (
                state.tag_filter.iter().map(|t| data.tag_id(t)).collect(),
                state.tag_mode,
            )
        });
        Filter {
            words,
            visible,
            columns: checks,
            tags,
        }
    }

    fn passes(&self, data: &TableData, row: usize) -> bool {
        self.quick(data, row) && self.checks(data, row) && self.tags(data, row)
    }

    /// Every word appears in some visible column.
    fn quick(&self, data: &TableData, row: usize) -> bool {
        self.words.iter().enumerate().all(|(w, word)| {
            self.visible.iter().any(|quick| match quick {
                Quick::Text(column) => contains_folded(&data.text(row, *column), word),
                Quick::Codes(column, matches) => data
                    .code(row, *column)
                    .is_some_and(|code| matches[w][code as usize]),
                Quick::Number(column, possible) => {
                    possible[w] && data.with_text(row, *column, |text| contains_folded(text, word))
                }
            })
        })
    }

    fn checks(&self, data: &TableData, row: usize) -> bool {
        self.columns.iter().all(|(column, check)| match check {
            Check::Text(want) => data.with_text(row, *column, |text| contains_folded(text, want)),
            Check::Codes(codes) => data.code(row, *column).is_some_and(|c| codes.contains(&c)),
            Check::Values(values) => {
                let text = data.text(row, *column);
                values.iter().any(|v| **v == *text)
            }
            Check::Range(min, max) => data
                .number(row, *column)
                .is_some_and(|n| min.is_none_or(|m| n >= m) && max.is_none_or(|m| n <= m)),
        })
    }

    fn tags(&self, data: &TableData, row: usize) -> bool {
        let Some((want, mode)) = &self.tags else {
            return true;
        };
        let have = data.tag_ids(row);
        let holds = |tag: &Option<u32>| tag.is_some_and(|id| have.binary_search(&id).is_ok());
        match mode {
            TagMode::Any => want.iter().any(holds),
            TagMode::All => want.iter().all(holds),
        }
    }
}

/// Whether `hay` contains `needle` (already lowercase), ignoring case.
fn contains_folded(hay: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    if hay.is_ascii() && needle.is_ascii() {
        let (h, n) = (hay.as_bytes(), needle.as_bytes());
        let Some(last) = h.len().checked_sub(n.len()) else {
            return false;
        };
        let (lower, upper) = (n[0], n[0].to_ascii_uppercase());
        let mut from = 0;
        while let Some(skip) = h[from..=last]
            .iter()
            .position(|&b| b == lower || b == upper)
        {
            let at = from + skip;
            if h[at + 1..at + n.len()].eq_ignore_ascii_case(&n[1..]) {
                return true;
            }
            from = at + 1;
        }
        return false;
    }
    hay.to_lowercase().contains(needle)
}

/// Grouping sorts by the group column first, so groups come out contiguous;
/// the user's sort orders rows inside each. A missing value sinks whichever
/// way its column sorts; ties keep their order.
fn sort(
    kept: &mut [usize],
    data: &TableData,
    state: &TableState,
    index: &dyn Fn(&str) -> Option<usize>,
) {
    let mut sort: Vec<SortSpec> = state.sort.clone();
    if let Some(group) = &state.group_by {
        sort.retain(|spec| &spec.column != group);
        sort.insert(0, SortSpec::new(group.clone(), SortDir::Asc));
    }
    let keys: Vec<(Order<'_>, SortDir)> = sort
        .iter()
        .filter_map(|spec| Some((data.order(index(&spec.column)?), spec.dir)))
        .collect();
    if keys.is_empty() {
        return;
    }
    // Each key becomes a u64 whose unsigned order is the sort order, missing
    // last either way; one or two keys pack with the row so the sort compares
    // plain integers.
    let encoded: Vec<Vec<u64>> = keys
        .iter()
        .map(|(order, dir)| {
            kept.iter()
                .map(|&row| encode(order.key(row), *dir))
                .collect()
        })
        .collect();
    match encoded.as_slice() {
        [first] => {
            let mut packed: Vec<(u64, usize)> =
                first.iter().copied().zip(kept.iter().copied()).collect();
            packed.sort_unstable();
            for (slot, (_, row)) in kept.iter_mut().zip(packed) {
                *slot = row;
            }
        }
        [first, second] => {
            let mut packed: Vec<(u64, u64, usize)> = first
                .iter()
                .zip(second)
                .zip(kept.iter())
                .map(|((&a, &b), &row)| (a, b, row))
                .collect();
            packed.sort_unstable();
            for (slot, (_, _, row)) in kept.iter_mut().zip(packed) {
                *slot = row;
            }
        }
        [first, second, third] => {
            let mut packed: Vec<(u64, u64, u64, usize)> = (0..kept.len())
                .map(|i| (first[i], second[i], third[i], kept[i]))
                .collect();
            packed.sort_unstable();
            for (slot, (_, _, _, row)) in kept.iter_mut().zip(packed) {
                *slot = row;
            }
        }
        many => {
            let mut at: Vec<usize> = (0..kept.len()).collect();
            at.sort_unstable_by(|&a, &b| {
                many.iter()
                    .map(|keys| keys[a].cmp(&keys[b]))
                    .find(|order| order.is_ne())
                    .unwrap_or(Ordering::Equal)
                    .then(kept[a].cmp(&kept[b]))
            });
            let rows: Vec<usize> = at.iter().map(|&i| kept[i]).collect();
            kept.copy_from_slice(&rows);
        }
    }
}

/// A sort key as a u64 whose unsigned order is the wanted order: present
/// values ascending or descending, missing after every present one.
#[inline]
fn encode(key: Option<f64>, dir: SortDir) -> u64 {
    let Some(value) = key else {
        return u64::MAX;
    };
    let bits = value.to_bits();
    // Flip so unsigned order matches numeric order for every finite value.
    let ascending = if bits >> 63 == 1 {
        !bits
    } else {
        bits | (1 << 63)
    };
    match dir {
        SortDir::Asc => ascending.min(u64::MAX - 1),
        SortDir::Desc => (!ascending).min(u64::MAX - 1),
    }
}

/// The values each filterable enum column offers: its declared options, else
/// its distinct non-empty values (at most [`OPTION_LIMIT`]) in natural order.
fn enum_options(columns: &[DataColumnSpec], data: &TableData) -> BTreeMap<String, Vec<String>> {
    columns
        .iter()
        .enumerate()
        .filter(|(_, c)| c.kind == ColumnKind::Enum && c.filterable)
        .map(|(at, column)| {
            let values = column.options.clone().unwrap_or_else(|| {
                let mut values: Vec<String> = data
                    .dictionary(at)
                    .unwrap_or_default()
                    .iter()
                    .filter(|v| !v.is_empty())
                    .take(OPTION_LIMIT)
                    .map(|v| v.to_string())
                    .collect();
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
pub fn empty_sentence(tag_filter: &BTreeSet<String>) -> String {
    if tag_filter.is_empty() {
        "no rows match the table filter".to_string()
    } else {
        let tags: Vec<&str> = tag_filter.iter().map(String::as_str).collect();
        format!("no rows match tags {}", tags.join(", "))
    }
}
