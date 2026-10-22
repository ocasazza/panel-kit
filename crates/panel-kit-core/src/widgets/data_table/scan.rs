//! Finding a word in every row of a text column at once: one pass over a
//! chunk's text instead of one call per row. A row's text is a run of the
//! chunk's bytes, so a hit is mapped to its row, and one that straddles two
//! rows is not a hit.

use std::ops::Range;

/// A set of rows `0..len`, one bit each.
#[derive(Clone, Debug)]
pub(super) struct RowSet {
    words: Vec<u64>,
}

impl RowSet {
    pub(super) fn empty(len: usize) -> Self {
        RowSet {
            words: vec![0; len.div_ceil(64)],
        }
    }

    /// Every row of `rows`, which lies within `0..len`.
    pub(super) fn of(len: usize, rows: Range<usize>) -> Self {
        let mut set = RowSet::empty(len);
        if rows.is_empty() {
            return set;
        }
        for word in rows.start / 64..rows.end.div_ceil(64) {
            let lo = rows.start.max(word * 64) - word * 64;
            let hi = rows.end.min(word * 64 + 64) - word * 64;
            set.words[word] = (!0u64 >> (64 - (hi - lo))) << lo;
        }
        set
    }

    pub(super) fn insert(&mut self, row: usize) {
        self.words[row / 64] |= 1 << (row % 64);
    }

    pub(super) fn remove(&mut self, row: usize) {
        self.words[row / 64] &= !(1 << (row % 64));
    }

    pub(super) fn contains(&self, row: usize) -> bool {
        self.words[row / 64] >> (row % 64) & 1 == 1
    }

    pub(super) fn count(&self) -> usize {
        self.words.iter().map(|w| w.count_ones() as usize).sum()
    }

    pub(super) fn intersect(&mut self, other: &RowSet) {
        for (word, other) in self.words.iter_mut().zip(&other.words) {
            *word &= other;
        }
    }

    /// The rows in the set, ascending.
    pub(super) fn rows(&self) -> impl Iterator<Item = usize> + '_ {
        self.words.iter().enumerate().flat_map(|(at, &word)| {
            let mut left = word;
            std::iter::from_fn(move || {
                (left != 0).then(|| {
                    let bit = left.trailing_zeros() as usize;
                    left &= left - 1;
                    at * 64 + bit
                })
            })
        })
    }

    /// The words from row `from` on, which is a multiple of 64.
    pub(super) fn words_from(&mut self, from: usize) -> &mut [u64] {
        &mut self.words[from / 64..]
    }
}

/// A word, folded as the quick filter folds text, to look for in text.
pub(super) struct Needle {
    text: Box<str>,
    ascii: bool,
}

impl Needle {
    /// `folded` is not empty.
    pub(super) fn new(folded: &str) -> Self {
        debug_assert!(!folded.is_empty());
        Needle {
            ascii: folded.is_ascii(),
            text: folded.into(),
        }
    }

    pub(super) fn text(&self) -> &str {
        &self.text
    }

    /// Whether every char of the needle is ASCII.
    pub(super) fn is_ascii(&self) -> bool {
        self.ascii
    }

    pub(super) fn bytes(&self) -> &[u8] {
        self.text.as_bytes()
    }
}

/// How common a byte is in text: the rarer two bytes of a needle find its
/// candidates.
fn commonness(byte: u8) -> u8 {
    match byte {
        b' ' => 255,
        b'e' | b't' | b'a' | b'o' | b'i' | b'n' => 250,
        b's' | b'r' | b'h' | b'l' | b'd' | b'c' | b'u' => 220,
        b'0'..=b'9' => 200,
        b'a'..=b'z' => 150,
        0x80.. => 60,
        _ => 100,
    }
}

/// A byte every candidate must have at one offset: `hay[at + offset] | or`
/// equals `val`. Setting bit 5 of both sides ignores the case of a letter.
#[derive(Clone, Copy)]
struct Probe {
    offset: usize,
    or: u8,
    val: u8,
}

/// Where a needle is, in bytes of text.
struct Search<'a> {
    needle: &'a [u8],
    /// Whether ASCII letters match in either case.
    ascii_case: bool,
    probes: [Probe; 2],
    reach: usize,
}

const ONES: u64 = 0x0101_0101_0101_0101;
const LOW_SEVENS: u64 = 0x7f7f_7f7f_7f7f_7f7f;

