//! The rows a table holds, stored by column and built once.
//!
//! Text columns, ids and row classes live in one buffer each, enum columns as
//! a code per row into a small dictionary, number columns as an `f64` (NaN for
//! none), and tags as interned ids. Building a million rows allocates per
//! column, not per row. Sort order is derived lazily, once per column, so
//! re-sorting compares integers rather than strings. Clones share storage.

use std::borrow::Cow;
use std::cell::OnceCell;
use std::collections::HashMap;
use std::rc::Rc;

use super::column::{natural_cmp, ColumnKind, DataColumnSpec};
use super::tags::canonical_tag;

/// One cell's value as a host hands it over.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Value<'a> {
    /// Text.
    Text(&'a str),
    /// A number.
    Number(f64),
    /// No value: sorts last in either direction and fails every filter.
    Missing,
}

impl<'a> From<&'a str> for Value<'a> {
    fn from(text: &'a str) -> Self {
        Value::Text(text)
    }
}

impl<'a> From<&'a String> for Value<'a> {
    fn from(text: &'a String) -> Self {
        Value::Text(text)
    }
}

impl From<f64> for Value<'_> {
    fn from(number: f64) -> Self {
        Value::Number(number)
    }
}

impl<'a, T: Into<Value<'a>>> From<Option<T>> for Value<'a> {
    fn from(value: Option<T>) -> Self {
        value.map(Into::into).unwrap_or(Value::Missing)
    }
}

/// Code of an enum cell with no value; rank of a text cell with none.
const NONE: u32 = u32::MAX;

/// Many strings, possibly absent, in one buffer.
#[derive(Default)]
struct Strings {
    bytes: String,
    ends: Vec<u32>,
    present: Vec<bool>,
}

impl Strings {
    fn with_capacity(rows: usize) -> Self {
        Strings {
            bytes: String::new(),
            ends: Vec::with_capacity(rows),
            present: Vec::with_capacity(rows),
        }
    }

    fn push(&mut self, text: Option<&str>) {
        if let Some(text) = text {
            self.bytes.push_str(text);
        }
        self.ends.push(self.bytes.len() as u32);
        self.present.push(text.is_some());
    }

    /// Replace the last string, which must be the most recently pushed.
    fn set_last(&mut self, text: &str) {
        let start = self.start(self.ends.len() - 1);
        self.bytes.truncate(start);
        self.bytes.push_str(text);
        *self.ends.last_mut().expect("a string was pushed") = self.bytes.len() as u32;
        *self.present.last_mut().expect("a string was pushed") = true;
    }

    fn start(&self, at: usize) -> usize {
        if at == 0 {
            0
        } else {
            self.ends[at - 1] as usize
        }
    }

    fn get(&self, at: usize) -> Option<&str> {
        self.present[at].then(|| &self.bytes[self.start(at)..self.ends[at] as usize])
    }

    fn len(&self) -> usize {
        self.ends.len()
    }
}

enum Column {
    Text {
        text: Strings,
        rank: OnceCell<Box<[u32]>>,
    },
    Enum {
        codes: Vec<u32>,
        dict: Vec<Box<str>>,
        rank: OnceCell<Box<[u32]>>,
    },
    Number {
        values: Vec<f64>,
    },
}

struct Store {
    ids: Strings,
    columns: Vec<Column>,
    /// Every row's tag ids, back to back; `tag_ends[row]` closes a row's run.
    tag_ids: Vec<u32>,
    tag_ends: Vec<u32>,
    tag_names: Vec<Box<str>>,
    classes: Strings,
}

/// A table's rows, by column. Cheap to clone; equality is identity, so a
/// painter re-derives its view only when it is handed new data.
#[derive(Clone)]
pub struct TableData(Rc<Store>);

impl PartialEq for TableData {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl std::fmt::Debug for TableData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TableData")
            .field("rows", &self.len())
            .field("columns", &self.0.columns.len())
            .finish()
    }
}

