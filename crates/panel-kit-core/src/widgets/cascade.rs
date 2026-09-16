//! Renderer-agnostic N-level Miller-columns single-select model.
//!
//! The host owns the item list and [`CascadeState`]. Renderers derive the open
//! columns and report committed selections or popup visibility changes as
//! [`CascadeAction`].

use serde::{Deserialize, Serialize};

/// One selectable leaf in a cascade.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CascadeItem {
    /// Ancestor category values, outermost first.
    pub path: Vec<String>,
    /// Stable machine value emitted on select.
    pub value: String,
    /// Human label rendered in the leaf's column.
    pub label: String,
}

/// One row in a cascade column.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CascadeEntry {
    /// Category key for descent or stable leaf value for selection.
    pub key: String,
    /// Display label.
    pub label: String,
    /// Whether choosing this entry opens the next column.
    pub has_children: bool,
    /// Stable selection value for a leaf; absent for a category.
    pub value: Option<String>,
}

/// Host-owned state for the cascade popup.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CascadeState {
    /// Whether the popup is open.
    pub open: bool,
    /// Category values descended into, outermost first.
    pub path: Vec<String>,
    /// Index highlighted in the deepest open column.
    pub highlighted: Option<usize>,
}

impl CascadeState {
    /// Return a freshly opened popup at its root column.
    pub fn open() -> Self {
        Self {
            open: true,
            ..Default::default()
        }
    }

    /// Descend into a category and reset the keyboard highlight.
    pub fn descend(&mut self, segment: String) {
        self.path.push(segment);
        self.highlighted = None;
    }

    /// Ascend one column and reset the keyboard highlight.
    ///
    /// This is a no-op at the root.
    pub fn ascend(&mut self) {
        self.path.pop();
        self.highlighted = None;
    }
}

/// User intent reported by a cascade painter to its host.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CascadeAction {
    /// The user picked a leaf.
    Select {
        /// The selected leaf's ancestor category values.
        path: Vec<String>,
        /// The selected leaf's stable machine value.
        value: String,
    },
    /// The popup opened or closed.
    OpenChanged {
        /// Whether the popup is now open.
        open: bool,
    },
}

/// Derive the entries in the column addressed by `path`.
///
/// Items whose path equals `path` are leaves in this column. Items with a
/// longer path contribute their next segment as a category in first-seen
/// order. If a segment appears as both a leaf and a category, the category
/// wins because the leaf would otherwise be unreachable.
pub fn column(items: &[CascadeItem], path: &[String]) -> Vec<CascadeEntry> {
    let mut entries: Vec<CascadeEntry> = Vec::new();
    for item in items {
        if item.path.len() < path.len() || !item.path.starts_with(path) {
            continue;
        }

        if item.path.len() == path.len() {
            if entries
                .iter()
                .any(|entry| entry.has_children && entry.key == item.value)
            {
                continue;
            }
            entries.push(CascadeEntry {
                key: item.value.clone(),
                label: item.label.clone(),
                has_children: false,
                value: Some(item.value.clone()),
            });
            continue;
        }

        let segment = &item.path[path.len()];
        entries.retain(|entry| entry.has_children || entry.key != *segment);
        if !entries
            .iter()
            .any(|entry| entry.has_children && entry.key == *segment)
        {
            entries.push(CascadeEntry {
                key: segment.clone(),
                label: segment.clone(),
                has_children: true,
                value: None,
            });
        }
    }
    entries
}

/// Derive every open column: the root plus one column per descended segment.
pub fn columns(items: &[CascadeItem], path: &[String]) -> Vec<Vec<CascadeEntry>> {
    let mut open_columns = vec![column(items, &[])];
    for depth in 1..=path.len() {
        open_columns.push(column(items, &path[..depth]));
    }
    open_columns
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(path: &[&str], value: &str, label: &str) -> CascadeItem {
        CascadeItem {
            path: path.iter().map(|segment| segment.to_string()).collect(),
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
        let keys: Vec<&str> = root.iter().map(|entry| entry.key.as_str()).collect();
        assert_eq!(keys, vec!["spot", "spider"]);
        assert!(root[0].has_children);
        assert!(!root[1].has_children);
        assert_eq!(root[1].value.as_deref(), Some("spider"));
    }

    #[test]
    fn category_wins_when_a_key_is_also_a_leaf() {
        let items = vec![
            item(&[], "spot", "spot leaf"),
            item(&["spot"], "policy", "policy"),
        ];

        let root = column(&items, &[]);
        assert_eq!(root.len(), 1);
        assert_eq!(root[0].key, "spot");
        assert!(root[0].has_children);
        assert_eq!(root[0].value, None);
    }

    #[test]
    fn descended_column_lists_leaves() {
        let category = column(&fixture(), &["spot".into()]);
        let keys: Vec<&str> = category.iter().map(|entry| entry.key.as_str()).collect();
        assert_eq!(keys, vec!["v74", "v72"]);
        assert!(category.iter().all(|entry| entry.has_children));

        let leaves = column(&fixture(), &["spot".into(), "v74".into()]);
        assert_eq!(leaves.len(), 2);
        assert!(leaves.iter().all(|entry| !entry.has_children));
        assert_eq!(leaves[0].value.as_deref(), Some("spot-walk-pbt-v74-000"));
    }

    #[test]
    fn descend_and_ascend_drive_columns() {
        let mut state = CascadeState::open();
        state.descend("spot".into());
        state.descend("v74".into());
        let open_columns = columns(&fixture(), &state.path);
        assert_eq!(open_columns.len(), 3);
        assert_eq!(open_columns[2].len(), 2);
        state.ascend();
        let open_columns = columns(&fixture(), &state.path);
        assert_eq!(open_columns.len(), 2);
        assert_eq!(state.highlighted, None);
    }

    #[test]
    fn unrelated_items_never_leak_into_a_column() {
        let category = column(&fixture(), &["spot".into(), "v72".into()]);
        assert_eq!(category.len(), 1);
        let empty = column(&fixture(), &["nope".into()]);
        assert!(empty.is_empty());
    }

    #[test]
    fn cascade_items_reject_unknown_fields() {
        let error = serde_json::from_value::<CascadeItem>(serde_json::json!({
            "path": ["spot", "v74"],
            "value": "spot-walk-pbt-v74-000",
            "label": "000 — real trot",
            "unexpected": true
        }))
        .expect_err("strict cascade items must reject unknown fields");

        assert!(error.to_string().contains("unknown field"));
    }
}
