//! The rows a table holds, stored by column and built once.
//!
//! Every column is a spine of fixed-size chunks of rows: text and ids and row
//! classes in one buffer per chunk, enum columns as a code per row into a small
//! dictionary, number columns as an `f64` (NaN for none), and tags as interned
//! ids. Building a million rows allocates per chunk, not per row. Sort order is
//! derived lazily, once per column, so re-sorting compares integers rather than
//! strings, and a text chunk keeps a folded copy of its non-ASCII rows once the
//! quick filter has searched it. Clones share storage, and so does
//! [`TableData::append`]: the new table shares every full chunk with the old one
//! and copies only the last.

use std::borrow::Cow;
use std::cell::{Cell, OnceCell, RefCell};
use std::collections::HashMap;
use std::mem::size_of;
use std::ops::Range;
use std::rc::{Rc, Weak};

use super::column::{natural_cmp, ColumnKind, DataColumnSpec};
use super::scan::{mark_rows, next_non_ascii, row_of, Needle, RowSet};
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

/// Rows per chunk: a row lives in chunk `row >> CHUNK_BITS`. Every chunk but a
/// column's last is full, so appending copies at most one chunk.
const CHUNK_BITS: u32 = 16;
pub(super) const CHUNK_ROWS: usize = 1 << CHUNK_BITS;
const CHUNK_MASK: usize = CHUNK_ROWS - 1;

/// A chunk's offset into its own buffer.
fn offset(len: usize) -> u32 {
    u32::try_from(len).expect("a chunk of a table column holds under 4 GiB")
}

/// What a column keeps per chunk.
pub(crate) trait Chunk: Clone + Default {
    fn with_capacity(rows: usize) -> Self;
    /// An empty chunk for `rows` rows that look like this one's.
    fn following(&self, rows: usize) -> Self {
        Self::with_capacity(rows)
    }
    /// Heap bytes held.
    fn heap(&self) -> usize;
}

/// Up to `CHUNK_ROWS` strings, possibly absent, in one buffer.
#[derive(Clone, Default)]
struct Strings {
    bytes: String,
    ends: Vec<u32>,
    present: Vec<bool>,
    /// The rows holding non-ASCII, folded; made by the first search.
    folded: OnceCell<Rc<Folded>>,
}

/// The rows of a chunk that hold a byte above ASCII, each folded as the
/// quick filter folds text, back to back.
#[derive(Default)]
struct Folded {
    rows: Vec<u16>,
    bytes: Vec<u8>,
    /// `ends[i]` closes the run of `rows[i]` within `bytes`.
    ends: Vec<u32>,
}

impl Chunk for Strings {
    fn with_capacity(rows: usize) -> Self {
        Strings {
            bytes: String::new(),
            ends: Vec::with_capacity(rows),
            present: Vec::with_capacity(rows),
            folded: OnceCell::new(),
        }
    }

    fn following(&self, rows: usize) -> Self {
        let mut next = Self::with_capacity(rows);
        let expected = self.bytes.len() as u64 * rows as u64 / self.ends.len().max(1) as u64;
        next.bytes
            .reserve_exact(expected as usize + expected as usize / 8);
        next
    }

    fn heap(&self) -> usize {
        let folded = self.folded.get().map_or(0, |f| {
            f.rows.capacity() * size_of::<u16>()
                + f.bytes.capacity()
                + f.ends.capacity() * size_of::<u32>()
        });
        self.bytes.capacity()
            + self.ends.capacity() * size_of::<u32>()
            + self.present.capacity()
            + folded
    }
}

impl Strings {
    fn push(&mut self, text: Option<&str>) {
        self.folded.take();
        if let Some(text) = text {
            self.bytes.push_str(text);
        }
        self.ends.push(offset(self.bytes.len()));
        self.present.push(text.is_some());
    }

    /// Replace the last string, which must be the most recently pushed.
    fn set_last(&mut self, text: &str) {
        self.folded.take();
        let start = self.start(self.ends.len() - 1);
        self.bytes.truncate(start);
        self.bytes.push_str(text);
        *self.ends.last_mut().expect("a string was pushed") = offset(self.bytes.len());
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

    fn folded(&self) -> &Folded {
        self.folded.get_or_init(|| {
            let mut folded = Folded::default();
            let (mut at, mut row) = (0, 0);
            while let Some(found) = next_non_ascii(self.bytes.as_bytes(), at) {
                row = row_of(&self.ends, row, found);
                at = self.ends[row] as usize;
                folded.rows.push(row as u16);
                fold_into(&mut folded.bytes, &self.bytes[self.start(row)..at]);
                folded.ends.push(offset(folded.bytes.len()));
            }
            Rc::new(folded)
        })
    }

    /// Marks in `out` (bit `row`) the rows whose text, folded, holds `needle`:
    /// the raw text answers for ASCII, the folded copy for every other row, as
    /// a char there may fold to ASCII (`İ`, the Kelvin sign).
    fn find(&self, needle: &Needle, out: &mut [u64]) {
        if needle.is_ascii() {
            mark_rows(
                self.bytes.as_bytes(),
                &self.ends,
                None,
                needle.bytes(),
                true,
                out,
            );
        }
        let folded = self.folded();
        mark_rows(
            &folded.bytes,
            &folded.ends,
            Some(&folded.rows),
            needle.bytes(),
            false,
            out,
        );
    }
}

impl Chunk for Vec<u32> {
    fn with_capacity(rows: usize) -> Self {
        Vec::with_capacity(rows)
    }

    fn heap(&self) -> usize {
        self.capacity() * size_of::<u32>()
    }
}

impl Chunk for Vec<f64> {
    fn with_capacity(rows: usize) -> Self {
        Vec::with_capacity(rows)
    }

    fn heap(&self) -> usize {
        self.capacity() * size_of::<f64>()
    }
}

/// Up to `CHUNK_ROWS` rows' tag ids, back to back; `ends[row]` closes a
/// row's run within `ids`.
#[derive(Clone, Default)]
struct TagRuns {
    ids: Vec<u32>,
    ends: Vec<u32>,
}

impl Chunk for TagRuns {
    fn with_capacity(rows: usize) -> Self {
        TagRuns {
            ids: Vec::new(),
            ends: Vec::with_capacity(rows),
        }
    }

    fn heap(&self) -> usize {
        (self.ids.capacity() + self.ends.capacity()) * size_of::<u32>()
    }
}

impl TagRuns {
    fn start(&self, at: usize) -> usize {
        if at == 0 {
            0
        } else {
            self.ends[at - 1] as usize
        }
    }

    fn of(&self, at: usize) -> &[u32] {
        &self.ids[self.start(at)..self.ends[at] as usize]
    }
}

/// Every tag a table carries, by id. Ids are numbered as tags are first met
/// and never change, so tables appended to one another share them.
#[derive(Default)]
struct TagNames {
    names: Vec<Box<str>>,
    /// Ids ordered by name.
    by_name: Vec<u32>,
}

impl TagNames {
    fn id(&self, tag: &str) -> Option<u32> {
        let at = self
            .by_name
            .binary_search_by(|&id| (*self.names[id as usize]).cmp(tag))
            .ok()?;
        Some(self.by_name[at])
    }

    /// `base`'s tags, then `added` (by id from `base`'s count).
    fn extended(base: Rc<TagNames>, added: HashMap<String, u32>) -> Rc<TagNames> {
        if added.is_empty() {
            return base;
        }
        let mut fresh: Vec<(u32, String)> =
            added.into_iter().map(|(name, id)| (id, name)).collect();
        fresh.sort_unstable();
        let mut names = base.names.clone();
        names.extend(fresh.into_iter().map(|(_, name)| name.into_boxed_str()));
        let mut by_name: Vec<u32> = (0..names.len() as u32).collect();
        by_name.sort_unstable_by(|&a, &b| names[a as usize].cmp(&names[b as usize]));
        Rc::new(TagNames { names, by_name })
    }

    fn heap(&self) -> usize {
        self.names
            .iter()
            .map(|name| name.len() + size_of::<Box<str>>())
            .sum::<usize>()
            + self.by_name.len() * size_of::<u32>()
    }
}

/// One column's chunks. Tables appended to one another share the full ones.
pub(crate) struct Spine<T>(Vec<Rc<T>>);

impl<T> Clone for Spine<T> {
    fn clone(&self) -> Self {
        Spine(self.0.clone())
    }
}

impl<T> Spine<T> {
    /// The chunk holding `row`, and the row's place in it.
    #[inline]
    fn chunk(&self, row: usize) -> (&T, usize) {
        (&self.0[row >> CHUNK_BITS], row & CHUNK_MASK)
    }
}

impl<T: Chunk> Spine<T> {
    fn heap(&self) -> usize {
        self.0.capacity() * size_of::<Rc<T>>()
            + self
                .0
                .iter()
                .map(|chunk| size_of::<T>() + chunk.heap())
                .sum::<usize>()
    }
}

impl Spine<Strings> {
    #[inline]
    fn get(&self, row: usize) -> Option<&str> {
        let (chunk, at) = self.chunk(row);
        chunk.get(at)
    }
}

impl<E> std::ops::Index<usize> for Spine<Vec<E>> {
    type Output = E;