impl TableData {
    /// Start a table of `columns`; each column's kind decides its storage.
    pub fn builder(columns: &[DataColumnSpec]) -> TableDataBuilder {
        TableDataBuilder::with_capacity(columns, 0)
    }

    /// Rows held.
    pub fn len(&self) -> usize {
        self.0.ids.len()
    }

    /// Whether there are no rows.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Columns held.
    pub fn width(&self) -> usize {
        self.0.columns.len()
    }

    /// A row's id.
    pub fn id(&self, row: usize) -> &str {
        self.0.ids.get(row).unwrap_or("")
    }

    /// The row with `id`, by scanning.
    pub fn row_of(&self, id: &str) -> Option<usize> {
        (0..self.len()).find(|&row| self.id(row) == id)
    }

    /// A row's extra classes.
    pub fn class(&self, row: usize) -> Option<&str> {
        self.0.classes.get(row)
    }

    /// A cell as plain text: what filters match, groups are named by, and
    /// exports write. Missing is empty.
    pub fn text(&self, row: usize, column: usize) -> Cow<'_, str> {
        match &self.0.columns[column] {
            Column::Text { text, .. } => Cow::Borrowed(text.get(row).unwrap_or("")),
            Column::Enum { codes, dict, .. } => match codes[row] {
                NONE => Cow::Borrowed(""),
                code => Cow::Borrowed(&dict[code as usize]),
            },
            Column::Number { values } => {
                let mut out = String::new();
                write_number(&mut out, values[row]);
                Cow::Owned(out)
            }
        }
    }

    /// Read a cell's text without allocating, numbers included.
    pub(crate) fn with_text<R>(
        &self,
        row: usize,
        column: usize,
        read: impl FnOnce(&str) -> R,
    ) -> R {
        match &self.0.columns[column] {
            Column::Number { values } => {
                let mut text = NumberText {
                    bytes: [0; 40],
                    len: 0,
                };
                write_number(&mut text, values[row]);
                read(std::str::from_utf8(&text.bytes[..text.len]).unwrap_or(""))
            }
            _ => read(&self.text(row, column)),
        }
    }

    /// A cell's number, when its column holds numbers or its text parses.
    pub fn number(&self, row: usize, column: usize) -> Option<f64> {
        match &self.0.columns[column] {
            Column::Number { values } => Some(values[row]).filter(|n| !n.is_nan()),
            _ => self.text(row, column).trim().parse().ok(),
        }
    }

    /// Whether a cell has no value.
    pub fn is_missing(&self, row: usize, column: usize) -> bool {
        match &self.0.columns[column] {
            Column::Text { text, .. } => text.get(row).is_none(),
            Column::Enum { codes, .. } => codes[row] == NONE,
            Column::Number { values } => values[row].is_nan(),
        }
    }

    /// A row's tags, canonical and in alphabetical order.
    pub fn tags(&self, row: usize) -> impl Iterator<Item = &str> + '_ {
        self.tag_ids(row)
            .iter()
            .map(|&id| &*self.0.tag_names[id as usize])
    }

    /// Whether any row carries a tag.
    pub fn tagged(&self) -> bool {
        !self.0.tag_names.is_empty()
    }

    /// The interned id of a canonical tag.
    pub(crate) fn tag_id(&self, tag: &str) -> Option<u32> {
        self.0
            .tag_names
            .binary_search_by(|name| (**name).cmp(tag))
            .ok()
            .map(|at| at as u32)
    }

    /// A row's interned tag ids, ascending.
    pub(crate) fn tag_ids(&self, row: usize) -> &[u32] {
        let start = if row == 0 {
            0
        } else {
            self.0.tag_ends[row - 1] as usize
        };
        &self.0.tag_ids[start..self.0.tag_ends[row] as usize]
    }

    /// The column's distinct values when it is an enum.
    pub(crate) fn dictionary(&self, column: usize) -> Option<&[Box<str>]> {
        match &self.0.columns[column] {
            Column::Enum { dict, .. } => Some(dict),
            _ => None,
        }
    }

    /// An enum cell's code.
    pub(crate) fn code(&self, row: usize, column: usize) -> Option<u32> {
        match &self.0.columns[column] {
            Column::Enum { codes, .. } => Some(codes[row]).filter(|&c| c != NONE),
            _ => None,
        }
    }

    /// What a sort compares for a column: `u32` ranks for text and enums
    /// (natural order, computed on first use), the value for numbers.
    pub(crate) fn order(&self, column: usize) -> Order<'_> {
        match &self.0.columns[column] {
            Column::Text { text, rank } => Order::Rank(rank.get_or_init(|| text_rank(text))),
            Column::Enum { codes, dict, rank } => {
                let by_code = rank.get_or_init(|| dict_rank(dict));
                Order::Code {
                    codes,
                    rank: by_code,
                }
            }
            Column::Number { values } => Order::Number(values),
        }
    }
}

