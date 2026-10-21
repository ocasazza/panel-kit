//! One column of the table: what it holds, how it sorts, filters and aligns.

use std::cmp::Ordering;

use serde::Deserialize;
use serde::Serialize;

/// Natural order: case-insensitive text, with each run of digits compared as
/// the number it spells, so `gn9` sorts before `gn10` and `GN1` before both.
/// Allocation-free; ASCII is compared byte by byte, anything else by char.
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let (x, y) = (a.as_bytes(), b.as_bytes());
    let (mut i, mut j) = (0, 0);
    loop {
        match (x.get(i).copied(), y.get(j).copied()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(l), Some(r)) if l.is_ascii_digit() && r.is_ascii_digit() => {
                let (start_i, start_j) = (i, j);
                while x.get(i).is_some_and(u8::is_ascii_digit) {
                    i += 1;
                }
                while y.get(j).is_some_and(u8::is_ascii_digit) {
                    j += 1;
                }
                match digits_cmp(&a[start_i..i], &b[start_j..j]) {
                    Ordering::Equal => {}
                    order => return order,
                }
            }
            (Some(l), Some(r)) if l.is_ascii() && r.is_ascii() => {
                match l.to_ascii_lowercase().cmp(&r.to_ascii_lowercase()) {
                    Ordering::Equal => {}
                    order => return order,
                }
                i += 1;
                j += 1;
            }
            _ => return unicode_cmp(&a[i..], &b[j..]),
        }
    }
}

/// Two digit runs as the numbers they spell, of any length.
fn digits_cmp(l: &str, r: &str) -> Ordering {
    let (l, r) = (l.trim_start_matches('0'), r.trim_start_matches('0'));
    l.len().cmp(&r.len()).then_with(|| l.cmp(r))
}

/// The char-by-char tail, once either side leaves ASCII.
fn unicode_cmp(a: &str, b: &str) -> Ordering {
    let (mut a, mut b) = (a, b);
    loop {
        let (Some(l), Some(r)) = (a.chars().next(), b.chars().next()) else {
            return a.is_empty().cmp(&b.is_empty()).reverse();
        };
        if l.is_ascii_digit() && r.is_ascii_digit() {
            let end_l = a.find(|c: char| !c.is_ascii_digit()).unwrap_or(a.len());
            let end_r = b.find(|c: char| !c.is_ascii_digit()).unwrap_or(b.len());
            match digits_cmp(&a[..end_l], &b[..end_r]) {
                Ordering::Equal => {}
                order => return order,
            }
            a = &a[end_l..];
            b = &b[end_r..];
            continue;
        }
        match l.to_lowercase().cmp(r.to_lowercase()) {
            Ordering::Equal => {}
            order => return order,
        }
        a = &a[l.len_utf8()..];
        b = &b[r.len_utf8()..];
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