/// `0x80` in each byte of `x` that is zero, and nowhere else.
#[inline(always)]
fn zero_bytes(x: u64) -> u64 {
    !(((x & LOW_SEVENS).wrapping_add(LOW_SEVENS)) | x | LOW_SEVENS)
}

#[inline(always)]
fn load(hay: &[u8], at: usize) -> u64 {
    u64::from_le_bytes(hay[at..at + 8].try_into().expect("eight bytes"))
}

impl<'a> Search<'a> {
    /// `needle` is not empty, and is ASCII when `ascii_case`.
    fn new(needle: &'a [u8], ascii_case: bool) -> Self {
        let probe = |offset: usize| {
            let byte = needle[offset];
            if ascii_case && byte.is_ascii_alphabetic() {
                Probe {
                    offset,
                    or: 0x20,
                    val: byte.to_ascii_lowercase(),
                }
            } else {
                Probe {
                    offset,
                    or: 0,
                    val: byte,
                }
            }
        };
        let mut by_rarity: Vec<usize> = (0..needle.len()).collect();
        by_rarity.sort_by_key(|&at| commonness(needle[at]));
        let first = by_rarity[0];
        let second = by_rarity.get(1).copied().unwrap_or(first);
        let probes = [probe(first), probe(second)];
        Search {
            needle,
            ascii_case,
            reach: first.max(second),
            probes,
        }
    }

    #[inline(always)]
    fn holds_at(&self, hay: &[u8], at: usize) -> bool {
        let text = &hay[at..at + self.needle.len()];
        if self.ascii_case {
            text.iter()
                .zip(self.needle)
                .all(|(h, n)| h.to_ascii_lowercase() == *n)
        } else {
            text == self.needle
        }
    }

    /// The first place at or after `from` where the needle is.
    fn find(&self, hay: &[u8], from: usize) -> Option<usize> {
        let last = hay.len().checked_sub(self.needle.len())?;
        let [a, b] = self.probes;
        let (a_or, a_val) = (ONES * u64::from(a.or), ONES * u64::from(a.val));
        let (b_or, b_val) = (ONES * u64::from(b.or), ONES * u64::from(b.val));
        let mut at = from;
        while at + 8 + self.reach <= hay.len() {
            let a_hit = zero_bytes((load(hay, at + a.offset) | a_or) ^ a_val);
            let b_hit = zero_bytes((load(hay, at + b.offset) | b_or) ^ b_val);
            let mut hits = a_hit & b_hit;
            while hits != 0 {
                let place = at + (hits.trailing_zeros() / 8) as usize;
                if place <= last && self.holds_at(hay, place) {
                    return Some(place);
                }
                hits &= hits - 1;
            }
            at += 8;
        }
        (at..=last).find(|&place| self.holds_at(hay, place))
    }
}

/// The first byte of `hay` at or after `from` that is not ASCII.
pub(super) fn next_non_ascii(hay: &[u8], from: usize) -> Option<usize> {
    let mut at = from;
    while at + 32 <= hay.len()
        && (load(hay, at) | load(hay, at + 8) | load(hay, at + 16) | load(hay, at + 24))
            & 0x8080_8080_8080_8080
            == 0
    {
        at += 32;
    }
    while at + 8 <= hay.len() {
        let high = load(hay, at) & 0x8080_8080_8080_8080;
        if high != 0 {
            return Some(at + (high.trailing_zeros() / 8) as usize);
        }
        at += 8;
    }
    (at..hay.len()).find(|&place| hay[place] >= 0x80)
}

/// The first row at or after `row` whose run ends after `at`.
#[inline]
pub(super) fn row_of(ends: &[u32], mut row: usize, at: usize) -> usize {
    for _ in 0..4 {
        if ends[row] as usize > at {
            return row;
        }
        row += 1;
    }
    row + ends[row..].partition_point(|&end| end as usize <= at)
}