/// How one column orders rows.
pub(crate) enum Order<'a> {
    /// A rank per row; `u32::MAX` is missing.
    Rank(&'a [u32]),
    /// A code per row and a rank per code.
    Code { codes: &'a [u32], rank: &'a [u32] },
    /// The value per row; NaN is missing.
    Number(&'a [f64]),
}

impl Order<'_> {
    /// The row's position in ascending order, or `None` when it has no value.
    #[inline]
    pub(crate) fn key(&self, row: usize) -> Option<f64> {
        match self {
            Order::Rank(rank) => Some(rank[row]).filter(|&r| r != NONE).map(f64::from),
            Order::Code { codes, rank } => match codes[row] {
                NONE => None,
                code => Some(f64::from(rank[code as usize])),
            },
            Order::Number(values) => Some(values[row]).filter(|n| !n.is_nan()),
        }
    }
}

/// Natural-order ranks for a text column. Rows sort first on a packed key
/// whose integer order agrees with [`natural_cmp`] wherever keys differ.
/// Within a tie, identical strings are grouped by a byte sort first, so only
/// the distinct values are compared naturally.
fn text_rank(text: &Strings) -> Box<[u32]> {
    let at = |r: u32| text.get(r as usize).unwrap_or("");
    let mut keyed: Vec<(u64, u64, u32)> = (0..text.len() as u32)
        .filter(|&r| text.get(r as usize).is_some())
        .map(|r| {
            let (head, number) = natural_key(at(r));
            (head, number, r)
        })
        .collect();
    keyed.sort_unstable();
    let mut rank = vec![NONE; text.len()];
    let mut next = 0u32;
    for run in keyed.chunk_by_mut(|a, b| (a.0, a.1) == (b.0, b.1)) {
        // Stable, so identical strings keep row order.
        run.sort_by(|a, b| at(a.2).cmp(at(b.2)));
        let mut blocks: Vec<&[(u64, u64, u32)]> = run.chunk_by(|a, b| at(a.2) == at(b.2)).collect();
        blocks.sort_by(|a, b| natural_cmp(at(a[0].2), at(b[0].2)));
        let mut previous: Option<&str> = None;
        let mut current = next;
        for block in blocks {
            let value = at(block[0].2);
            if previous.is_some_and(|p| natural_cmp(p, value).is_ne()) {
                current = next;
            }
            for &(_, _, row) in block {
                rank[row as usize] = current;
            }
            next += block.len() as u32;
            previous = Some(value);
        }
    }
    rank.into_boxed_slice()
}

