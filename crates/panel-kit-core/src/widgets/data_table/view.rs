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
            sort(&mut rows, &sort_keys(data, state, &index));
        }
        Ordered {
            rows,
            given: data.len(),
        }
    }

    /// The rows of `self` that pass `state`, in order. `self` is the build of
    /// the same `data` under a state whose rows `state` can only narrow (see
    /// [`OrderKey::narrows`](super::OrderKey::narrows)); then the result is
    /// what [`Ordered::build`] gives, without looking at the other rows.
    pub fn refine(
        &self,
        columns: &[DataColumnSpec],
        data: &TableData,
        state: &TableState,
        options: BuildOptions,
    ) -> Ordered {
        let rows = match options.mode {
            QueryMode::Server => self.rows.clone(),
            QueryMode::Client => {
                let index = |id: &str| columns.iter().position(|c| c.id == id);
                let filter = Filter::new(columns, data, state, options.tags, &index);
                let passes = |&row: &usize| filter.passes(data, row);
                self.rows.iter().copied().filter(passes).collect()
            }
        };
        Ordered {
            rows,
            given: data.len(),
        }
    }

    /// The order of `data` when `self` is the build of its first `from_row`
    /// rows under the same `state`: only the rows from `from_row` on are
    /// filtered and sorted, then merged in behind equal old ones, as
    /// [`Ordered::build`] ties break by row.
    pub fn extend(
        &self,
        columns: &[DataColumnSpec],
        data: &TableData,
        state: &TableState,
        options: BuildOptions,
        from_row: usize,
    ) -> Ordered {
        let index = |id: &str| columns.iter().position(|c| c.id == id);
        let mut added: Vec<usize> = match options.mode {
            QueryMode::Server => (from_row..data.len()).collect(),
            QueryMode::Client => {
                let filter = Filter::new(columns, data, state, options.tags, &index);
                (from_row..data.len())
                    .filter(|&row| filter.passes(data, row))
                    .collect()
            }
        };
        let mut rows = Vec::with_capacity(self.rows.len() + added.len());
        let keys = sort_keys(data, state, &index);
        if options.mode == QueryMode::Server || keys.is_empty() {
            rows.extend_from_slice(&self.rows);
            rows.extend_from_slice(&added);
        } else {
            sort(&mut added, &keys);
            // Old rows keep their order under the grown ranks, and each sorts
            // before an equal new row; the new rows are ascending, so each
            // insertion point lies at or after the last.
            let mut rest = &self.rows[..];
            for &new in &added {
                let ahead = prefix_len(rest, |&old| keys.cmp(old, new).is_le());
                rows.extend_from_slice(&rest[..ahead]);
                rows.push(new);
                rest = &rest[ahead..];
            }
            rows.extend_from_slice(rest);
        }
        Ordered {
            rows,
            given: data.len(),
        }
    }
}

