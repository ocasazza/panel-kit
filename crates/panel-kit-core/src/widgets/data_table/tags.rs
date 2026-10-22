//! The ribbon a row carries: its tags in a stable order, the colour slot and
//! letter each segment shows, and the names a screen reader hears.
//!
//! The slot is FNV-1a/32 over the UTF-8 bytes of the lowercased tag, modulo
//! [`TAG_SLOTS`]: a tag's colour must not change across restarts or platforms.

use std::collections::BTreeSet;

use serde::Deserialize;
use serde::Serialize;

/// Colour slots a tag can fill (`--tag-0` … `--tag-7`).
pub const TAG_SLOTS: u32 = 8;
/// Longest canonical tag, in characters.
pub const MAX_TAG_LENGTH: usize = 24;

/// Whether a row must carry one of the filtered tags or all of them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TagMode {
    /// A row passes when it carries any filtered tag.
    #[default]
    Any,
    /// A row passes when it carries every filtered tag.
    All,
}

/// How much of the ribbon a row draws, chosen by the width it is given.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RibbonMode {
    /// Raked, overlapping, lettered segments.
    #[default]
    Full,
    /// Unraked segments.
    Tight,
    /// Thin unlettered stripes.
    Flat,
    /// Only the first tag, plus the `+N` tail.
    Flag,
}

impl RibbonMode {
    /// The `data-tags-mode` value.
    pub fn as_str(self) -> &'static str {
        match self {
            RibbonMode::Full => "full",
            RibbonMode::Tight => "tight",
            RibbonMode::Flat => "flat",
            RibbonMode::Flag => "flag",
        }
    }

    /// The segments drawn before the `+N` tail.
    pub fn shown(self, labels: &[String], limit: usize) -> &[String] {
        let limit = match self {
            RibbonMode::Flag => 1,
            _ => limit,
        };
        &labels[..labels.len().min(limit)]
    }
}

/// The tag as the filter, the legend and the segment all spell it: trimmed,
/// lowercased, at most [`MAX_TAG_LENGTH`] characters.
pub fn canonical_tag(tag: &str) -> String {
    tag.trim()
        .to_lowercase()
        .chars()
        .take(MAX_TAG_LENGTH)
        .collect()
}

/// FNV-1a/32 over the UTF-8 bytes of the lowercased tag.
pub fn tag_hash(tag: &str) -> u32 {
    tag.to_lowercase()
        .bytes()
        .fold(0x811C_9DC5_u32, |hash, byte| {
            (hash ^ u32::from(byte)).wrapping_mul(0x0100_0193)
        })
}

/// Which `--tag-N` colour slot a tag fills.
pub fn tag_slot(tag: &str) -> u32 {
    tag_hash(tag) % TAG_SLOTS
}

/// The first character of the canonical tag, uppercased: the segment's letter.
pub fn tag_letter(tag: &str) -> String {
    canonical_tag(tag)
        .chars()
        .next()
        .map(|c| c.to_uppercase().collect())
        .unwrap_or_else(|| "?".to_string())
}

/// A row's tags, canonical, deduplicated and sorted, so a ribbon never
/// reshuffles when someone filters.
pub fn row_labels(tags: &[String]) -> Vec<String> {
    let labels: BTreeSet<String> = tags
        .iter()
        .map(|tag| canonical_tag(tag))
        .filter(|tag| !tag.is_empty())
        .collect();
    labels.into_iter().collect()
}

/// The ribbon's group name. Tag text stays out of the row's own name, so it
/// is not announced twice.
pub fn ribbon_label(labels: &[String]) -> String {
    format!("tags: {}", labels.join(", "))
}

/// A segment's name, which says whether the tag is filtering.
pub fn segment_label(tag: &str, filter: &BTreeSet<String>) -> String {
    if filter.contains(tag) {
        format!("{tag}, filtering")
    } else {
        format!("{tag}, filter")
    }
}
