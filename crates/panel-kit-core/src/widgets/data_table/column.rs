//! One column of the table: what it holds, how it sorts, filters and aligns.

use std::cmp::Ordering;

use serde::Deserialize;
use serde::Serialize;

/// Typed per-cell sort key. Rows sort by key, never by rendered markup.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum SortKey {
    /// Text, ordered naturally and case-insensitively (see [`natural_cmp`]).
    Text(String),
    /// Numeric value (counts, scores, epoch milliseconds).
    Num(f64),
    /// No value: sorts after present values, in either direction.
    Missing,
}

impl SortKey {
    /// Text key. Only an absent value sinks; use [`SortKey::Missing`] for it.
    pub fn text(value: impl AsRef<str>) -> Self {
        SortKey::Text(value.as_ref().to_string())
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
    pub(crate) fn order(&self, other: &SortKey) -> Ordering {
        match (self, other) {
            (SortKey::Num(a), SortKey::Num(b)) => a.partial_cmp(b).unwrap_or(Ordering::Equal),
            (SortKey::Text(a), SortKey::Text(b)) => natural_cmp(a, b),
            (SortKey::Num(_), SortKey::Text(_)) => Ordering::Less,
            (SortKey::Text(_), SortKey::Num(_)) => Ordering::Greater,
            (SortKey::Missing, SortKey::Missing) => Ordering::Equal,
            (SortKey::Missing, _) => Ordering::Greater,
            (_, SortKey::Missing) => Ordering::Less,
        }
    }
}

/// Natural order: case-insensitive text, with each run of digits compared as
/// the number it spells, so `gn9` sorts before `gn10` and `GN1` before both.
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let mut left = a.chars().peekable();
    let mut right = b.chars().peekable();
    loop {
        match (left.peek().copied(), right.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let l = digit_run(&mut left);
                let r = digit_run(&mut right);
                let (l, r) = (l.trim_start_matches('0'), r.trim_start_matches('0'));
                match l.len().cmp(&r.len()).then_with(|| l.cmp(r)) {
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

fn digit_run(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> String {
    let mut run = String::new();
    while let Some(c) = chars.peek().copied().filter(char::is_ascii_digit) {
        run.push(c);
        chars.next();
    }
    run
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

impl SortSpec {
    /// One sort key.
    pub fn new(column: impl Into<String>, dir: SortDir) -> Self {
        SortSpec {
            column: column.into(),
            dir,
        }
    }
}

/// What a column holds, which decides its filter control.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ColumnKind {
    /// Free text: a contains filter.
    #[default]
    Text,
    /// One of a few values: a checklist of the values present.
    Enum,
    /// A number: a min/max range.
    Number,
}

/// Horizontal alignment of a column's header and cells.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Align {
    /// Flush left.
    #[default]
    Left,
    /// Flush right (numbers).
    Right,
    /// Centred.
    Center,
}

impl Align {
    /// The CSS class suffix.
    pub fn as_str(self) -> &'static str {
        match self {
            Align::Left => "left",
            Align::Right => "right",
            Align::Center => "center",
        }
    }
}

/// Column declaration for the table.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataColumnSpec {
    /// Stable id used for sort state, filters and visibility persistence.
    pub id: String,
    /// Header text.
    pub title: String,
    /// Whether clicking the header sorts by this column.
    #[serde(default = "yes")]
    pub sortable: bool,
    /// Whether the user may hide this column.
    #[serde(default = "yes")]
    pub hideable: bool,
    /// Hidden until the user shows it.
    #[serde(default)]
    pub hidden_by_default: bool,
    /// Whether the filter row offers a control for this column.
    #[serde(default = "yes")]
    pub filterable: bool,
    /// What the column holds: text, an enum, or a number.
    #[serde(default)]
    pub kind: ColumnKind,
    /// Header and cell alignment.
    #[serde(default)]
    pub align: Align,
    /// Declared CSS width (`"12ch"`); a dragged width overrides it.
    #[serde(default)]
    pub width: Option<String>,
    /// Folds away while the table is narrower than [`NARROW_WIDTH`], unless
    /// the user asked for it in the columns menu.
    ///
    /// [`NARROW_WIDTH`]: super::NARROW_WIDTH
    #[serde(default)]
    pub narrow: bool,
    /// Keeps its left edge while the grid scrolls sideways.
    #[serde(default)]
    pub sticky: bool,
    /// Extra class on the header and every cell.
    #[serde(default)]
    pub class: Option<String>,
    /// The values an enum filter offers; `None` offers the values present.
    #[serde(default)]
    pub options: Option<Vec<String>>,
}

fn yes() -> bool {
    true
}

impl DataColumnSpec {
    /// A sortable, filterable, hideable, initially visible text column.
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            sortable: true,
            hideable: true,
            hidden_by_default: false,
            filterable: true,
            kind: ColumnKind::Text,
            align: Align::Left,
            width: None,
            narrow: false,
            sticky: false,
            class: None,
            options: None,
        }
    }

    /// Column that cannot be hidden (typically the identity column).
    pub fn pinned(mut self) -> Self {
        self.hideable = false;
        self
    }

    /// Column that does not sort.
    pub fn unsorted(mut self) -> Self {
        self.sortable = false;
        self
    }

    /// Column that starts hidden.
    pub fn hidden(mut self) -> Self {
        self.hidden_by_default = true;
        self
    }

    /// Column the filter row leaves alone.
    pub fn unfiltered(mut self) -> Self {
        self.filterable = false;
        self
    }

    /// Enum column: filtered by a checklist of the values present.
    pub fn enumerated(mut self) -> Self {
        self.kind = ColumnKind::Enum;
        self
    }

    /// Enum column offering exactly these values.
    pub fn options(mut self, options: &[&str]) -> Self {
        self.kind = ColumnKind::Enum;
        self.options = Some(options.iter().map(|o| o.to_string()).collect());
        self
    }

    /// Numeric column: filtered by a min/max range.
    pub fn number(mut self) -> Self {
        self.kind = ColumnKind::Number;
        self
    }

    /// Alignment.
    pub fn align(mut self, align: Align) -> Self {
        self.align = align;
        self
    }

    /// Declared CSS width, e.g. `"12ch"`.
    pub fn width(mut self, width: impl Into<String>) -> Self {
        self.width = Some(width.into());
        self
    }

    /// Folds away on a narrow table.
    pub fn narrow(mut self) -> Self {
        self.narrow = true;
        self
    }

    /// Keeps its left edge while the grid scrolls sideways.
    pub fn sticky(mut self) -> Self {
        self.sticky = true;
        self
    }

    /// Extra class on the header and every cell (`"pk-dt-col-title"` takes
    /// the slack width).
    pub fn class(mut self, class: impl Into<String>) -> Self {
        self.class = Some(class.into());
        self
    }
}
