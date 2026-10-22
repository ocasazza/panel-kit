//! Renderer-agnostic dropdown model: a grouped, optionally searchable
//! single-select. What the items are, which group they belong to, what is
//! selected, whether the popup is open, and what the filter query is.
//!
//! Rendering lives in the shells: the Dioxus `panel-kit` crate draws a DOM
//! popup, `panel-kit-tui` draws a terminal overlay — both over these types,
//! so a host app's selection handling is identical in browser and terminal.
//!
//! The model is deliberately free of async/fetch concerns: the host owns the
//! item list and can swap it live (e.g. the spot gym re-listing a GCS bucket).

/// One selectable entry. `group` is the optgroup-style bucket label; items
/// with equal `group` render together, in list order.
#[derive(Debug, Clone, PartialEq)]
pub struct DropdownItem {
    /// Stable machine value emitted on select (e.g. the experiment id).
    pub value: String,
    /// Human label rendered in the list.
    pub label: String,
    /// Group bucket (e.g. the campaign name). Empty string = ungrouped.
    pub group: String,
}

/// State machine for the dropdown popup.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DropdownState {
    /// Whether the popup is open.
    pub open: bool,
    /// Current filter query (case-insensitive substring over label+value).
    pub query: String,
    /// Index into the *filtered* list that keyboard navigation highlights.
    pub highlighted: Option<usize>,
}

impl DropdownState {
    /// A freshly-opened popup: open, no query, nothing highlighted.
    pub fn open() -> Self {
        Self {
            open: true,
            ..Default::default()
        }
    }
}

/// What the dropdown reports back to the host.
#[derive(Debug, Clone, PartialEq)]
pub enum DropdownAction {
    /// User picked an item; carries its `value`.
    Select {
        /// The selected item's machine value (`DropdownItem::value`).
        value: String,
    },
    /// The popup opened or closed.
    OpenChanged {
        /// Whether the popup is now open.
        open: bool,
    },
}

/// Case-insensitive substring filter over label and value; groups survive
/// only while they still have visible items.
pub fn filter_items(items: &[DropdownItem], query: &str) -> Vec<DropdownItem> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return items.to_vec();
    }
    items
        .iter()
        .filter(|i| i.label.to_lowercase().contains(&q) || i.value.to_lowercase().contains(&q))
        .cloned()
        .collect()
}

/// Collapse a flat item list into ordered groups, first-seen order.
/// Ungrouped items (empty group) come first under an empty label.
pub fn group_items(items: &[DropdownItem]) -> Vec<(String, Vec<DropdownItem>)> {
    let mut out: Vec<(String, Vec<DropdownItem>)> = Vec::new();
    for item in items {
        match out.iter_mut().find(|(g, _)| *g == item.group) {
            Some((_, entries)) => entries.push(item.clone()),
            None => out.push((item.group.clone(), vec![item.clone()])),
        }
    }
    out
}

/// Advance the keyboard highlight through the filtered list; `None` starts
/// at the first entry, wraps at both ends. Empty list stays `None`.
pub fn highlight_next(current: Option<usize>, len: usize, forward: bool) -> Option<usize> {
    if len == 0 {
        return None;
    }
    Some(match (current, forward) {
        (None, _) => 0,
        (Some(i), true) => (i + 1) % len,
        (Some(0), false) => len - 1,
        (Some(i), false) => i - 1,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(v: &str, l: &str, g: &str) -> DropdownItem {
        DropdownItem {
            value: v.into(),
            label: l.into(),
            group: g.into(),
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
}