/// How many leading items of `slice` satisfy `holds`, which holds for a prefix
/// only; found by doubling, so a short answer costs few calls.
fn prefix_len<T>(slice: &[T], holds: impl Fn(&T) -> bool) -> usize {
    let mut bound = 1;
    while bound <= slice.len() && holds(&slice[bound - 1]) {
        bound *= 2;
    }
    let (from, to) = (bound / 2, bound.min(slice.len()));
    from + slice[from..to].partition_point(holds)
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
                // Equal numbers print equal and missing ones print empty, so
                // comparing values groups as the text would, without printing.
                let numbers = match columns[column].kind {
                    ColumnKind::Number => match data.order(column) {
                        Order::Number(values) => Some(values),
                        _ => None,
                    },
                    _ => None,
                };
                let same = |a: usize, b: usize| match numbers {
                    Some(values) => {
                        let (x, y) = (values[a], values[b]);
                        x == y || (x.is_nan() && y.is_nan())
                    }
                    None => match data.code(a, column) {
                        Some(code) => data.code(b, column) == Some(code),
                        None => data.text(a, column) == data.text(b, column),
                    },
                };
                view.entries.reserve(kept.len());
                view.rows.reserve(kept.len());
                view.row_entries.reserve(kept.len());
                for run in kept.chunk_by(|&a, &b| same(a, b)) {
                    let value = data.text(run[0], column).into_owned();
                    let folded = state.collapsed.contains(value.as_str());
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
        let words: Vec<String> = state.quick.split_whitespace().map(fold).collect();
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
                    ColumnFilter::Text(text) => Check::Text(fold(text)),
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

/// `text` with every char lowercased on its own. Unlike `str::to_lowercase`
/// this ignores word position, so `Σ` always folds to `σ` and never to `ς`.
pub(super) fn fold(text: &str) -> String {
    text.chars().flat_map(char::to_lowercase).collect()
}

/// Whether `hay`, folded as by [`fold`], contains `needle` (already folded).
fn contains_folded(hay: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    if hay.is_ascii() {
        if !needle.is_ascii() {
            return false;
        }
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
    // Lowercasing grows a char's UTF-8 length by at most half, so a cell this
    // short folds into the stack buffer.
    let mut buf = [0u8; 256];
    if hay.len() <= 170 {
        let (bytes, mut len) = (hay.as_bytes(), 0);
        let mut at = 0;
        while at < bytes.len() {
            if bytes[at].is_ascii() {
                buf[len] = bytes[at].to_ascii_lowercase();
                len += 1;
                at += 1;
            } else if let Some(c) = hay[at..].chars().next() {
                for lower in c.to_lowercase() {
                    len += lower.encode_utf8(&mut buf[len..]).len();
                }
                at += c.len_utf8();
            }
        }
        return contains_bytes(&buf[..len], needle.as_bytes());
    }
    // A folded char may expand to several, and a match may start inside one.
    let mut start = hay.chars().flat_map(char::to_lowercase);
    loop {
        let mut at = start.clone();
        if needle.chars().all(|want| at.next() == Some(want)) {
            return true;
        }
        if start.next().is_none() {
            return false;
        }
    }
}

/// Whether `hay` holds `needle`, which is not empty, as a run of bytes.
fn contains_bytes(hay: &[u8], needle: &[u8]) -> bool {
    let Some(last) = hay.len().checked_sub(needle.len()) else {
        return false;
    };
    let mut from = 0;
    while let Some(skip) = hay[from..=last].iter().position(|&b| b == needle[0]) {
        let at = from + skip;
        if hay[at + 1..at + needle.len()] == needle[1..] {
            return true;
        }
        from = at + 1;
    }
    false
}

/// The columns rows sort by, each with its direction. Grouping sorts by the
/// group column first, so groups come out contiguous; the user's sort orders
/// rows inside each.
struct SortKeys<'d>(Vec<(Order<'d>, SortDir)>);

fn sort_keys<'d>(
    data: &'d TableData,
    state: &TableState,
    index: &dyn Fn(&str) -> Option<usize>,
) -> SortKeys<'d> {
    let mut sort: Vec<SortSpec> = state.sort.clone();
    if let Some(group) = &state.group_by {
        sort.retain(|spec| &spec.column != group);
        sort.insert(0, SortSpec::new(group.clone(), SortDir::Asc));
    }
    SortKeys(
        sort.iter()
            .filter_map(|spec| Some((data.order(index(&spec.column)?), spec.dir)))
            .collect(),
    )
}

impl SortKeys<'_> {
    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The order [`sort`] gives rows `a` and `b`: by key, then by row.
    fn cmp(&self, a: usize, b: usize) -> Ordering {
        self.0
            .iter()
            .map(|(order, dir)| encode(order.key(a), *dir).cmp(&encode(order.key(b), *dir)))
            .find(|order| order.is_ne())
            .unwrap_or_else(|| a.cmp(&b))
    }
}

