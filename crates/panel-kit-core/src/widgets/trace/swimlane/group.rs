//! Events grouped by work item in one pass: item ids interned to dense
//! indices through a hash table, so no sort ever compares id strings.

use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};

use super::super::model::{Event, Micros};

/// Multiplicative string hash with a splitmix finaliser: ids are short and
/// differ in their last bytes, which the table's low bits must see.
#[derive(Default)]
pub struct Fx(u64);

const K: u64 = 0x517c_c1b7_2722_0a95;

impl Fx {
    fn mix(&mut self, word: u64) {
        self.0 = (self.0.rotate_left(5) ^ word).wrapping_mul(K);
    }
}

impl Hasher for Fx {
    fn write(&mut self, bytes: &[u8]) {
        let mut chunks = bytes.chunks_exact(8);
        for chunk in &mut chunks {
            let mut word = [0u8; 8];
            word.copy_from_slice(chunk);
            self.mix(u64::from_le_bytes(word));
        }
        let rest = chunks.remainder();
        if !rest.is_empty() {
            let mut word = [0u8; 8];
            word[..rest.len()].copy_from_slice(rest);
            self.mix(u64::from_le_bytes(word));
        }
    }

    fn write_u8(&mut self, byte: u8) {
        self.mix(u64::from(byte));
    }

    fn finish(&self) -> u64 {
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
}

/// A map keyed by short strings, hashed by [`Fx`].
pub type FastMap<K, V> = HashMap<K, V, BuildHasherDefault<Fx>>;

/// What one pass over the events with an item learns about each item.
pub struct Groups {
    /// For each event passed in, the dense index of its item.
    pub item_of: Vec<u32>,
    /// Event index of the first event seen of each item.
    pub first: Vec<u32>,
    /// Newest instant of each item.
    pub last: Vec<Micros>,
    /// Events of each item.
    pub count: Vec<u32>,
}

impl Groups {
    /// Items found.
    pub fn items(&self) -> usize {
        self.first.len()
    }
}

/// Groups the events at `with_item` (indices into the events `get` reads, all
/// with an item id) by that id, items numbered in order of first appearance.
pub fn group<'e>(get: &impl Fn(u32) -> &'e Event, with_item: &[u32]) -> Groups {
    let mut ids: FastMap<&'e str, u32> =
        FastMap::with_capacity_and_hasher(with_item.len() / 4 + 8, BuildHasherDefault::default());
    let mut groups = Groups {
        item_of: Vec::with_capacity(with_item.len()),
        first: Vec::new(),
        last: Vec::new(),
        count: Vec::new(),
    };
    for &index in with_item {
        let event = get(index);
        let id = event.item_id.as_deref().unwrap_or("");
        let next = groups.first.len() as u32;
        let item = *ids.entry(id).or_insert(next);
        if item == next {
            groups.first.push(index);
            groups.last.push(event.at);
            groups.count.push(0);
        }
        let at = item as usize;
        groups.count[at] += 1;
        groups.last[at] = groups.last[at].max(event.at);
        groups.item_of.push(item);
    }
    groups
}
