//! Renderer-agnostic dropdown model for a grouped, optionally searchable
//! single-select.
//!
//! The host owns the items and [`DropdownState`]. Renderers derive the visible
//! groups from these values and report user intent as [`DropdownAction`].

use serde::{Deserialize, Serialize};

/// One selectable entry. `group` is the optgroup-style bucket label; items
/// with equal `group` render together, in list order.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DropdownItem {
    /// Stable machine value emitted on select (for example, an experiment id).
    pub value: String,
    /// Human label rendered in the list.
    pub label: String,
    /// Group bucket. An empty string represents an ungrouped item.
    pub group: String,
}

/// Host-owned state for the dropdown popup.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DropdownState {
    /// Whether the popup is open.
    pub open: bool,
    /// Current case-insensitive filter query over item labels and values.
    pub query: String,
    /// Index into the filtered list highlighted by keyboard navigation.
    pub highlighted: Option<usize>,
}

impl DropdownState {
    /// Return a freshly opened popup with no query or highlighted item.
    pub fn open() -> Self {
        Self {
            open: true,
            ..Default::default()
        }
    }
}

/// User intent reported by a dropdown painter to its host.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DropdownAction {
    /// The user picked an item.
    Select {
        /// The selected item's stable machine value.
        value: String,
    },
    /// The popup opened or closed.
    OpenChanged {
        /// Whether the popup is now open.
        open: bool,
    },
}

/// Apply a case-insensitive substring filter over item labels and values.
///
/// Groups survive only while they still have visible items.
pub fn filter_items(items: &[DropdownItem], query: &str) -> Vec<DropdownItem> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return items.to_vec();
    }

    items
        .iter()
        .filter(|item| {
            item.label.to_lowercase().contains(&query) || item.value.to_lowercase().contains(&query)
        })
        .cloned()
        .collect()
}

/// Collapse a flat item list into ordered groups in first-seen order.
///
/// Ungrouped items retain their empty group label.
pub fn group_items(items: &[DropdownItem]) -> Vec<(String, Vec<DropdownItem>)> {
    let mut groups: Vec<(String, Vec<DropdownItem>)> = Vec::new();
    for item in items {
        match groups.iter_mut().find(|(group, _)| *group == item.group) {
            Some((_, entries)) => entries.push(item.clone()),
            None => groups.push((item.group.clone(), vec![item.clone()])),
        }
    }
    groups
}

/// Advance the keyboard highlight through a filtered list.
///
/// `None` starts at the first entry, navigation wraps in either direction,
/// and an empty list stays unhighlighted.
pub fn highlight_next(current: Option<usize>, len: usize, forward: bool) -> Option<usize> {
    if len == 0 {
        return None;
    }

    Some(match (current, forward) {
        (None, _) => 0,
        (Some(index), true) => (index + 1) % len,
        (Some(0), false) => len - 1,
        (Some(index), false) => index - 1,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(value: &str, label: &str, group: &str) -> DropdownItem {
        DropdownItem {
            value: value.into(),
            label: label.into(),
            group: group.into(),
        }
    }

    #[test]
    fn filter_is_case_insensitive_over_label_and_value() {
        let items = vec![
            item("v74-000", "real trot", "c1"),
            item("v74-001", "velocity tracker", "c1"),
        ];
        assert_eq!(filter_items(&items, "TROT"), vec![items[0].clone()]);
        assert_eq!(filter_items(&items, "V74-001"), vec![items[1].clone()]);
        assert_eq!(filter_items(&items, ""), items);
    }

    #[test]
    fn group_items_preserves_first_seen_order() {
        let items = vec![
            item("a", "a", "g2"),
            item("b", "b", "g1"),
            item("c", "c", "g2"),
        ];
        let groups = group_items(&items);
        assert_eq!(groups[0].0, "g2");
        assert_eq!(groups[0].1.len(), 2);
        assert_eq!(groups[1].0, "g1");
    }

    #[test]
    fn highlight_wraps_and_handles_empty() {
        assert_eq!(highlight_next(None, 3, true), Some(0));
        assert_eq!(highlight_next(Some(2), 3, true), Some(0));
        assert_eq!(highlight_next(Some(0), 3, false), Some(2));
        assert_eq!(highlight_next(None, 0, true), None);
    }

    #[test]
    fn dropdown_items_reject_unknown_fields() {
        let error = serde_json::from_value::<DropdownItem>(serde_json::json!({
            "value": "v74-000",
            "label": "real trot",
            "group": "c1",
            "unexpected": true
        }))
        .expect_err("strict dropdown items must reject unknown fields");

        assert!(error.to_string().contains("unknown field"));
    }
}