    #[inline]
    fn index(&self, row: usize) -> &E {
        let (chunk, at) = self.chunk(row);
        &chunk[at]
    }
}

impl<E: Copy> Spine<Vec<E>> {
    #[inline]
    fn at(&self, row: usize) -> E {
        self[row]
    }
}

/// An enum column's distinct values by code. Shared by every table appended
/// to one another until a value is new.
struct Dict {
    names: Vec<Box<str>>,
    /// Code by value, made on the first append.
    index: OnceCell<HashMap<Box<str>, u32>>,
    /// Rank by code, made on the first sort.
    rank: OnceCell<Box<[u32]>>,
}

impl Dict {
    fn new(names: Vec<Box<str>>) -> Rc<Dict> {
        Rc::new(Dict {
            names,
            index: OnceCell::new(),
            rank: OnceCell::new(),
        })
    }

    fn index(&self) -> &HashMap<Box<str>, u32> {
        self.index.get_or_init(|| {
            self.names
                .iter()
                .enumerate()
                .map(|(code, name)| (name.clone(), code as u32))
                .collect()
        })
    }

    fn heap(&self) -> usize {
        let names: usize = self
            .names
            .iter()
            .map(|n| n.len() + size_of::<Box<str>>())
            .sum();
        let index = self.index.get().map_or(0, |index| {
            index
                .keys()
                .map(|n| n.len() + size_of::<Box<str>>() + size_of::<u32>())
                .sum()
        });
        names + index + self.rank.get().map_or(0, |r| r.len() * size_of::<u32>())
    }
}

/// A text column's natural-order ranks: `ranks[row]` is the number of rows
/// whose value sorts strictly before the row's (`NONE` when it has none).
struct RankInfo {
    ranks: Box<[u32]>,
    /// One row per distinct value, in ascending order.
    reps: Box<[u32]>,
    /// Rows holding a value.
    present: u32,
}

impl RankInfo {
    fn heap(&self) -> usize {
        (self.ranks.len() + self.reps.len()) * size_of::<u32>()
    }
}

/// A text column's ranks: derived on first use, from its nearest ancestor's
/// when the table was appended to one that had them.
struct Ranks {
    done: OnceCell<Rc<RankInfo>>,
    base: RefCell<Option<Rc<RankInfo>>>,
}

impl Ranks {
    fn new(base: Option<Rc<RankInfo>>) -> Self {
        Ranks {
            done: OnceCell::new(),
            base: RefCell::new(base),
        }
    }

    fn get(&self, text: &Spine<Strings>, rows: usize) -> &RankInfo {
        self.done.get_or_init(|| {
            Rc::new(match self.base.borrow_mut().take() {
                Some(base) => carry(&base, text, rows),
                None => cold(text, rows),
            })
        })
    }

    /// The ranks a table appended to this one starts from.
    fn carry_source(&self) -> Option<Rc<RankInfo>> {
        self.done
            .get()
            .cloned()
            .or_else(|| self.base.borrow().clone())
    }

    fn heap(&self) -> usize {
        self.done.get().map_or(0, |r| r.heap())
            + self.base.borrow().as_ref().map_or(0, |r| r.heap())
    }
}

enum Column {
    Text {
        text: Spine<Strings>,
        rank: Ranks,
    },
    Enum {
        codes: Spine<Vec<u32>>,
        dict: Rc<Dict>,
    },
    Number {
        values: Spine<Vec<f64>>,
    },
}

/// A line of appends. Tables on one branch are prefixes of its longest; a
/// table appended to after a longer one exists starts a branch of its own,
/// remembering the rows it shares with its parent.
struct Branch {
    parent: Option<(Rc<Branch>, usize)>,
    /// Rows of the longest table on this branch.
    head: Cell<usize>,
}

struct Store {
    rows: usize,
    ids: Spine<Strings>,
    columns: Vec<Column>,
    tags: Spine<TagRuns>,
    tag_names: Rc<TagNames>,
    classes: Spine<Strings>,
    branch: Rc<Branch>,
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

/// A table named without being held: a record of which table it was does
/// not keep the rows alive. See [`TableData::downgrade`].
#[derive(Clone)]
pub(super) struct WeakData(Weak<Store>);

impl WeakData {
    /// Whether this names `data`. The allocation outlives its rows while it
    /// is named, so an address is never taken by another table.
    pub(super) fn is(&self, data: &TableData) -> bool {
        std::ptr::eq(self.0.as_ptr(), Rc::as_ptr(&data.0))
    }
}

impl TableData {
    /// Start a table of `columns`; each column's kind decides its storage.
    pub fn builder(columns: &[DataColumnSpec]) -> TableDataBuilder {
        TableDataBuilder::with_capacity(columns, 0)
    }

    /// This table's rows followed by the rows `build` pushes, as a new table.
    /// The old table is unchanged and the two share their full chunks, so the
    /// cost is the rows pushed plus one chunk, however many rows there are.
    /// A column's cached ranks carry over without a re-sort. An enum column
    /// that meets a new value copies its dictionary.
    pub fn append(&self, build: impl FnOnce(&mut TableDataBuilder)) -> TableData {
        let mut builder = TableDataBuilder::extending(self);
        build(&mut builder);
        if builder.rows == self.len() {
            return self.clone();
        }
        builder.build()
    }

    /// Whether `earlier` is this table or one it was appended from, so that
    /// its rows are this table's first rows. Tables built apart are never
    /// related, whatever they hold. Constant time unless appends forked.
    pub fn extends(&self, earlier: &TableData) -> bool {
        let wanted = earlier.len();
        let mut branch = &self.0.branch;
        let mut shared = self.len();
        loop {
            if Rc::ptr_eq(branch, &earlier.0.branch) {
                return shared >= wanted;
            }
            match &branch.parent {
                Some((parent, fork)) => {
                    shared = shared.min(*fork);
                    branch = parent;
                }
                None => return false,
            }
        }
    }

    /// This table without its rows: what a record keeps to tell it again.
    pub(super) fn downgrade(&self) -> WeakData {
        WeakData(Rc::downgrade(&self.0))
    }

    /// Derive a column's sort order now rather than on its first sort. A
    /// column without a derivable order, or none at `column`, is left alone.
    pub fn warm(&self, column: usize) {
        if column < self.width() {
            let _ = self.order(column);
        }
    }

    /// Approximate heap bytes this table reaches: chunks, dictionaries, tags
    /// and the sort orders derived so far. Chunks shared with other tables
    /// count in full.
    pub fn memory_bytes(&self) -> usize {
        let store = &*self.0;
        let names = store.tag_names.heap();
        let columns: usize = store
            .columns
            .iter()
            .map(|column| match column {
                Column::Text { text, rank } => text.heap() + rank.heap(),
                Column::Enum { codes, dict } => codes.heap() + dict.heap(),
                Column::Number { values } => values.heap(),
            })
            .sum();
        store.ids.heap() + store.classes.heap() + store.tags.heap() + names + columns
    }

    /// Rows held.
    pub fn len(&self) -> usize {
        self.0.rows
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
            Column::Enum { codes, dict } => match codes.at(row) {
                NONE => Cow::Borrowed(""),
                code => Cow::Borrowed(&dict.names[code as usize]),
            },
            Column::Number { values } => {
                let mut out = String::new();
                write_number(&mut out, values.at(row));
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
                write_number(&mut text, values.at(row));
                read(std::str::from_utf8(&text.bytes[..text.len]).unwrap_or(""))
            }
            _ => read(&self.text(row, column)),
        }
    }

    /// A cell's number, when its column holds numbers or its text parses.
    pub fn number(&self, row: usize, column: usize) -> Option<f64> {
        match &self.0.columns[column] {
            Column::Number { values } => Some(values.at(row)).filter(|n| !n.is_nan()),
            _ => self.text(row, column).trim().parse().ok(),
        }
    }

    /// Whether a cell has no value.
    pub fn is_missing(&self, row: usize, column: usize) -> bool {
        match &self.0.columns[column] {
            Column::Text { text, .. } => text.get(row).is_none(),
            Column::Enum { codes, .. } => codes.at(row) == NONE,
            Column::Number { values } => values.at(row).is_nan(),
        }
    }