/// A packed prefix of `text` whose order agrees with [`natural_cmp`] wherever
/// two keys differ: up to eight lowercase ASCII bytes before the first digit
/// or non-ASCII byte (padded so a digit sorts as `'0'`, a non-ASCII character
/// above every ASCII one, and the end below everything), then the value of
/// the digit run that stopped it.
fn natural_key(text: &str) -> (u64, u64) {
    let bytes = text.as_bytes();
    let mut head = [0u8; 8];
    let mut at = 0;
    let mut number = 0u64;
    while at < 8 {
        match bytes.get(at) {
            None => break,
            Some(b) if b.is_ascii_digit() => {
                head[at..].fill(b'0');
                let digits = &text[at..];
                let end = digits
                    .find(|c: char| !c.is_ascii_digit())
                    .unwrap_or(digits.len());
                let digits = digits[..end].trim_start_matches('0');
                number = if digits.len() > 19 {
                    u64::MAX
                } else {
                    digits.parse().unwrap_or(0)
                };
                break;
            }
            Some(b) if !b.is_ascii() => {
                head[at..].fill(0xFF);
                break;
            }
            Some(b) => head[at] = b.to_ascii_lowercase(),
        }
        at += 1;
    }
    (u64::from_be_bytes(head), number)
}

fn dict_rank(dict: &[Box<str>]) -> Box<[u32]> {
    let mut order: Vec<usize> = (0..dict.len()).collect();
    order.sort_by(|&a, &b| natural_cmp(&dict[a], &dict[b]));
    let mut rank = vec![0u32; dict.len()];
    let mut current = 0u32;
    for (i, &code) in order.iter().enumerate() {
        if i > 0 && natural_cmp(&dict[order[i - 1]], &dict[code]).is_ne() {
            current = i as u32;
        }
        rank[code] = current;
    }
    rank.into_boxed_slice()
}

/// A number as its text: integers without a fraction.
pub(crate) fn write_number(out: &mut impl std::fmt::Write, value: f64) {
    if value.is_nan() {
        return;
    }
    if value.fract() == 0.0 && value.abs() < 1e15 {
        let _ = write!(out, "{}", value as i64);
    } else {
        let _ = write!(out, "{value}");
    }
}

/// A number's text on the stack: no allocation per row while filtering.
struct NumberText {
    bytes: [u8; 40],
    len: usize,
}

impl std::fmt::Write for NumberText {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        let end = self.len + s.len();
        let slot = self.bytes.get_mut(self.len..end).ok_or(std::fmt::Error)?;
        slot.copy_from_slice(s.as_bytes());
        self.len = end;
        Ok(())
    }
}

enum Building {
    Text(Strings),
    Enum {
        codes: Vec<u32>,
        dict: Vec<Box<str>>,
        index: HashMap<Box<str>, u32>,
    },
    Number(Vec<f64>),
}

/// Appends rows; [`TableDataBuilder::build`] seals them.
pub struct TableDataBuilder {
    ids: Strings,
    columns: Vec<Building>,
    tag_ids: Vec<u32>,
    tag_ends: Vec<u32>,
    tag_index: HashMap<String, u32>,
    classes: Strings,
    number: String,
}

impl TableDataBuilder {
    /// A builder expecting about `rows` rows.
    pub fn with_capacity(columns: &[DataColumnSpec], rows: usize) -> Self {
        let columns = columns
            .iter()
            .map(|column| match column.kind {
                ColumnKind::Text => Building::Text(Strings::with_capacity(rows)),
                ColumnKind::Enum => Building::Enum {
                    codes: Vec::with_capacity(rows),
                    dict: Vec::new(),
                    index: HashMap::new(),
                },
                ColumnKind::Number => Building::Number(Vec::with_capacity(rows)),
            })
            .collect();
        TableDataBuilder {
            ids: Strings::with_capacity(rows),
            columns,
            tag_ids: Vec::new(),
            tag_ends: Vec::with_capacity(rows),
            tag_index: HashMap::new(),
            classes: Strings::with_capacity(rows),
            number: String::new(),
        }
    }