/// Sort `kept` by `keys`. A missing value sinks whichever way its column
/// sorts; ties keep their order.
fn sort(kept: &mut [usize], keys: &SortKeys<'_>) {
    let keys = &keys.0;
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

#[cfg(test)]
mod tests {
    use super::super::data::{TableDataBuilder, Value};
    use super::super::state::TableDefaults;
    use super::*;
    use proptest::prelude::*;
    use rstest::rstest;

    const COLUMN_IDS: [&str; 4] = ["item", "title", "kind", "n"];

    fn columns() -> Vec<DataColumnSpec> {
        vec![
            DataColumnSpec::new("item", "item"),
            DataColumnSpec::new("title", "title"),
            DataColumnSpec::new("kind", "kind").enumerated(),
            DataColumnSpec::new("n", "n").number(),
        ]
    }

    #[derive(Clone, Debug)]
    struct Row {
        title: Option<String>,
        kind: Option<&'static str>,
        n: Option<f64>,
        tags: Vec<&'static str>,
    }

    fn data_of(rows: &[Row]) -> TableData {
        let mut builder = TableDataBuilder::with_capacity(&columns(), rows.len());
        for (at, row) in rows.iter().enumerate() {
            let id = format!("r{at}");
            builder
                .row(
                    &id,
                    [
                        Value::Text(&id),
                        row.title.as_deref().map_or(Value::Missing, Value::Text),
                        row.kind.map_or(Value::Missing, Value::Text),
                        row.n.map_or(Value::Missing, Value::Number),
                    ],
                )
                .tags(row.tags.as_slice());
        }
        builder.build()
    }

    /// Words that fold to something else, expand, or share a natural order.
    const WORDS: [&str; 14] = [
        "ab",
        "AB",
        "école",
        "ÉCOLE",
        "straße",
        "STRASSE",
        "İstanbul",
        "istanbul",
        "東京",
        "a1",
        "a10",
        "a2",
        "x",
        "Σίσυφος",
    ];

    fn row() -> impl Strategy<Value = Row> {
        (
            prop::option::of(prop::collection::vec(
                prop::sample::select(&WORDS[..]),
                0..4,
            )),
            prop::option::of(prop::sample::select(
                &["open", "Open", "clôsed", "done"][..],
            )),
            prop_oneof![
                Just(None),
                Just(Some(-0.0)),
                (-3i32..4).prop_map(|n| Some(f64::from(n) / 2.0)),
            ],
            prop::sample::subsequence(vec!["a", "b", "c"], 0..=3),
        )
            .prop_map(|(title, kind, n, tags)| Row {
                title: title.map(|words| words.join(" ")),
                kind,
                n,
                tags,
            })
    }

    /// Everything about a query except its quick text.
    #[derive(Clone, Debug)]
    struct Query {
        sort: Vec<(usize, bool)>,
        group: Option<usize>,
        kind: Option<&'static str>,
        title: Option<&'static str>,
        tag: Option<&'static str>,
    }

    fn query() -> impl Strategy<Value = Query> {
        (
            prop::collection::vec((0..4usize, any::<bool>()), 0..4),
            prop::option::of(0..4usize),
            prop::option::of(prop::sample::select(&["open", "done"][..])),
            prop::option::of(prop::sample::select(&["A", "é"][..])),
            prop::option::of(prop::sample::select(&["a", "b"][..])),
        )
            .prop_map(|(sort, group, kind, title, tag)| Query {
                sort,
                group,
                kind,
                title,
                tag,
            })
    }

    fn state_of(query: &Query, quick: &str) -> TableState {
        let columns = columns();
        let mut state = TableState::new(&columns, TableDefaults::default());
        state.sort = query
            .sort
            .iter()
            .map(|&(column, asc)| {
                let dir = if asc { SortDir::Asc } else { SortDir::Desc };
                SortSpec::new(COLUMN_IDS[column], dir)
            })
            .collect();
        state.group_by = query.group.map(|column| COLUMN_IDS[column].to_string());
        state.set_filter(
            "kind",
            query.kind.and_then(|kind| ColumnFilter::values([kind])),
        );
        state.set_filter("title", query.title.and_then(ColumnFilter::text));
        if let Some(tag) = query.tag {
            state.toggle_tag(tag, false);
        }
        state.set_quick(quick);
        state
    }

    fn quick_word() -> impl Strategy<Value = String> {
        prop::collection::vec(
            prop::sample::select(
                &[
                    "a", "A", "b", "é", "É", "ss", "ß", "i", "İ", "東", "1", "0", "-", "o", "σ",
                    "Σ",
                ][..],
            ),
            1..4,
        )
        .prop_map(|parts| parts.concat())
    }

    /// A quick text with every word of `earlier` grown by a prefix, a suffix
    /// or a recasing, and maybe one more word.
    fn grown(earlier: Vec<String>) -> impl Strategy<Value = String> {
        let len = earlier.len();
        (
            prop::collection::vec((quick_word(), quick_word(), 0..3u8), len),
            prop::option::of(quick_word()),
        )
            .prop_map(move |(grow, extra)| {
                let mut words: Vec<String> = earlier
                    .iter()
                    .zip(grow)
                    .map(|(word, (before, after, how))| match how {
                        0 => format!("{before}{word}"),
                        1 => format!("{word}{after}"),
                        _ => format!("{}{after}", word.to_ascii_uppercase()),
                    })
                    .collect();
                words.extend(extra);
                words.join(" ")
            })
    }

    fn both_modes() -> impl Strategy<Value = BuildOptions> {
        (any::<bool>(), any::<bool>()).prop_map(|(server, tags)| BuildOptions {
            mode: if server {
                QueryMode::Server
            } else {
                QueryMode::Client
            },
            tags,
        })
    }

    /// A quick text, one grown from it, and an unrelated one.
    fn quick_texts() -> impl Strategy<Value = (String, String, String)> {
        prop::collection::vec(quick_word(), 0..3).prop_flat_map(|before| {
            (
                Just(before.join(" ")),
                grown(before),
                prop::collection::vec(quick_word(), 0..3).prop_map(|words| words.join(" ")),
            )
        })
    }

    proptest! {
        /// Typing more into the quick filter only removes rows: refining the
        /// previous order is the order a fresh build gives.
        #[test]
        fn refine_is_build_when_the_key_narrows(
            rows in prop::collection::vec(row(), 0..40),
            query in query(),
            (before, grown, unrelated) in quick_texts(),
            options in both_modes(),
        ) {
            let data = data_of(&rows);
            let columns = columns();
            let earlier = state_of(&query, &before);
            let old = Ordered::build(&columns, &data, &earlier, options);
            prop_assert!(state_of(&query, &grown).order_key().narrows(&earlier.order_key()));
            for quick in [&grown, &unrelated] {
                let now = state_of(&query, quick);
                if now.order_key().narrows(&earlier.order_key()) {
                    prop_assert_eq!(
                        old.refine(&columns, &data, &now, options),
                        Ordered::build(&columns, &data, &now, options),
                        "{:?} -> {:?}", before, quick
                    );
                }
            }
        }

        /// Rows appended later are merged into the old order exactly where a
        /// full build would put them.
        #[test]
        fn extend_is_build_of_all_rows(
            rows in prop::collection::vec(row(), 0..60),
            cut in 0.0..=1.0f64,
            query in query(),
            quick in prop::collection::vec(quick_word(), 0..2),
            options in both_modes(),
        ) {
            let from = (rows.len() as f64 * cut) as usize;
            let columns = columns();
            let state = state_of(&query, &quick.join(" "));
            let (first, all) = (data_of(&rows[..from]), data_of(&rows));
            let old = Ordered::build(&columns, &first, &state, options);
            prop_assert_eq!(
                old.extend(&columns, &all, &state, options, from),
                Ordered::build(&columns, &all, &state, options)
            );
        }

        #[test]
        fn contains_folded_is_lowercased_contains(
            hay in prop::collection::vec(prop::sample::select(&FOLDING[..]), 0..160),
            needle in prop::collection::vec(prop::sample::select(&FOLDING[..]), 0..4),
        ) {
            let (hay, needle) = (hay.concat(), needle.concat());
            prop_assert_eq!(
                contains_folded(&hay, &needle.to_lowercase()),
                hay.to_lowercase().contains(&needle.to_lowercase()),
                "{:?} in {:?}", needle, hay
            );
        }

        /// Groups are the runs of equal printed numbers, whatever the order.
        #[test]
        fn number_groups_are_runs_of_equal_text(
            rows in prop::collection::vec(row(), 1..30),
            picks in prop::collection::vec(any::<prop::sample::Index>(), 0..50),
            folded in prop::collection::vec(0..6i32, 0..2),
        ) {
            let data = data_of(&rows);
            let columns = columns();
            let kept: Vec<usize> = picks.iter().map(|p| p.index(rows.len())).collect();
            let mut state = TableState::new(&columns, TableDefaults::default());
            state.group_by = Some("n".into());
            state.collapsed = folded.iter().map(|n| (f64::from(*n - 3) / 2.0).to_string()).collect();
            let ordered = Ordered { rows: kept.clone(), given: rows.len() };
            let mut expected = Vec::new();
            for run in kept.chunk_by(|&a, &b| data.text(a, 3) == data.text(b, 3)) {
                let value = data.text(run[0], 3).into_owned();
                let hidden = state.collapsed.contains(&value);
                expected.push(Entry::Group { value, count: run.len(), folded: hidden });
                if !hidden {
                    expected.extend(run.iter().map(|&row| Entry::Row(row)));
                }
            }
            prop_assert_eq!(View::group(&ordered, &columns, &data, &state).entries, expected);
        }
    }

    /// ASCII, accented Latin, a char that lowercases to two (İ), one that
    /// does not change (ß), a combining dot, the Kelvin sign and CJK.
    const FOLDING: [&str; 12] = [
        "a", "B", "é", "É", "ß", "İ", "i", "\u{307}", "\u{212A}", "k", "東", "京",
    ];

    #[rstest]
    #[case("İSTANBUL", "i\u{307}stan", true)]
    #[case("İSTANBUL", "\u{307}s", true)]
    #[case("STRASSE", "straße", false)]
    #[case("\u{212A}elvin", "kelvin", true)]
    #[case("ÉCOLE", "école", true)]
    #[case("école", "ecole", false)]
    fn contains_folded_cases(#[case] hay: &str, #[case] needle: &str, #[case] found: bool) {
        assert_eq!(contains_folded(hay, &fold(needle)), found);
    }

    /// The stack buffer in `contains_folded` holds 1.5 bytes per source byte.
    #[test]
    fn folding_grows_a_char_by_at_most_half() {
        for c in (0..=char::MAX as u32).filter_map(char::from_u32) {
            let folded: usize = c.to_lowercase().map(char::len_utf8).sum();
            assert!(2 * folded <= 3 * c.len_utf8(), "{c:?}");
        }
    }

    /// Folding is per char: a capital sigma ending a word still matches the
    /// query typed in the same case, which `str::to_lowercase` would split
    /// into `σ` against `ς`.
    #[test]
    fn final_sigma_matches_itself() {
        assert_eq!(fold("ΟΔΟΣ"), "οδοσ");
        assert_eq!("ΟΔΟΣ".to_lowercase(), "οδος");
        assert!(contains_folded("ΟΔΟΣ", &fold("ΟΔΟΣ")));
        assert!(!contains_folded("οδος", &fold("οδοσ")));
    }

    #[rstest]
    #[case("", "", true)]
    #[case("", "ab", true)]
    #[case("ab", "", false)]
    #[case("ab", "abc", true)]
    #[case("a b", "ab c", true)]
    #[case("ab", "a", false)]
    #[case("a b", "b", false)]
    #[case("AB", "ab", true)]
    #[case("É", "é1", true)]
    #[case("a a", "a", true)]
    fn narrows_by_quick_words(#[case] earlier: &str, #[case] now: &str, #[case] narrows: bool) {
        let columns = columns();
        let key = |quick: &str| {
            let mut state = TableState::new(&columns, TableDefaults::default());
            state.set_quick(quick);
            state.order_key()
        };
        assert_eq!(
            key(now).narrows(&key(earlier)),
            narrows,
            "{earlier:?} -> {now:?}"
        );
    }

    #[test]
    fn only_the_quick_text_may_differ_to_narrow() {
        let columns = columns();
        let base = TableState::new(&columns, TableDefaults::default());
        let mut other = base.clone();
        other.set_quick("a");
        assert!(other.order_key().narrows(&base.order_key()));
        for change in [
            |s: &mut TableState| s.sort = vec![SortSpec::new("n", SortDir::Asc)],
            |s: &mut TableState| s.group_by = Some("kind".into()),
            |s: &mut TableState| s.set_filter("n", ColumnFilter::range(Some(1.0), None)),
            |s: &mut TableState| s.toggle_tag("a", false),
            |s: &mut TableState| {
                s.hidden.insert("title".into());
            },
        ] {
            let mut changed = other.clone();
            change(&mut changed);
            assert!(!changed.order_key().narrows(&base.order_key()));
        }
    }

    #[rstest]
    #[case(&[], 3, 0)]
    #[case(&[1], 3, 1)]
    #[case(&[1, 2, 3, 4, 5, 6, 7], 3, 3)]
    #[case(&[1, 2, 3, 4, 5, 6, 7], 0, 0)]
    #[case(&[1, 2, 3, 4, 5, 6, 7], 9, 7)]
    #[case(&[1, 2, 3, 4, 5, 6, 7, 8], 5, 5)]
    fn prefix_len_finds_the_boundary(#[case] items: &[i32], #[case] below: i32, #[case] at: usize) {
        assert_eq!(prefix_len(items, |&item| item <= below), at);
    }
}