    /// A row's tags, canonical and in alphabetical order.
    pub fn tags(&self, row: usize) -> impl Iterator<Item = &str> + '_ {
        let names = &self.0.tag_names.names;
        let mut tags: Vec<&str> = self
            .tag_ids(row)
            .iter()
            .map(|&id| &*names[id as usize])
            .collect();
        tags.sort_unstable();
        tags.into_iter()
    }

    /// Whether any row carries a tag.
    pub fn tagged(&self) -> bool {
        !self.0.tag_names.names.is_empty()
    }

    /// The interned id of a canonical tag.
    pub(crate) fn tag_id(&self, tag: &str) -> Option<u32> {
        self.0.tag_names.id(tag)
    }

    /// A row's interned tag ids, ascending. Ids follow first appearance, not
    /// the alphabet.
    pub(crate) fn tag_ids(&self, row: usize) -> &[u32] {
        let (chunk, at) = self.0.tags.chunk(row);
        chunk.of(at)
    }

    /// The column's distinct values when it is an enum.
    pub(crate) fn dictionary(&self, column: usize) -> Option<&[Box<str>]> {
        match &self.0.columns[column] {
            Column::Enum { dict, .. } => Some(&dict.names),
            _ => None,
        }
    }

    /// An enum cell's code.
    pub(crate) fn code(&self, row: usize, column: usize) -> Option<u32> {
        match &self.0.columns[column] {
            Column::Enum { codes, .. } => Some(codes.at(row)).filter(|&c| c != NONE),
            _ => None,
        }
    }

    /// Marks in `found` the rows of `rows` whose text in `column`, folded,
    /// holds `needle`, and says whether it could: only a text column is
    /// searched at once. Other rows of the chunks `rows` touches may be marked
    /// too.
    pub(super) fn find_text(
        &self,
        column: usize,
        needle: &Needle,
        found: &mut RowSet,
        rows: Range<usize>,
    ) -> bool {
        let Column::Text { text, .. } = &self.0.columns[column] else {
            return false;
        };
        for chunk in chunks(&rows) {
            let out = found.words_from(chunk << CHUNK_BITS);
            text.0[chunk].find(needle, out);
        }
        true
    }

    /// Marks in `found` the rows of `rows` whose code in the enum column
    /// `column` is one `hit` marks. Other rows of the chunks `rows` touches
    /// may be marked too.
    pub(super) fn find_codes(
        &self,
        column: usize,
        hit: &[bool],
        found: &mut RowSet,
        rows: Range<usize>,
    ) {
        let Column::Enum { codes, .. } = &self.0.columns[column] else {
            return;
        };
        for chunk in chunks(&rows) {
            let out = found.words_from(chunk << CHUNK_BITS);
            for (word, block) in out.iter_mut().zip(codes.0[chunk].chunks(64)) {
                for (bit, &code) in block.iter().enumerate() {
                    *word |= u64::from(hit.get(code as usize).copied().unwrap_or(false)) << bit;
                }
            }
        }
    }

    /// What a sort compares for a column: `u32` ranks for text and enums
    /// (natural order, computed on first use), the value for numbers.
    pub(crate) fn order(&self, column: usize) -> Order<'_> {
        match &self.0.columns[column] {
            Column::Text { text, rank } => Order::Rank(&rank.get(text, self.0.rows).ranks),
            Column::Enum { codes, dict } => Order::Code {
                codes,
                rank: dict.rank.get_or_init(|| dict_rank(&dict.names)),
            },
            Column::Number { values } => Order::Number(values),
        }
    }
}

/// Appends `text` lowercased char by char, as the quick filter folds text.
fn fold_into(out: &mut Vec<u8>, text: &str) {
    let mut utf8 = [0u8; 4];
    for c in text.chars() {
        if c.is_ascii() {
            out.push(c.to_ascii_lowercase() as u8);
        } else {
            for lower in c.to_lowercase() {
                out.extend_from_slice(lower.encode_utf8(&mut utf8).as_bytes());
            }
        }
    }
}

/// The chunks that hold rows `rows`.
fn chunks(rows: &Range<usize>) -> Range<usize> {
    if rows.is_empty() {
        return 0..0;
    }
    rows.start >> CHUNK_BITS..((rows.end - 1) >> CHUNK_BITS) + 1
}

/// How one column orders rows.
pub(crate) enum Order<'a> {
    /// A rank per row; `u32::MAX` is missing.
    Rank(&'a [u32]),
    /// A code per row and a rank per code.
    Code {
        codes: &'a Spine<Vec<u32>>,
        rank: &'a [u32],
    },
    /// The value per row; NaN is missing.
    Number(&'a Spine<Vec<f64>>),
}

impl Order<'_> {
    /// The row's position in ascending order, or `None` when it has no value.
    #[inline]
    pub(crate) fn key(&self, row: usize) -> Option<f64> {
        match self {
            Order::Rank(rank) => Some(rank[row]).filter(|&r| r != NONE).map(f64::from),
            Order::Code { codes, rank } => match codes.at(row) {
                NONE => None,
                code => Some(f64::from(rank[code as usize])),
            },
            Order::Number(values) => Some(values.at(row)).filter(|n| !n.is_nan()),
        }
    }
}

#[inline]
fn at(text: &Spine<Strings>, row: u32) -> &str {
    text.get(row as usize).unwrap_or("")
}

/// Ranks of every row of a text column, from nothing.
fn cold(text: &Spine<Strings>, rows: usize) -> RankInfo {
    let sorted = natural_runs(text, 0..rows);
    let mut ranks = vec![NONE; rows];
    let mut reps = Vec::with_capacity(sorted.starts.len());
    for (group, &start) in sorted.starts.iter().enumerate() {
        let end = sorted.end_of(group);
        for &row in &sorted.order[start as usize..end] {
            ranks[row as usize] = start;
        }
        reps.push(sorted.order[start as usize]);
    }
    RankInfo {
        ranks: ranks.into_boxed_slice(),
        reps: reps.into_boxed_slice(),
        present: sorted.order.len() as u32,
    }
}

/// Where a run of equal new values falls among the old distinct values.
struct Fall {
    /// Index of the run's first row in the new rows' sorted order.
    first: usize,
    len: usize,
    /// The first old distinct value not below it.
    class: usize,
    /// Whether that value equals it.
    equal: bool,
}

/// Ranks for `rows` rows when `base` ranks the first `base.ranks.len()`:
/// the new rows are sorted among themselves and placed among the old
/// distinct values, and every old rank rises by the new rows below it.
fn carry(base: &RankInfo, text: &Spine<Strings>, rows: usize) -> RankInfo {
    let old = base.ranks.len();
    let fresh = natural_runs(text, old..rows);
    let classes = base.reps.len();
    let rank_of = |class: usize| base.ranks[base.reps[class] as usize];

    let mut falls: Vec<Fall> = Vec::with_capacity(fresh.starts.len());
    let mut class = 0;
    for (group, &start) in fresh.starts.iter().enumerate() {
        let first = start as usize;
        let value = at(text, fresh.order[first]);
        class +=
            base.reps[class..].partition_point(|&rep| natural_cmp(at(text, rep), value).is_lt());
        falls.push(Fall {
            first,
            len: fresh.end_of(group) - first,
            class,
            equal: class < classes && natural_cmp(at(text, base.reps[class]), value).is_eq(),
        });
    }

    // Old rows from `.0` up have `.1` new rows below them.
    let mut steps: Vec<(u32, u32)> = Vec::new();
    let mut below = 0u32;
    for fall in &falls {
        below += fall.len as u32;
        let shifted = fall.class + usize::from(fall.equal);
        if shifted < classes {
            let from = rank_of(shifted);
            match steps.last_mut() {
                Some(step) if step.0 == from => step.1 = below,
                _ => steps.push((from, below)),
            }
        }
    }

    let mut ranks: Vec<u32> = Vec::with_capacity(rows);
    match steps.first() {
        None => ranks.extend_from_slice(&base.ranks),
        Some(&(lowest, _)) => {
            let mut shift = vec![0u32; (base.present - lowest) as usize];
            for (at, &(from, below)) in steps.iter().enumerate() {
                let to = steps.get(at + 1).map_or(base.present, |next| next.0);
                shift[(from - lowest) as usize..(to - lowest) as usize].fill(below);
            }
            ranks.extend(base.ranks.iter().map(|&rank| {
                match shift.get(rank.wrapping_sub(lowest) as usize) {
                    Some(by) => rank + by,
                    None => rank,
                }
            }));
        }
    }
    ranks.resize(rows, NONE);

    let mut reps = Vec::with_capacity(classes + fresh.starts.len());
    let mut copied = 0;
    for fall in &falls {
        let before = if fall.class < classes {
            rank_of(fall.class)
        } else {
            base.present
        };
        let rank = before + fall.first as u32;
        for &row in &fresh.order[fall.first..fall.first + fall.len] {
            ranks[row as usize] = rank;
        }
        if !fall.equal {
            reps.extend_from_slice(&base.reps[copied..fall.class]);
            copied = fall.class;
            reps.push(fresh.order[fall.first]);
        }
    }
    reps.extend_from_slice(&base.reps[copied..]);

    RankInfo {
        ranks: ranks.into_boxed_slice(),
        reps: reps.into_boxed_slice(),
        present: base.present + fresh.order.len() as u32,
    }
}