/// Marks in `out` (bit `row`) each row whose text holds `needle`. `hay` is the
/// rows' text back to back and `ends[row]` closes row's run; `rows`, when
/// given, names the row each run stands for. With `ascii_case`, `needle` is
/// ASCII and ASCII letters in `hay` match in either case.
pub(super) fn mark_rows(
    hay: &[u8],
    ends: &[u32],
    rows: Option<&[u16]>,
    needle: &[u8],
    ascii_case: bool,
    out: &mut [u64],
) {
    if ends.is_empty() {
        return;
    }
    let search = Search::new(needle, ascii_case);
    let (mut from, mut run) = (0, 0);
    while let Some(at) = search.find(hay, from) {
        run = row_of(ends, run, at);
        let end = ends[run] as usize;
        if at + needle.len() <= end {
            let row = rows.map_or(run, |rows| usize::from(rows[run]));
            out[row / 64] |= 1 << (row % 64);
            from = end;
        } else {
            from = at + 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    /// The rows of `text` holding `needle`, asked of each row alone.
    fn by_row(text: &[String], needle: &str, ascii_case: bool) -> Vec<usize> {
        let found = |row: &String| match ascii_case {
            true => row.to_ascii_lowercase().contains(needle),
            false => row.contains(needle),
        };
        (0..text.len()).filter(|&row| found(&text[row])).collect()
    }

    /// The rows `mark_rows` marks for `text`, the rows' bytes back to back.
    fn marked(text: &[String], needle: &str, ascii_case: bool) -> Vec<usize> {
        let hay: String = text.concat();
        let ends: Vec<u32> = text
            .iter()
            .scan(0, |end, row| {
                *end += row.len() as u32;
                Some(*end)
            })
            .collect();
        let mut out = vec![0u64; text.len().div_ceil(64)];
        mark_rows(
            hay.as_bytes(),
            &ends,
            None,
            needle.as_bytes(),
            ascii_case,
            &mut out,
        );
        (0..text.len())
            .filter(|&row| out[row / 64] >> (row % 64) & 1 == 1)
            .collect()
    }

    fn row_text() -> impl Strategy<Value = String> {
        prop::collection::vec(
            prop::sample::select(&["a", "b", "A", "B", "s", "e", "É", "é", " ", "1"][..]),
            0..24,
        )
        .prop_map(|parts| parts.concat())
    }

    proptest! {
        /// Finding a needle in the rows' bytes marks the rows that hold it,
        /// not those where it straddles two rows.
        #[test]
        fn marks_the_rows_that_hold_the_needle(
            text in prop::collection::vec(row_text(), 0..150),
            needle in prop::collection::vec(prop::sample::select(&["a", "b", "s", "e", "é", " ", "1"][..]), 1..5),
            ascii_case in any::<bool>(),
        ) {
            let needle = needle.concat();
            prop_assume!(!ascii_case || needle.is_ascii());
            prop_assert_eq!(
                marked(&text, &needle, ascii_case),
                by_row(&text, &needle, ascii_case),
                "{:?} in {:?}", needle, text
            );
        }

        #[test]
        fn a_set_of_rows_holds_those_rows(
            len in 0..300usize,
            from in 0..300usize,
            to in 0..300usize,
        ) {
            let (from, to) = (from.min(len), to.min(len));
            let rows = from.min(to)..to.max(from);
            let set = RowSet::of(len, rows.clone());
            prop_assert_eq!(set.rows().collect::<Vec<_>>(), rows.clone().collect::<Vec<_>>());
            prop_assert_eq!(set.count(), rows.len());
        }

        #[test]
        fn the_next_non_ascii_byte_is_the_first_found(
            text in prop::collection::vec(row_text(), 0..20),
            from in 0..300usize,
        ) {
            let hay = text.concat();
            let bytes = hay.as_bytes();
            let from = from.min(bytes.len());
            prop_assert_eq!(
                next_non_ascii(bytes, from),
                (from..bytes.len()).find(|&at| bytes[at] >= 0x80)
            );
        }
    }

    #[test]
    fn a_needle_across_two_rows_is_in_neither() {
        let text = [
            "xse".to_string(),
            "atx".to_string(),
            "seat".to_string(),
            String::new(),
        ];
        assert_eq!(marked(&text, "seat", false), [2]);
        assert_eq!(marked(&text, "seat", true), [2]);
    }

    #[test]
    fn ascii_case_ignores_only_ascii_letters() {
        let text = [
            "SEAT".to_string(),
            "Seat".to_string(),
            "sEÁt".to_string(),
            "se@t".to_string(),
        ];
        assert_eq!(marked(&text, "seat", true), [0, 1]);
        assert_eq!(marked(&text, "seat", false), Vec::<usize>::new());
        assert_eq!(marked(&text, "@", true), [3]);
    }
}
