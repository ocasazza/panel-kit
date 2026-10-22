//! Renderer-agnostic cascade model: an N-level Miller-columns single-select.
//!
//! A flat grouped dropdown stops scaling when the grouping itself becomes the
//! navigation problem (e.g. robot -> campaign -> experiment: the group choice
//! IS the first selection). The cascade renders one column per level; picking
//! a category opens the next column, picking a leaf commits the selection.
//!
//! Rendering lives in the shells (Dioxus `panel-kit` draws the DOM popup);
//! this module is state and derivation only. As with the dropdown, the host
//! owns the item list and may swap it live.

/// One selectable entry. `path` is the chain of ancestor category values
/// (e.g. `["spot", "spot-walk-pbt-v74"]`); the item itself is a leaf in the
/// column addressed by that path. An item with an empty path is selectable
/// at the root column (e.g. a robot with no policies — the category itself
/// is the choice); a robot column may likewise carry a "sandbox" leaf beside
/// its campaign categories.
#[derive(Debug, Clone, PartialEq)]
pub struct CascadeItem {
    /// Ancestor category values, outermost first.
    pub path: Vec<String>,
    /// Stable machine value emitted on select.
    pub value: String,
    /// Human label rendered in the column.
    pub label: String,
}

/// One row in a cascade column: either a category that descends, or a
/// selectable leaf.
#[derive(Debug, Clone, PartialEq)]
pub struct CascadeEntry {
    /// Category value (for descend) or leaf value (for select).
    pub key: String,
    /// Display label.
    pub label: String,
    /// True when picking this entry opens the next column instead of
    /// committing a selection.
    pub has_children: bool,
    /// `Some(value)` for leaf entries; the value passed to
    /// [`CascadeAction::Select`].
    pub value: Option<String>,
}

/// State machine for the cascade popup.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CascadeState {
    /// Whether the popup is open.
    pub open: bool,
    /// Category values the user has descended into (the open columns).
    pub path: Vec<String>,
    /// Index into the deepest open column that keyboard navigation highlights.
    pub highlighted: Option<usize>,
}

impl CascadeState {
    /// A freshly-opened popup: open at the root column, nothing highlighted.
    pub fn open() -> Self {
        Self {
            open: true,
            ..Default::default()
        }
    }

    /// Descend into a category, opening the next column.
    pub fn descend(&mut self, segment: String) {
        self.path.push(segment);
        self.highlighted = None;
    }

    /// Ascend one column (drops the deepest category). No-op at the root.
    pub fn ascend(&mut self) {
        self.path.pop();
        self.highlighted = None;
    }
}

/// What the cascade reports back to the host.
#[derive(Debug, Clone, PartialEq)]
pub enum CascadeAction {
    /// User picked a leaf; carries the ancestor `path` plus the leaf `value`.
    Select {
        /// Ancestor category values (the item's `CascadeItem::path`).
        path: Vec<String>,
        /// The selected item's machine value.
        value: String,
    },
    /// The popup opened or closed.
    OpenChanged {
        /// Whether the popup is now open.
        open: bool,
    },
}

/// Derive the entries of the column at `path` (empty = root column).
///
/// Items whose path equals `path` are leaves in this column; items with a
/// longer path contribute their next segment as a category, first-seen order.
/// A segment that appears as both a leaf and a category is a host modeling
/// error — the category wins (the leaf is unreachable otherwise).
pub fn column(items: &[CascadeItem], path: &[String]) -> Vec<CascadeEntry> {
    let mut out: Vec<CascadeEntry> = Vec::new();
    for item in items {
        if item.path.len() < path.len() || !item.path.starts_with(path) {
            continue;
        }
        if item.path.len() == path.len() {
            // Leaf at this level (item.path == path, possibly both empty).
            out.push(CascadeEntry {
                key: item.value.clone(),
                label: item.label.clone(),
                has_children: false,
                value: Some(item.value.clone()),
            });
            continue;
        }
        let segment = &item.path[path.len()];
        if !out.iter().any(|e| e.has_children && e.key == *segment) {
            out.push(CascadeEntry {
                key: segment.clone(),
                label: segment.clone(),
                has_children: true,
                value: None,
            });
        }
    }
    out
}

/// All open columns for rendering: root plus one per descended segment.
/// Each entry is (column path, entries).
pub fn columns(items: &[CascadeItem], path: &[String]) -> Vec<Vec<CascadeEntry>> {
    let mut out = vec![column(items, &[])];
    for depth in 1..=path.len() {
        out.push(column(items, &path[..depth]));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(path: &[&str], value: &str, label: &str) -> CascadeItem {
        CascadeItem {
            path: path.iter().map(|s| s.to_string()).collect(),
            value: value.into(),
            label: label.into(),
        }
    }

    fn fixture() -> Vec<CascadeItem> {
        vec![
            item(&["spot", "v74"], "spot-walk-pbt-v74-000", "000 — real trot"),
            item(&["spot", "v74"], "spot-walk-pbt-v74-001", "001 — no gait"),
            item(&["spot", "v72"], "spot-walk-pbt-v72-011", "011 — baseline"),
            item(&[], "spider", "spider (sandbox)"),
        ]
    }

    #[test]
    fn root_column_lists_categories_first_seen_plus_root_leaves() {
        let root = column(&fixture(), &[]);
        let keys: Vec<&str> = root.iter().map(|e| e.key.as_str()).collect();
        assert_eq!(keys, vec!["spot", "spider"]);
        assert!(root[0].has_children);
        assert!(!root[1].has_children);
        assert_eq!(root[1].value.as_deref(), Some("spider"));
    }

    #[test]
    fn descended_column_lists_leaves() {
        let col = column(&fixture(), &["spot".into()]);
        let keys: Vec<&str> = col.iter().map(|e| e.key.as_str()).collect();
        assert_eq!(keys, vec!["v74", "v72"]);
        assert!(col.iter().all(|e| e.has_children));

        let leaf = column(&fixture(), &["spot".into(), "v74".into()]);
        assert_eq!(leaf.len(), 2);
        assert!(leaf.iter().all(|e| !e.has_children));
        assert_eq!(leaf[0].value.as_deref(), Some("spot-walk-pbt-v74-000"));
    }

    #[test]
    fn descend_and_ascend_drive_columns() {
        let mut st = CascadeState::open();
        st.descend("spot".into());
        st.descend("v74".into());
        let cols = columns(&fixture(), &st.path);
        assert_eq!(cols.len(), 3);
        assert_eq!(cols[2].len(), 2);
        st.ascend();
        let cols = columns(&fixture(), &st.path);
        assert_eq!(cols.len(), 2);
        assert_eq!(st.highlighted, None);
    }

    #[test]
    fn unrelated_items_never_leak_into_a_column() {
        let col = column(&fixture(), &["spot".into(), "v72".into()]);
        assert_eq!(col.len(), 1);
        let empty = column(&fixture(), &["nope".into()]);
        assert!(empty.is_empty());
    }
}