/// Rows holding a value, in natural order.
struct Sorted {
    order: Vec<u32>,
    /// Index into `order` where each run of equal values begins.
    starts: Vec<u32>,
}

impl Sorted {
    fn end_of(&self, group: usize) -> usize {
        self.starts
            .get(group + 1)
            .map_or(self.order.len(), |&next| next as usize)
    }
}

/// Runs of at most this many rows sharing a key are compared whole; longer
/// ones are split by their next segment.
const SMALL_RUN: usize = 16;

/// Set in a packed head byte where a non-ASCII character stood.
const NON_ASCII: u64 = 0x8080_8080_8080_8080;

/// A text cell mid-sort: the key of the segment just read and where the next
/// one begins.
#[derive(Clone, Copy)]
struct Item {
    head: u64,
    number: u64,
    row: u32,
    pos: u32,
}

/// Sort the rows of `rows` that hold a value into natural order. Rows sort
/// first on a packed key whose integer order agrees with [`natural_cmp`]
/// wherever keys differ. Rows that tie are split by the key of their next
/// segment, again and again, so strings sharing a long prefix are told apart
/// without comparing them whole; only runs that stay ambiguous (short, or
/// reaching non-ASCII text or a huge number) fall back to [`natural_cmp`].
fn natural_runs(text: &Spine<Strings>, rows: Range<usize>) -> Sorted {
    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        let Some(value) = text.get(row) else { continue };
        let (head, number, used) = segment(value, 0);
        items.push(Item {
            head,
            number,
            row: row as u32,
            pos: used as u32,
        });
    }
    items.sort_unstable_by_key(|item| (item.head, item.number, item.row));
    let mut refiner = Refiner {
        text,
        boundary: vec![false; items.len()],
        items,
        pending: Vec::new(),
    };
    refiner.run();
    Sorted {
        order: refiner.items.iter().map(|item| item.row).collect(),
        starts: refiner
            .boundary
            .iter()
            .enumerate()
            .filter_map(|(at, &begins)| begins.then_some(at as u32))
            .collect(),
    }
}

struct Refiner<'a> {
    text: &'a Spine<Strings>,
    items: Vec<Item>,
    /// Whether the item at each place begins a run of equal values.
    boundary: Vec<bool>,
    /// Ranges of items that tie so far and are worth splitting further.
    pending: Vec<(usize, usize)>,
}

impl Refiner<'_> {
    fn run(&mut self) {
        self.split(0, self.items.len());
        // A NUL byte is indistinguishable from key padding: compare those runs whole.
        let queued = std::mem::take(&mut self.pending);
        for (start, end) in queued {
            let text = self.text;
            if self.items[start..end]
                .iter()
                .any(|item| at(text, item.row).as_bytes().contains(&0))
            {
                self.compare(start, end);
            } else {
                self.pending.push((start, end));
            }
        }
        while let Some((start, end)) = self.pending.pop() {
            if self.skip_shared(start, end) {
                self.boundary[start] = true;
                continue;
            }
            let text = self.text;
            for item in &mut self.items[start..end] {
                let (head, number, used) = segment(at(text, item.row), item.pos as usize);
                (item.head, item.number) = (head, number);
                item.pos += used as u32;
            }
            self.items[start..end].sort_unstable_by_key(|item| (item.head, item.number, item.row));
            self.split(start, end);
        }
    }

    /// Settle each run of equal keys in the sorted `start..end`.
    fn split(&mut self, start: usize, end: usize) {
        let mut from = start;
        while from < end {
            let key = (self.items[from].head, self.items[from].number);
            let mut to = from + 1;
            while to < end && (self.items[to].head, self.items[to].number) == key {
                to += 1;
            }
            self.settle(from, to);
            from = to;
        }
    }

    /// Items `start..end` tie on every key so far.
    fn settle(&mut self, start: usize, end: usize) {
        let first = self.items[start];
        if end - start == 1 {
            self.boundary[start] = true;
        } else if end - start <= SMALL_RUN
            || first.head & NON_ASCII != 0
            || first.number == u64::MAX
        {
            self.compare(start, end);
        } else {
            self.pending.push((start, end));
        }
    }

    /// Advance the tied items past the bytes they all share, short of a digit
    /// run or character the next byte would split. True when they share all
    /// that is left: they are equal.
    fn skip_shared(&mut self, start: usize, end: usize) -> bool {
        let text = self.text;
        let rest = |item: &Item| &at(text, item.row).as_bytes()[item.pos as usize..];
        let first = rest(&self.items[start]);
        let mut shared = first.len();
        let mut alike = true;
        for item in &self.items[start + 1..end] {
            let other = rest(item);
            alike &= other.len() == first.len();
            shared = shared.min(common_len(&first[..shared], other));
        }
        if alike && shared == first.len() {
            return true;
        }
        let mut cut = shared;
        while cut > 0 && first[cut - 1].is_ascii_digit() {
            cut -= 1;
        }
        while cut > 0 && first.get(cut).is_some_and(|&byte| (byte as i8) < -0x40) {
            cut -= 1;
        }
        for item in &mut self.items[start..end] {
            item.pos += cut as u32;
        }
        false
    }

    /// Order `start..end` by [`natural_cmp`], the only judge left. Identical
    /// strings are grouped by a byte sort first, so only distinct values are
    /// compared naturally.
    fn compare(&mut self, start: usize, end: usize) {
        let text = self.text;
        let run = &mut self.items[start..end];
        let flags = &mut self.boundary[start..end];
        if run.len() <= SMALL_RUN {
            run.sort_by(|a, b| natural_cmp(at(text, a.row), at(text, b.row)));
            flags[0] = true;
            for place in 1..run.len() {
                flags[place] =
                    natural_cmp(at(text, run[place - 1].row), at(text, run[place].row)).is_ne();
            }
            return;
        }
        run.sort_by(|a, b| at(text, a.row).cmp(at(text, b.row)));
        let mut blocks: Vec<&[Item]> = run
            .chunk_by(|a, b| at(text, a.row) == at(text, b.row))
            .collect();
        blocks.sort_by(|a, b| natural_cmp(at(text, a[0].row), at(text, b[0].row)));
        let mut sorted = Vec::with_capacity(run.len());
        let mut previous: Option<&str> = None;
        for block in blocks {
            let value = at(text, block[0].row);
            flags[sorted.len()] = previous.is_none_or(|p| natural_cmp(p, value).is_ne());
            sorted.extend_from_slice(block);
            previous = Some(value);
        }
        run.copy_from_slice(&sorted);
    }
}

/// The length of the longest common prefix of `a` and `b`.
fn common_len(a: &[u8], b: &[u8]) -> usize {
    let n = a.len().min(b.len());
    let mut at = 0;
    while at + 8 <= n {
        let diff = u64::from_le_bytes(a[at..at + 8].try_into().expect("eight bytes"))
            ^ u64::from_le_bytes(b[at..at + 8].try_into().expect("eight bytes"));
        if diff != 0 {
            return at + (diff.trailing_zeros() / 8) as usize;
        }
        at += 8;
    }
    while at < n && a[at] == b[at] {
        at += 1;
    }
    at
}

