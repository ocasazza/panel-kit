//! Semantic table model shared by web and terminal painters.

use serde::{Deserialize, Serialize};

use crate::badge::Rgb;

/// Horizontal text alignment for a semantic table column.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum TextAlign {
    /// Left-align text.
    Left,
    /// Center-align text.
    Center,
    /// Right-align text.
    Right,
}

/// Renderer-neutral column width request.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
pub enum ColumnWidth {
    /// Fixed width in renderer units.
    Fixed {
        /// Fixed width value.
        value: u16,
    },
    /// Flexible weighted width.
    Flex {
        /// Flexible width weight.
        weight: u16,
    },
}

/// One table column.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TableColumn {
    /// Stable column key.
    pub key: String,
    /// Display title.
    pub title: String,
    /// Requested width.
    pub width: ColumnWidth,
    /// Text alignment.
    pub align: TextAlign,
}

/// Semantic cell variants supported by authored tables.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum TableCell {
    /// Plain text cell.
    Text(String),
    /// Colored status label.
    Status {
        /// Status label.
        label: String,
        /// Status display color.
        color: Rgb,
    },
    /// Inline meter cell.
    Meter {
        /// Fill fraction.
        ratio: f64,
        /// Human-readable value text.
        text: String,
        /// Optional renderer-neutral meter color.
        color: Option<Rgb>,
    },
}

/// One table row.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TableRow {
    /// Stable identity: selection, focus and re-sorts survive a reload by id,
    /// never by position.
    pub id: String,
    /// Cells in column order.
    pub cells: Vec<TableCell>,
    /// Tags a tag filter matches; empty on a table nobody filters by tag.
    #[serde(default)]
    pub tags: Vec<String>,
}

/// Owned semantic table data.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TableModel {
    /// Column definitions.
    pub columns: Vec<TableColumn>,
    /// Table rows.
    pub rows: Vec<TableRow>,
}

/// Borrowed semantic table data for runtime providers: the ordered, filtered
/// window the painter draws, plus the counts an empty state needs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TableView<'a> {
    /// Borrowed column definitions, in the user's order and visibility.
    pub columns: &'a [TableColumn],
    /// The rows to paint, in order.
    pub rows: &'a [&'a TableRow],
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_model_carries_columns_and_tagged_cells() {
        let table = TableModel {
            columns: vec![TableColumn {
                key: "service".into(),
                title: "Service".into(),
                width: ColumnWidth::Flex { weight: 2 },
                align: TextAlign::Left,
            }],
            rows: vec![TableRow {
                id: "api".into(),
                tags: Vec::new(),
                cells: vec![TableCell::Text("api".into())],
            }],
        };

        assert_eq!(table.columns[0].key, "service");
        assert_eq!(table.rows[0].cells[0], TableCell::Text("api".into()));
    }
}
