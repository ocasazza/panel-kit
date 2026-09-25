//! Interactive data-table state: filter, sort, and column visibility.
//!
//! The semantic [`table`](super::table) model paints fixed rows. A data table
//! adds the three things every long resource list needs — a text filter,
//! click-to-sort columns, and user-chosen visible columns — as pure,
//! renderer-neutral state so web and terminal painters share one behaviour and
//! hosts can persist the [`TableQuery`] however they like.

use std::cmp::Ordering;
use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

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

/// Column declaration for a data table.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataColumnSpec {
    /// Stable id used for sort state and visibility persistence.
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

/// User-controlled table state: filter text, sort, and hidden columns.
///
/// Serializable so hosts can persist it (the web painter stores everything
/// but the filter text under a caller-chosen key).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableQuery {
    /// Whitespace-separated terms; every term must match the row's haystack.
    #[serde(default, skip_serializing)]
    pub filter: String,
    /// Active sort, if any.
    #[serde(default)]
    pub sort: Option<SortSpec>,
    /// Column ids explicitly hidden by the user.
    #[serde(default)]
    pub hidden: BTreeSet<String>,
    /// Column ids explicitly shown that are hidden by default.
    #[serde(default)]
    pub shown: BTreeSet<String>,
}

impl TableQuery {
    /// Initial state with a default sort.
    pub fn sorted(column: impl Into<String>, dir: SortDir) -> Self {
        Self {
            sort: Some(SortSpec {
                column: column.into(),
                dir,
            }),
            ..Self::default()
        }
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

    /// Header click: new column sorts ascending, same column flips direction.
    /// Unsortable or unknown columns are ignored.
    pub fn click_header(&mut self, columns: &[DataColumnSpec], id: &str) {
        if !columns.iter().any(|c| c.id == id && c.sortable) {
            return;
        }
        self.sort = Some(match &self.sort {
            Some(s) if s.column == id && s.dir == SortDir::Asc => SortSpec {
                column: id.to_string(),
                dir: SortDir::Desc,
            },
            _ => SortSpec {
                column: id.to_string(),
                dir: SortDir::Asc,
            },
        });
    }
}

/// One row's query-relevant data: a lowercase search haystack and one sort key
/// per declared column (same order as the column list).
pub struct RowKeys<'a> {
    /// Lowercased text the filter matches against.
    pub search: &'a str,
    /// Sort keys, parallel to the column list.
    pub keys: &'a [SortKey],
}

/// Indices of rows that pass the filter, in sorted order. Stable: rows with
/// equal keys keep their input order.
pub fn visible_rows(
    columns: &[DataColumnSpec],
    rows: &[RowKeys<'_>],
    query: &TableQuery,
) -> Vec<usize> {
    let terms: Vec<String> = query
        .filter
        .split_whitespace()
        .map(str::to_lowercase)
        .collect();
    let mut out: Vec<usize> = rows
        .iter()
        .enumerate()
        .filter(|(_, row)| terms.iter().all(|t| row.search.contains(t.as_str())))
        .map(|(i, _)| i)
        .collect();

    let sort = query.sort.as_ref().and_then(|s| {
        columns
            .iter()
            .position(|c| c.id == s.column && c.sortable)
            .map(|i| (i, s.dir))
    });
    if let Some((col, dir)) = sort {
        let missing = SortKey::Missing;
        out.sort_by(|&a, &b| {
            let ka = rows[a].keys.get(col).unwrap_or(&missing);
            let kb = rows[b].keys.get(col).unwrap_or(&missing);
            match (ka, kb) {
                (SortKey::Missing, SortKey::Missing) => Ordering::Equal,
                (SortKey::Missing, _) => Ordering::Greater,
                (_, SortKey::Missing) => Ordering::Less,
                _ if dir == SortDir::Desc => kb.order(ka),
                _ => ka.order(kb),
            }
        });
    }
    out
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

    fn run(data: &[(String, Vec<SortKey>)], q: &TableQuery) -> Vec<usize> {
        let rows: Vec<RowKeys> = data
            .iter()
            .map(|(s, k)| RowKeys { search: s, keys: k })
            .collect();
        visible_rows(&cols(), &rows, q)
    }

    #[test]
    fn filter_requires_every_term() {
        let data = [
            keys("humanoid-recover", None),
            keys("spider-recover", None),
            keys("spot-walk", None),
        ];
        let q = TableQuery {
            filter: "RECOVER spi".into(),
            ..TableQuery::default()
        };
        assert_eq!(run(&data, &q), vec![1]);
    }

    #[test]
    fn missing_values_sort_last_in_both_directions() {
        let data = [
            keys("a", Some(900.0)),
            keys("b", None),
            keys("c", Some(10_000.0)),
        ];
        assert_eq!(
            run(&data, &TableQuery::sorted("created", SortDir::Desc)),
            vec![2, 0, 1]
        );
        assert_eq!(
            run(&data, &TableQuery::sorted("created", SortDir::Asc)),
            vec![0, 2, 1]
        );
    }

    #[test]
    fn header_click_cycles_and_ignores_unsortable() {
        let c = cols();
        let mut q = TableQuery::default();
        q.click_header(&c, "created");
        assert_eq!(q.sort.as_ref().unwrap().dir, SortDir::Asc);
        q.click_header(&c, "created");
        assert_eq!(q.sort.as_ref().unwrap().dir, SortDir::Desc);
        q.click_header(&c, "act");
        assert_eq!(q.sort.as_ref().unwrap().column, "created");
    }

    #[test]
    fn visibility_respects_defaults_pins_and_last_column() {
        let c = cols();
        let mut q = TableQuery::default();
        assert!(
            !q.is_visible(&c[2]),
            "hidden-by-default column starts hidden"
        );
        assert!(q.toggle_column(&c, "gpu"));
        assert!(q.is_visible(&c[2]));
        assert!(!q.toggle_column(&c, "name"), "pinned column cannot hide");

        let only = vec![DataColumnSpec::new("x", "X"), DataColumnSpec::new("y", "Y")];
        let mut q = TableQuery::default();
        assert!(q.toggle_column(&only, "x"));
        assert!(
            !q.toggle_column(&only, "y"),
            "last visible column cannot hide"
        );
    }

    #[test]
    fn persisted_query_omits_filter_text() {
        let mut q = TableQuery::sorted("created", SortDir::Desc);
        q.filter = "secret".into();
        let json = serde_json::to_string(&q).unwrap();
        assert!(!json.contains("secret"));
        let back: TableQuery = serde_json::from_str(&json).unwrap();
        assert_eq!(back.sort, q.sort);
    }
}