/// A packed prefix of `text` from byte `from` whose order agrees with
/// [`natural_cmp`] wherever two keys differ, and the bytes it covers: up to
/// eight lowercase ASCII bytes before the first digit or non-ASCII byte
/// (padded so a digit sorts as `'0'`, a non-ASCII character above every ASCII
/// one, and the end below everything), then the value of the digit run that
/// stopped it. Equal keys mean equal text over the covered bytes, unless a
/// non-ASCII byte or a number past `u64` is in them.
fn segment(text: &str, from: usize) -> (u64, u64, usize) {
    let bytes = &text.as_bytes()[from..];
    let mut head = [0u8; 8];
    let mut number = 0u64;
    let mut at = 0;
    let used = loop {
        if at == 8 {
            break 8;
        }
        match bytes.get(at) {
            None => break at,
            Some(b) if b.is_ascii_digit() => {
                head[at..].fill(b'0');
                let run = bytes[at..]
                    .iter()
                    .take_while(|d| d.is_ascii_digit())
                    .count();
                let digits = &bytes[at..at + run];
                let digits = &digits[digits.iter().take_while(|&&d| d == b'0').count()..];
                number = if digits.len() > 19 {
                    u64::MAX
                } else {
                    digits.iter().fold(0, |n, d| n * 10 + u64::from(d - b'0'))
                };
                break at + run;
            }
            Some(b) if !b.is_ascii() => {
                head[at..].fill(0xFF);
                break at;
            }
            Some(b) => head[at] = b.to_ascii_lowercase(),
        }
        at += 1;
    };
    (u64::from_be_bytes(head), number, used)
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

/// A column being written: its finished chunks, then the one being filled.
struct Open<T> {
    full: Vec<Rc<T>>,
    tail: T,
}

impl<T: Chunk> Open<T> {
    fn new(rows: usize) -> Self {
        Open {
            full: Vec::new(),
            tail: T::with_capacity(rows),
        }
    }

    /// Carry on from `spine`'s `rows` rows, copying its last chunk when it
    /// is not full so it can be extended.
    fn resume(spine: &Spine<T>, rows: usize) -> Self {
        let mut full = spine.0.clone();
        let tail = if rows & CHUNK_MASK == 0 {
            T::default()
        } else {
            Rc::unwrap_or_clone(full.pop().expect("a partly filled chunk"))
        };
        Open { full, tail }
    }

    fn seal(&mut self, next_rows: usize) {
        let next = self.tail.following(next_rows);
        self.full
            .push(Rc::new(std::mem::replace(&mut self.tail, next)));
    }

    fn finish(mut self, tail_rows: usize) -> Spine<T> {
        if tail_rows > 0 {
            self.full.push(Rc::new(self.tail));
        }
        Spine(self.full)
    }
}

enum Building {
    Text(Open<Strings>),
    Enum {
        codes: Open<Vec<u32>>,
        dict: Rc<Dict>,
        /// Values the dictionary lacks, by code from `dict.names.len()`.
        added: Vec<Box<str>>,
        added_index: HashMap<Box<str>, u32>,
    },
    Number(Open<Vec<f64>>),
}

/// Appends rows; [`TableDataBuilder::build`] seals them.
pub struct TableDataBuilder {
    /// The table being appended to.
    base: Option<Rc<Store>>,
    /// Rows held, the base's included.
    rows: usize,
    /// Rows expected in all, to size chunks.
    hint: usize,
    /// Rows in the chunk being filled.
    tail_rows: usize,
    ids: Open<Strings>,
    columns: Vec<Building>,
    tags: Open<TagRuns>,
    /// The base's tags: ids below its count are the base's.
    tag_base: Rc<TagNames>,
    /// Tags the base lacks, by id from the base's count.
    tag_index: HashMap<String, u32>,
    classes: Open<Strings>,
    number: String,
    /// Each column's ranks to carry onto the table built.
    bases: Vec<Option<Rc<RankInfo>>>,
}

impl TableDataBuilder {
    /// A builder expecting about `rows` rows.
    pub fn with_capacity(columns: &[DataColumnSpec], rows: usize) -> Self {
        let chunk = rows.min(CHUNK_ROWS);
        let columns = columns
            .iter()
            .map(|column| match column.kind {
                ColumnKind::Text => Building::Text(Open::new(chunk)),
                ColumnKind::Enum => Building::Enum {
                    codes: Open::new(chunk),
                    dict: Dict::new(Vec::new()),
                    added: Vec::new(),
                    added_index: HashMap::new(),
                },
                ColumnKind::Number => Building::Number(Open::new(chunk)),
            })
            .collect();
        TableDataBuilder {
            base: None,
            rows: 0,
            hint: rows,
            tail_rows: 0,
            ids: Open::new(chunk),
            columns,
            tags: Open::new(chunk),
            tag_base: Rc::default(),
            tag_index: HashMap::new(),
            classes: Open::new(chunk),
            number: String::new(),
            bases: Vec::new(),
        }
    }

    /// A builder that continues `data`'s rows.
    fn extending(data: &TableData) -> Self {
        let store = &data.0;
        let rows = store.rows;
        let columns = store
            .columns
            .iter()
            .map(|column| match column {
                Column::Text { text, .. } => Building::Text(Open::resume(text, rows)),
                Column::Enum { codes, dict } => Building::Enum {
                    codes: Open::resume(codes, rows),
                    dict: Rc::clone(dict),
                    added: Vec::new(),
                    added_index: HashMap::new(),
                },
                Column::Number { values } => Building::Number(Open::resume(values, rows)),
            })
            .collect();
        let bases = store
            .columns
            .iter()
            .map(|column| match column {
                Column::Text { rank, .. } => rank.carry_source(),
                _ => None,
            })
            .collect();
        TableDataBuilder {
            base: Some(Rc::clone(store)),
            rows,
            hint: 0,
            tail_rows: rows & CHUNK_MASK,
            ids: Open::resume(&store.ids, rows),
            columns,
            tags: Open::resume(&store.tags, rows),
            tag_base: Rc::clone(&store.tag_names),
            tag_index: HashMap::new(),
            classes: Open::resume(&store.classes, rows),
            number: String::new(),
            bases,
        }
    }

    /// Append a row with `id` and one value per declared column, in order;
    /// missing trailing values are missing cells.
    pub fn row<'a>(
        &mut self,
        id: &str,
        values: impl IntoIterator<Item = Value<'a>>,
    ) -> RowWriter<'_> {
        if self.tail_rows == CHUNK_ROWS {
            self.seal();
        }
        let mut values = values.into_iter();
        for column in &mut self.columns {
            push(
                column,
                values.next().unwrap_or(Value::Missing),
                &mut self.number,
            );
        }
        self.ids.tail.push(Some(id));
        let tags = &mut self.tags.tail;
        tags.ends.push(offset(tags.ids.len()));
        self.classes.tail.push(None);
        self.tail_rows += 1;
        self.rows += 1;
        RowWriter { builder: self }
    }

    /// Close every column's full chunk and open the next.
    fn seal(&mut self) {
        let next = self.hint.saturating_sub(self.rows).min(CHUNK_ROWS);
        self.ids.seal(next);
        self.classes.seal(next);
        self.tags.seal(next);
        for column in &mut self.columns {
            match column {
                Building::Text(open) => open.seal(next),
                Building::Enum { codes, .. } => codes.seal(next),
                Building::Number(open) => open.seal(next),
            }
        }
        self.tail_rows = 0;
    }

    /// Seal the rows.
    pub fn build(self) -> TableData {
        let TableDataBuilder {
            base,
            rows,
            tail_rows,
            ids,
            columns,
            tags,
            tag_base,
            tag_index,
            classes,
            bases,
            ..
        } = self;
        let branch = match &base {
            Some(base) if base.branch.head.get() == base.rows => Rc::clone(&base.branch),
            Some(base) => Rc::new(Branch {
                parent: Some((Rc::clone(&base.branch), base.rows)),
                head: Cell::new(0),
            }),
            None => Rc::new(Branch {
                parent: None,
                head: Cell::new(0),
            }),
        };
        branch.head.set(rows);
        let tag_names = TagNames::extended(tag_base, tag_index);
        let columns = columns
            .into_iter()
            .enumerate()
            .map(|(at, column)| match column {
                Building::Text(open) => Column::Text {
                    text: open.finish(tail_rows),
                    rank: Ranks::new(bases.get(at).cloned().flatten()),
                },
                Building::Enum {
                    codes, dict, added, ..
                } => Column::Enum {
                    codes: codes.finish(tail_rows),
                    dict: if added.is_empty() {
                        dict
                    } else {
                        let mut names = dict.names.clone();
                        names.extend(added);
                        Dict::new(names)
                    },
                },
                Building::Number(open) => Column::Number {
                    values: open.finish(tail_rows),
                },
            })
            .collect();
        TableData(Rc::new(Store {
            rows,
            ids: ids.finish(tail_rows),
            columns,
            tags: tags.finish(tail_rows),
            tag_names,
            classes: classes.finish(tail_rows),
            branch,
        }))
    }
}

fn push(column: &mut Building, value: Value<'_>, scratch: &mut String) {
    let number_text = |scratch: &mut String, n: f64| {
        scratch.clear();
        write_number(scratch, n);
    };
    match column {
        Building::Text(open) => match value {
            Value::Text(t) => open.tail.push(Some(t)),
            Value::Number(n) => {
                number_text(scratch, n);
                open.tail.push(Some(scratch));
            }
            Value::Missing => open.tail.push(None),
        },
        Building::Number(open) => open.tail.push(match value {
            Value::Number(n) if n.is_finite() => n,
            Value::Text(t) => t
                .trim()
                .parse()
                .ok()
                .filter(|n: &f64| n.is_finite())
                .unwrap_or(f64::NAN),
            _ => f64::NAN,
        }),
        Building::Enum {
            codes,
            dict,
            added,
            added_index,
        } => codes.tail.push(match value {
            Value::Missing => NONE,
            Value::Text(t) => intern(dict, added, added_index, t),
            Value::Number(n) => {
                number_text(scratch, n);
                intern(dict, added, added_index, scratch)
            }
        }),
    }
}