    /// Append a row with `id` and one value per declared column, in order;
    /// missing trailing values are missing cells.
    pub fn row<'a>(
        &mut self,
        id: &str,
        values: impl IntoIterator<Item = Value<'a>>,
    ) -> RowWriter<'_> {
        let mut values = values.into_iter();
        for column in &mut self.columns {
            push(
                column,
                values.next().unwrap_or(Value::Missing),
                &mut self.number,
            );
        }
        self.ids.push(Some(id));
        self.tag_ends.push(self.tag_ids.len() as u32);
        self.classes.push(None);
        RowWriter { builder: self }
    }

    /// Seal the rows. Tag ids are renumbered alphabetically, so a row's ids in
    /// ascending order are its tags in alphabetical order.
    pub fn build(self) -> TableData {
        let mut names: Vec<(String, u32)> = self.tag_index.into_iter().collect();
        names.sort();
        let mut remap = vec![0u32; names.len()];
        for (new, (_, old)) in names.iter().enumerate() {
            remap[*old as usize] = new as u32;
        }
        let mut tag_ids: Vec<u32> = self.tag_ids.iter().map(|&id| remap[id as usize]).collect();
        let mut start = 0usize;
        for &end in &self.tag_ends {
            tag_ids[start..end as usize].sort_unstable();
            start = end as usize;
        }
        let columns = self
            .columns
            .into_iter()
            .map(|column| match column {
                Building::Text(text) => Column::Text {
                    text,
                    rank: OnceCell::new(),
                },
                Building::Enum { codes, dict, .. } => Column::Enum {
                    codes,
                    dict,
                    rank: OnceCell::new(),
                },
                Building::Number(values) => Column::Number { values },
            })
            .collect();
        TableData(Rc::new(Store {
            ids: self.ids,
            columns,
            tag_ids,
            tag_ends: self.tag_ends,
            tag_names: names
                .into_iter()
                .map(|(name, _)| name.into_boxed_str())
                .collect(),
            classes: self.classes,
        }))
    }
}

fn push(column: &mut Building, value: Value<'_>, scratch: &mut String) {
    let number_text = |scratch: &mut String, n: f64| {
        scratch.clear();
        write_number(scratch, n);
    };
    match column {
        Building::Text(text) => match value {
            Value::Text(t) => text.push(Some(t)),
            Value::Number(n) => {
                number_text(scratch, n);
                text.push(Some(scratch));
            }
            Value::Missing => text.push(None),
        },
        Building::Number(values) => values.push(match value {
            Value::Number(n) if n.is_finite() => n,
            Value::Text(t) => t
                .trim()
                .parse()
                .ok()
                .filter(|n: &f64| n.is_finite())
                .unwrap_or(f64::NAN),
            _ => f64::NAN,
        }),
        Building::Enum { codes, dict, index } => codes.push(match value {
            Value::Missing => NONE,
            Value::Text(t) => intern(dict, index, t),
            Value::Number(n) => {
                number_text(scratch, n);
                intern(dict, index, scratch)
            }
        }),
    }
}

fn intern(dict: &mut Vec<Box<str>>, index: &mut HashMap<Box<str>, u32>, text: &str) -> u32 {
    if let Some(&code) = index.get(text) {
        return code;
    }
    let code = dict.len() as u32;
    dict.push(text.into());
    index.insert(text.into(), code);
    code
}

/// The row just appended, for its tags and classes.
pub struct RowWriter<'a> {
    builder: &'a mut TableDataBuilder,
}

impl RowWriter<'_> {
    /// The row's tags; blanks and repeats drop, case folds.
    pub fn tags<S: AsRef<str>>(self, tags: &[S]) -> Self {
        let builder = &mut *self.builder;
        let start = builder
            .tag_ends
            .len()
            .checked_sub(2)
            .map_or(0, |at| builder.tag_ends[at] as usize);
        builder.tag_ids.truncate(start);
        for tag in tags {
            let tag = canonical_tag(tag.as_ref());
            if tag.is_empty() {
                continue;
            }
            let next = builder.tag_index.len() as u32;
            let id = *builder.tag_index.entry(tag).or_insert(next);
            if !builder.tag_ids[start..].contains(&id) {
                builder.tag_ids.push(id);
            }
        }
        *builder.tag_ends.last_mut().expect("a row was appended") = builder.tag_ids.len() as u32;
        self
    }

    /// Extra classes on the row (`"pk-dt-muted"` dims a finished one).
    pub fn class(self, class: &str) -> Self {
        self.builder.classes.set_last(class);
        self
    }
}