fn intern(
    dict: &Dict,
    added: &mut Vec<Box<str>>,
    added_index: &mut HashMap<Box<str>, u32>,
    text: &str,
) -> u32 {
    if let Some(&code) = dict.index().get(text) {
        return code;
    }
    if let Some(&code) = added_index.get(text) {
        return code;
    }
    let code = (dict.names.len() + added.len()) as u32;
    added.push(text.into());
    added_index.insert(text.into(), code);
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
        let runs = &mut builder.tags.tail;
        let start = runs
            .ends
            .len()
            .checked_sub(2)
            .map_or(0, |at| runs.ends[at] as usize);
        runs.ids.truncate(start);
        for tag in tags {
            let tag = canonical_tag(tag.as_ref());
            if tag.is_empty() {
                continue;
            }
            let id = match builder.tag_base.id(&tag) {
                Some(id) => id,
                None => {
                    let next = (builder.tag_base.names.len() + builder.tag_index.len()) as u32;
                    *builder.tag_index.entry(tag).or_insert(next)
                }
            };
            if !runs.ids[start..].contains(&id) {
                runs.ids.push(id);
            }
        }
        runs.ids[start..].sort_unstable();
        *runs.ends.last_mut().expect("a row was appended") = offset(runs.ids.len());
        self
    }

    /// Extra classes on the row (`"pk-dt-muted"` dims a finished one).
    pub fn class(self, class: &str) -> Self {
        self.builder.classes.tail.set_last(class);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use rstest::rstest;
    use std::collections::BTreeSet;

    const KINDS: [&str; 7] = ["a", "B", "b", "10", "9", "", "ä"];
    const NUMBERS: [f64; 8] = [0.0, -1.5, 3.0, 1e15, 2.5, 1e-7, f64::NAN, f64::INFINITY];
    const TAGS: [&str; 7] = ["raft", "Raft", " ui ", "", "api", "Zed", "ops"];
    const CLASSES: [&str; 3] = ["pk-dt-muted", "", "x y"];

    fn spec() -> Vec<DataColumnSpec> {
        vec![
            DataColumnSpec::new("text", "text"),
            DataColumnSpec::new("kind", "kind").enumerated(),
            DataColumnSpec::new("n", "n").number(),
        ]
    }

    #[derive(Clone, Debug)]
    struct Row {
        id: String,
        text: Option<String>,
        kind: Option<&'static str>,
        number: Option<f64>,
        tags: Vec<&'static str>,
        class: Option<&'static str>,
    }

    fn push(builder: &mut TableDataBuilder, row: &Row) {
        let writer = builder
            .row(
                &row.id,
                [
                    row.text.as_deref().map_or(Value::Missing, Value::Text),
                    row.kind.map_or(Value::Missing, Value::Text),
                    row.number.map_or(Value::Missing, Value::Number),
                ],
            )
            .tags(&row.tags);
        if let Some(class) = row.class {
            writer.class(class);
        }
    }

    fn build(rows: &[Row]) -> TableData {
        let mut builder = TableData::builder(&spec());
        for row in rows {
            push(&mut builder, row);
        }
        builder.build()
    }

    fn append(data: &TableData, rows: &[Row]) -> TableData {
        data.append(|builder| rows.iter().for_each(|row| push(builder, row)))
    }

    /// Whether the text column's ranks will start from an earlier table's.
    fn carries(data: &TableData, column: usize) -> bool {
        matches!(&data.0.columns[column], Column::Text { rank, .. } if rank.carry_source().is_some())
    }

    /// `rows` built up to the first cut, then appended piece by piece. The
    /// text ranks are derived before the steps `warm` names, so those appends
    /// carry them. Each table comes with the rows it covers.
    fn chained(rows: &[Row], cuts: &[usize], warm: &[bool]) -> Vec<(usize, TableData)> {
        let mut ends: Vec<usize> = cuts.iter().map(|cut| cut % (rows.len() + 1)).collect();
        ends.sort_unstable();
        ends.push(rows.len());
        let mut parts = vec![(ends[0], build(&rows[..ends[0]]))];
        let mut ranked = false;
        for (step, window) in ends.windows(2).enumerate() {
            let (from, to) = (window[0], window[1]);
            let last = parts.last().expect("a first part").1.clone();
            if warm.get(step).copied().unwrap_or(false) {
                last.warm(0);
                ranked = true;
            }
            let next = append(&last, &rows[from..to]);
            if to > from {
                assert_eq!(carries(&next, 0), ranked, "step {step} carries the ranks");
            }
            parts.push((to, next));
        }
        parts
    }

    /// Every accessor, and every sort order, agrees row by row.
    fn assert_same(got: &TableData, want: &TableData, what: &str) {
        assert_eq!(got.len(), want.len(), "{what}: rows");
        assert_eq!(got.width(), want.width(), "{what}: columns");
        assert_eq!(got.tagged(), want.tagged(), "{what}: tagged");
        for tag in ["raft", "ui", "api", "zed", "ops"] {
            assert_eq!(
                got.tag_id(tag),
                want.tag_id(tag),
                "{what}: id of tag {tag:?}"
            );
        }
        for column in 0..want.width() {
            assert_eq!(
                got.dictionary(column),
                want.dictionary(column),
                "{what}: dictionary {column}"
            );
            let (got_order, want_order) = (got.order(column), want.order(column));
            for row in 0..want.len() {
                assert_eq!(
                    got.text(row, column),
                    want.text(row, column),
                    "{what}: text {row}/{column}"
                );
                assert_eq!(
                    got.number(row, column),
                    want.number(row, column),
                    "{what}: number {row}/{column}"
                );
                assert_eq!(
                    got.is_missing(row, column),
                    want.is_missing(row, column),
                    "{what}: missing {row}/{column}"
                );
                assert_eq!(
                    got.code(row, column),
                    want.code(row, column),
                    "{what}: code {row}/{column}"
                );
                assert_eq!(
                    got_order.key(row),
                    want_order.key(row),
                    "{what}: sort key {row}/{column}"
                );
            }
        }
        for row in 0..want.len() {
            assert_eq!(got.id(row), want.id(row), "{what}: id {row}");
            assert_eq!(got.class(row), want.class(row), "{what}: class {row}");
            assert_eq!(got.tag_ids(row), want.tag_ids(row), "{what}: tag ids {row}");
            assert!(got.tags(row).eq(want.tags(row)), "{what}: tags {row}");
        }
    }

    fn piece() -> impl Strategy<Value = String> {
        prop_oneof![
            4 => "[a-cA-C]{1,3}",
            2 => Just("Trace the ingest path through shard ".to_string()),
            1 => Just("trace THE ingest path through shard ".to_string()),
            3 => "[0-9]{1,3}",
            2 => "0{1,4}[1-9]{0,2}",
            1 => "[1-9][0-9]{19,22}",
            2 => prop::sample::select(vec![" ", "-", "_", ".", "/", "~", "{", "@", "\u{1}"])
                .prop_map(str::to_string),
            2 => prop::sample::select(vec!["é", "É", "ß", "中", "😀", "σ", "Σ"])
                .prop_map(str::to_string),
            1 => Just("\0".to_string()),
        ]
    }

    fn text() -> impl Strategy<Value = String> {
        prop::collection::vec(piece(), 0..7).prop_map(|pieces| pieces.concat())
    }

    /// Few distinct values, repeated: ties of equal strings.
    fn pooled() -> impl Strategy<Value = Vec<Option<String>>> {
        prop::collection::vec(text(), 1..9).prop_flat_map(|pool| {
            prop::collection::vec(
                prop::option::weighted(0.9, prop::sample::select(pool)),
                0..100,
            )
        })
    }

    /// Many distinct values that share long prefixes: ties the packed key
    /// cannot break.
    fn shared_prefix() -> impl Strategy<Value = Vec<Option<String>>> {
        let prefix = prop::sample::select(vec![
            "",
            "Trace the ingest path through shard ",
            "trace THE ingest path through shard ",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "AAAAAAAA",
            "shard 0000000000000000000000000012 ",
            "path/to/some/very/long/prefix/number-",
        ]);
        let tail = prop::collection::vec(piece(), 0..4).prop_map(|pieces| pieces.concat());
        prop::collection::vec(
            prop::option::weighted(0.95, (prefix, tail).prop_map(|(p, t)| format!("{p}{t}"))),
            18..80,
        )
    }

    /// One prefix, then numbers that all start with 1: ties whose shared text
    /// ends inside a digit run.
    fn numbered() -> impl Strategy<Value = Vec<Option<String>>> {
        let prefix =
            prop::sample::select(vec!["Trace the ingest path through shard ", "shard-", "x"]);
        let tail = prop::option::weighted(0.95, "1[0-9]{0,4}( [a-c]{1,2})?");
        (prefix, prop::collection::vec(tail, 18..60)).prop_map(|(prefix, tails)| {
            tails
                .into_iter()
                .map(|tail| tail.map(|tail| format!("{prefix}{tail}")))
                .collect()
        })
    }

    fn texts() -> impl Strategy<Value = Vec<Option<String>>> {
        prop_oneof![pooled(), shared_prefix(), numbered()]
    }

    fn rows() -> impl Strategy<Value = Vec<Row>> {
        texts().prop_flat_map(|texts| {
            let others = (
                prop::option::of(prop::sample::select(KINDS.to_vec())),
                prop::option::of(prop::sample::select(NUMBERS.to_vec())),
                prop::collection::vec(prop::sample::select(TAGS.to_vec()), 0..4),
                prop::option::of(prop::sample::select(CLASSES.to_vec())),
            );
            prop::collection::vec(others, texts.len()).prop_map(move |others| {
                texts
                    .iter()
                    .zip(others)
                    .enumerate()
                    .map(|(n, (text, (kind, number, tags, class)))| Row {
                        id: format!("r{n}"),
                        text: text.clone(),
                        kind,
                        number,
                        tags,
                        class,
                    })
                    .collect()
            })
        })
    }

    fn ranks_of(texts: &[Option<String>]) -> Vec<u32> {
        let mut builder = TableData::builder(&[DataColumnSpec::new("t", "t")]);
        for (n, text) in texts.iter().enumerate() {
            builder.row(
                &n.to_string(),
                [text.as_deref().map_or(Value::Missing, Value::Text)],
            );
        }
        match builder.build().order(0) {
            Order::Rank(ranks) => ranks.to_vec(),
            _ => unreachable!("a text column ranks its rows"),
        }
    }

    /// The rows holding a value that sort strictly before each row's.
    fn reference_ranks(texts: &[Option<String>]) -> Vec<u32> {
        texts
            .iter()
            .map(|text| match text {
                None => NONE,
                Some(text) => texts
                    .iter()
                    .flatten()
                    .filter(|other| natural_cmp(other, text).is_lt())
                    .count() as u32,
            })
            .collect()
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(160))]

        #[test]
        fn appending_in_pieces_is_building_at_once(
            rows in rows(),
            cuts in prop::collection::vec(any::<usize>(), 0..4),
            warm in prop::collection::vec(any::<bool>(), 5),
        ) {
            let parts = chained(&rows, &cuts, &warm);
            for (end, part) in &parts {
                assert_same(part, &build(&rows[..*end]), &format!("first {end} rows"));
            }
        }

        #[test]
        fn ranks_follow_natural_order(texts in texts()) {
            prop_assert_eq!(ranks_of(&texts), reference_ranks(&texts));
        }
    }

    #[rstest]
    #[case::one_run_of_one(1)]
    #[case::two(2)]
    #[case::just_under_the_split_threshold(SMALL_RUN - 1)]
    #[case::at_the_threshold(SMALL_RUN)]
    #[case::just_over(SMALL_RUN + 1)]
    #[case::many(300)]
    fn a_tie_run_of_any_size_sorts_by_what_follows_the_shared_prefix(#[case] count: usize) {
        let texts: Vec<Option<String>> = (0..count)
            .map(|n| {
                let m = n * 7_919 % count;
                Some(format!(
                    "Trace the ingest path through shard {m:03} replica {}",
                    m % 3
                ))
            })
            .collect();
        assert_eq!(ranks_of(&texts), reference_ranks(&texts));
    }

    #[test]
    fn strings_that_differ_only_in_case_or_leading_zeros_share_a_rank() {
        let texts: Vec<Option<String>> = ["gn007", "GN7", "gn0007", "gn8", "Gn6", "gn10"]
            .into_iter()
            .map(|text| Some(text.to_string()))
            .collect();
        assert_eq!(ranks_of(&texts), vec![1, 1, 1, 4, 0, 5]);
    }

    #[test]
    fn a_zero_padded_number_wider_than_a_u64_orders_by_its_value() {
        let texts: Vec<Option<String>> = [
            "id 0000000000000000000000000012",
            "id 13",
            "id 12",
            "id 000000000000000000000000000000000000011",
            "id 9",
        ]
        .into_iter()
        .map(|text| Some(text.to_string()))
        .collect();
        assert_eq!(ranks_of(&texts), vec![2, 4, 2, 1, 0]);
    }

    #[test]
    fn numbers_sharing_leading_digits_after_a_shared_prefix_order_by_value() {
        let texts: Vec<Option<String>> = [
            1, 10, 100, 1000, 10_000, 11, 111, 1_111, 12, 123, 1_234, 12_345, 15, 150, 1_500, 18,
            180, 19, 190, 1_900,
        ]
        .into_iter()
        .rev()
        .map(|n| Some(format!("Trace the ingest path through shard {n}")))
        .collect();
        assert_eq!(ranks_of(&texts), reference_ranks(&texts));
    }

    #[test]
    fn a_long_shared_prefix_of_non_ascii_text_still_orders_by_natural_comparison() {
        let texts: Vec<Option<String>> = (0..40)
            .rev()
            .map(|n| Some(format!("Ünïcode ünïcode ünïcode path {n} ÄÖ")))
            .collect();
        assert_eq!(ranks_of(&texts), reference_ranks(&texts));
    }

    fn chunked(count: usize) -> Vec<Row> {
        (0..count)
            .map(|n| Row {
                id: format!("r{n}"),
                text: (n % 11 != 0).then(|| format!("title {}", n * 7 % 1_001)),
                kind: [Some("open"), Some("done"), None][n % 3],
                number: (n % 5 != 0).then_some(n as f64),
                tags: if n % 4 == 0 {
                    vec!["Raft", "api"]
                } else {
                    vec![]
                },
                class: (n % 1_000 == 0).then_some("pk-dt-muted"),
            })
            .collect()
    }

    #[rstest]
    #[case::the_last_row_of_a_chunk(CHUNK_ROWS + 1, &[CHUNK_ROWS - 1])]
    #[case::the_chunk_filled_exactly(CHUNK_ROWS + 1, &[CHUNK_ROWS])]
    #[case::onto_a_full_chunk(CHUNK_ROWS, &[CHUNK_ROWS - 1])]
    #[case::across_three_chunks(2 * CHUNK_ROWS + 5, &[CHUNK_ROWS - 3, CHUNK_ROWS + 2])]
    #[case::a_few_rows_onto_a_single_row(CHUNK_ROWS + 5, &[1])]
    fn appending_across_chunk_boundaries_matches_one_build(
        #[case] total: usize,
        #[case] cuts: &[usize],
    ) {
        let rows = chunked(total);
        for (end, part) in chained(&rows, cuts, &[true, true]) {
            assert_same(&part, &build(&rows[..end]), &format!("first {end} rows"));
        }
    }

    #[test]
    fn appending_shares_every_full_chunk_and_copies_only_the_last() {
        let rows = chunked(2 * CHUNK_ROWS + 10);
        let old = build(&rows[..2 * CHUNK_ROWS + 5]);
        let new = append(&old, &rows[2 * CHUNK_ROWS + 5..]);
        assert_eq!(new.len(), rows.len());
        let (old_store, new_store) = (&old.0, &new.0);
        assert!(Rc::ptr_eq(&old_store.ids.0[0], &new_store.ids.0[0]));
        assert!(Rc::ptr_eq(&old_store.ids.0[1], &new_store.ids.0[1]));
        assert!(!Rc::ptr_eq(&old_store.ids.0[2], &new_store.ids.0[2]));
        match (&old_store.columns[2], &new_store.columns[2]) {
            (Column::Number { values: old }, Column::Number { values: new }) => {
                assert!(Rc::ptr_eq(&old.0[0], &new.0[0]));
                assert!(Rc::ptr_eq(&old.0[1], &new.0[1]));
            }
            _ => unreachable!("column 2 is a number column"),
        }
        assert!(Rc::ptr_eq(&old_store.tags.0[1], &new_store.tags.0[1]));
        assert_eq!(
            old.len(),
            2 * CHUNK_ROWS + 5,
            "the earlier table keeps its rows"
        );
        assert_eq!(old.id(2 * CHUNK_ROWS + 4), "r131076");
    }

    #[test]
    fn appended_values_the_old_table_lacks_do_not_reach_it() {
        let old = build(&[Row {
            id: "a".into(),
            text: Some("m".into()),
            kind: Some("open"),
            number: Some(1.0),
            tags: vec!["raft"],
            class: None,
        }]);
        old.warm(0);
        let new = append(
            &old,
            &[Row {
                id: "b".into(),
                text: Some("a".into()),
                kind: Some("0 new"),
                number: None,
                tags: vec!["zeta", "api", "raft"],
                class: Some("pk-dt-muted"),
            }],
        );
        assert_eq!(old.dictionary(1), Some(&["open".into()][..]));
        assert_eq!(
            new.dictionary(1),
            Some(&["open".into(), "0 new".into()][..])
        );
        assert_eq!(old.tag_id("api"), None);
        assert!(new.tag_id("api").is_some());
        assert!(
            new.tags(1).eq(["api", "raft", "zeta"]),
            "tags read alphabetically"
        );
        assert!(old.tags(0).eq(["raft"]));
        assert_eq!(old.class(0), None);
        assert_eq!(new.class(1), Some("pk-dt-muted"));
        // The old table's ranks and orders are what a table of its rows alone has.
        assert_same(
            &old,
            &build(&[Row {
                id: "a".into(),
                text: Some("m".into()),
                kind: Some("open"),
                number: Some(1.0),
                tags: vec!["raft"],
                class: None,
            }]),
            "old table",
        );
        // A tag filter finds rows by binary search on ids, however new tags sort.
        let raft = new.tag_id("raft").expect("raft is known");
        assert!((0..2).all(|row| new.tag_ids(row).binary_search(&raft).is_ok()));
        let api = new.tag_id("api").expect("api is known");
        assert!(new.tag_ids(0).binary_search(&api).is_err());
        assert!(new.tag_ids(1).binary_search(&api).is_ok());
    }

    #[test]
    fn a_row_s_tags_read_alphabetically_whatever_order_they_were_met_in() {
        let table = build(&[
            Row {
                id: "a".into(),
                text: None,
                kind: None,
                number: None,
                tags: vec!["zulu", "Alpha", "mike", "alpha"],
                class: None,
            },
            Row {
                id: "b".into(),
                text: None,
                kind: None,
                number: None,
                tags: vec!["bravo", " ", "mike"],
                class: None,
            },
        ]);
        assert!(table.tags(0).eq(["alpha", "mike", "zulu"]));
        assert!(table.tags(1).eq(["bravo", "mike"]));
        assert!((0..2).all(|row| table.tag_ids(row).windows(2).all(|pair| pair[0] < pair[1])));
    }

    #[test]
    fn extends_follows_append_ancestry_and_never_crosses_a_fork() {
        let rows = chunked(40);
        let t0 = build(&rows[..10]);
        let t1 = append(&t0, &rows[10..15]);
        let t2 = append(&t1, &rows[15..18]);
        let t3 = append(&t1, &rows[15..17]);
        let t4 = append(&t3, &rows[17..20]);
        for (later, earlier) in [
            (&t1, &t0),
            (&t2, &t0),
            (&t2, &t1),
            (&t3, &t1),
            (&t4, &t3),
            (&t4, &t0),
            (&t4, &t1),
            (&t2, &t2),
        ] {
            assert!(
                later.extends(earlier),
                "{} rows extend {} rows",
                later.len(),
                earlier.len()
            );
        }
        for (later, earlier) in [(&t0, &t1), (&t3, &t2), (&t2, &t3), (&t4, &t2), (&t2, &t4)] {
            assert!(
                !later.extends(earlier),
                "{} rows do not extend {} rows",
                later.len(),
                earlier.len()
            );
        }
        let apart = build(&rows[..10]);
        assert!(!apart.extends(&t0), "equal rows built apart are unrelated");
        assert!(!t1.extends(&apart));
        assert!(t0.extends(&t0.clone()));
    }

    #[test]
    fn appending_nothing_returns_the_same_table() {
        let table = build(&chunked(5));
        assert!(table.append(|_| {}) == table);
    }

    #[test]
    fn a_sort_key_reads_by_row_across_chunks() {
        let rows = chunked(CHUNK_ROWS + 3);
        let table = build(&rows);
        let last = rows.len() - 1;
        match table.order(2) {
            Order::Number(values) => {
                assert_eq!(values[last], last as f64);
                assert!(values[0].is_nan(), "row 0 has no number");
            }
            _ => unreachable!("column 2 is a number column"),
        }
        match table.order(1) {
            Order::Code { codes, rank } => {
                assert_eq!(codes[last], table.code(last, 1).unwrap_or(NONE));
                let open = table.code(0, 1).expect("row 0 is open") as usize;
                let done = table.code(1, 1).expect("row 1 is done") as usize;
                assert!(rank[done] < rank[open], "done sorts before open");
            }
            _ => unreachable!("column 1 is an enum column"),
        }
    }

    #[test]
    fn warming_derives_the_ranks_once_and_ignores_columns_without_any() {
        let table = build(&chunked(50));
        let before = table.memory_bytes();
        table.warm(0);
        let first = match table.order(0) {
            Order::Rank(ranks) => ranks.as_ptr(),
            _ => unreachable!("a text column ranks its rows"),
        };
        table.warm(0);
        table.warm(1);
        table.warm(2);
        table.warm(99);
        let second = match table.order(0) {
            Order::Rank(ranks) => ranks.as_ptr(),
            _ => unreachable!("a text column ranks its rows"),
        };
        assert_eq!(first, second, "warming twice keeps the first ranks");
        assert!(
            table.memory_bytes() >= before + 50 * size_of::<u32>(),
            "ranks count toward memory"
        );
    }

    #[test]
    fn memory_covers_the_text_the_table_holds() {
        let rows = chunked(CHUNK_ROWS + 100);
        let table = build(&rows);
        let content: usize = rows
            .iter()
            .map(|row| row.id.len() + row.text.as_deref().map_or(0, str::len) + size_of::<f64>())
            .sum();
        assert!(
            table.memory_bytes() >= content,
            "{} < {content}",
            table.memory_bytes()
        );
        assert!(
            table.memory_bytes() < 4 * content,
            "{} >= 4 * {content}",
            table.memory_bytes()
        );
        assert!(build(&[]).memory_bytes() < 1_024);
    }

    #[derive(Clone, Debug)]
    enum Make {
        /// A table built apart from the rest, of this many rows.
        Build(usize),
        /// One of the tables so far, with this many rows appended.
        Append(prop::sample::Index, usize),
    }

    fn make() -> impl Strategy<Value = Make> {
        prop_oneof![
            1 => (0usize..12).prop_map(Make::Build),
            3 => (any::<prop::sample::Index>(), 0usize..6).prop_map(|(at, count)| Make::Append(at, count)),
        ]
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(200))]

        /// Over tables made by building apart and by appending, with forks:
        /// `extends` is exactly "is this table or one it was appended from".
        #[test]
        fn extends_is_the_ancestry_of_appends(ops in prop::collection::vec(make(), 1..30)) {
            let rows = chunked(300);
            let mut tables: Vec<TableData> = Vec::new();
            // The tables each one descends from, itself included.
            let mut ancestry: Vec<BTreeSet<usize>> = Vec::new();
            for op in &ops {
                match op {
                    Make::Build(count) => {
                        ancestry.push(BTreeSet::from([tables.len()]));
                        tables.push(build(&rows[..*count]));
                    }
                    Make::Append(at, count) if !tables.is_empty() => {
                        let at = at.index(tables.len());
                        let parent = &tables[at];
                        let grown = append(parent, &rows[parent.len()..parent.len() + count]);
                        if *count == 0 {
                            prop_assert!(grown == *parent, "appending nothing is the same table");
                            continue;
                        }
                        let mut from = ancestry[at].clone();
                        from.insert(tables.len());
                        ancestry.push(from);
                        tables.push(grown);
                    }
                    Make::Append(..) => {}
                }
            }
            let count = tables.len();
            for a in 0..count {
                prop_assert!(tables[a].extends(&tables[a].clone()), "table {} extends itself", a);
                for b in 0..count {
                    let (later, earlier) = (&tables[a], &tables[b]);
                    let extends = later.extends(earlier);
                    prop_assert_eq!(extends, ancestry[a].contains(&b), "table {} over table {}", a, b);
                    if extends {
                        prop_assert!(later.len() >= earlier.len());
                        prop_assert!(
                            (0..earlier.len()).all(|row| later.id(row) == earlier.id(row)),
                            "table {} holds the first rows of table {}", a, b
                        );
                        if earlier.extends(later) {
                            prop_assert!(later == earlier, "tables {} and {} extend each other", a, b);
                        }
                    }
                    for c in 0..count {
                        if extends && tables[b].extends(&tables[c]) {
                            prop_assert!(tables[a].extends(&tables[c]), "{} over {} over {}", a, b, c);
                        }
                    }
                }
            }
        }
    }
}
