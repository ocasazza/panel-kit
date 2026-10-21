//! Semantic table model shared by web and terminal painters.

use serde::{Deserialize, Serialize};

use crate::badge::Rgb;

use super::data_table::{Align, ColumnKind, DataColumnSpec, TableData, TableDataBuilder, Value};

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
    /// What the column holds; absent reads as text.
    #[serde(default)]
    pub kind: ColumnKind,
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

impl TableColumn {
    /// The interactive table's declaration of this column: its kind decides
    /// how rows sort and which filter control it gets.
    pub fn data_column(&self) -> DataColumnSpec {
        let align = match self.align {
            TextAlign::Left => Align::Left,
            TextAlign::Center => Align::Center,
            TextAlign::Right => Align::Right,
        };
        let spec = DataColumnSpec::new(self.key.clone(), self.title.clone()).align(align);
        let spec = match self.kind {
            ColumnKind::Text => spec,
            ColumnKind::Enum => spec.enumerated(),
            ColumnKind::Number => spec.number(),
        };
        match self.width {
            ColumnWidth::Fixed { value } => spec.width(format!("{value}ch")),
            ColumnWidth::Flex { .. } => spec,
        }
    }
}

impl TableCell {
    /// The cell's value as the interactive table sorts, filters and exports a
    /// column of `kind`. In a number column a meter is its ratio and text is
    /// its parsed number; what is no number is missing, which sorts last.
    pub fn value(&self, kind: ColumnKind) -> Value<'_> {
        match (kind, self) {
            (ColumnKind::Number, TableCell::Meter { ratio, .. }) => Value::Number(*ratio),
            (ColumnKind::Number, TableCell::Text(text)) => text
                .trim()
                .parse::<f64>()
                .map_or(Value::Missing, Value::Number),
            (ColumnKind::Number, TableCell::Status { .. }) => Value::Missing,
            (_, TableCell::Text(text)) => Value::Text(text),
            (_, TableCell::Status { label, .. }) => Value::Text(label),
            (_, TableCell::Meter { text, .. }) => Value::Text(text),
        }
    }
}

impl TableModel {
    /// The interactive table's column declarations, one per column.
    pub fn data_columns(&self) -> Vec<DataColumnSpec> {
        self.columns.iter().map(TableColumn::data_column).collect()
    }

    /// The rows by column for the interactive table, each cell valued by its
    /// column's kind. Row ids and tags carry over.
    pub fn data(&self) -> TableData {
        let columns = self.data_columns();
        let mut builder = TableDataBuilder::with_capacity(&columns, self.rows.len());
        for row in &self.rows {
            let values = row
                .cells
                .iter()
                .zip(&self.columns)
                .map(|(cell, column)| cell.value(column.kind));
            builder.row(&row.id, values).tags(&row.tags);
        }
        builder.build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::data_table::{
        natural_cmp, BuildOptions, SortDir, SortSpec, TableDefaults, TableState, View,
    };
    use proptest::prelude::*;

    #[test]
    fn table_model_carries_columns_and_tagged_cells() {
        let table = TableModel {
            columns: vec![TableColumn {
                key: "service".into(),
                title: "Service".into(),
                width: ColumnWidth::Flex { weight: 2 },
                align: TextAlign::Left,
                kind: ColumnKind::Text,
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

    #[test]
    fn a_column_without_a_kind_decodes_as_text_and_a_kind_round_trips() {
        let bare = r#"{"key":"k","title":"K","width":{"Flex":{"weight":1}},"align":"left"}"#;
        let column: TableColumn = serde_json::from_str(bare).unwrap();
        assert_eq!(column.kind, ColumnKind::Text);

        for kind in [ColumnKind::Text, ColumnKind::Enum, ColumnKind::Number] {
            let typed = TableColumn {
                kind,
                ..column.clone()
            };
            let json = serde_json::to_string(&typed).unwrap();
            assert_eq!(serde_json::from_str::<TableColumn>(&json).unwrap(), typed);
        }
        let number = bare.replace(r#""align":"left""#, r#""align":"left","kind":"number""#);
        assert_eq!(
            serde_json::from_str::<TableColumn>(&number).unwrap().kind,
            ColumnKind::Number
        );
    }

    #[test]
    fn a_declared_alignment_wins_whatever_the_columns_kind() {
        let cases = [
            (TextAlign::Left, Align::Left),
            (TextAlign::Center, Align::Center),
            (TextAlign::Right, Align::Right),
        ];
        for kind in [ColumnKind::Text, ColumnKind::Enum, ColumnKind::Number] {
            for (declared, painted) in cases {
                let column = TableColumn {
                    align: declared,
                    kind,
                    ..typed_column("k", kind)
                };
                assert_eq!(column.data_column().align, painted, "{kind:?} {declared:?}");
            }
        }
    }

    fn typed_column(key: &str, kind: ColumnKind) -> TableColumn {
        TableColumn {
            key: key.into(),
            title: key.into(),
            width: ColumnWidth::Flex { weight: 1 },
            align: TextAlign::Left,
            kind,
        }
    }

    /// Row ids in the order the interactive table shows them sorted on its first column.
    fn sorted_on_first(model: &TableModel, dir: SortDir) -> Vec<String> {
        let columns = model.data_columns();
        let data = model.data();
        let defaults = TableDefaults {
            sort: vec![SortSpec::new(columns[0].id.clone(), dir)],
            ..TableDefaults::default()
        };
        let state = TableState::new(&columns, defaults);
        View::build(&columns, &data, &state, BuildOptions::default())
            .ids(&data)
            .map(str::to_owned)
            .collect()
    }

    /// `values` in the order a numeric sort shows them: present ones by value, then the missing.
    fn numeric_order(values: &[Option<f64>], dir: SortDir) -> Vec<Option<f64>> {
        let mut present: Vec<f64> = values.iter().flatten().copied().collect();
        present.sort_by(f64::total_cmp);
        if dir == SortDir::Desc {
            present.reverse();
        }
        let missing = values.iter().filter(|value| value.is_none()).count();
        present
            .into_iter()
            .map(Some)
            .chain(std::iter::repeat_n(None, missing))
            .collect()
    }

    /// Values with decimals and negatives, some missing.
    fn numbers() -> impl Strategy<Value = Vec<Option<f64>>> {
        let number = (-5000i32..5000).prop_map(|hundredths| f64::from(hundredths) / 100.0);
        prop::collection::vec(prop::option::weighted(0.8, number), 0..30)
    }

    fn shown(values: &[Option<f64>], order: &[String]) -> Vec<Option<f64>> {
        order
            .iter()
            .map(|id| values[id[1..].parse::<usize>().unwrap()])
            .collect()
    }

    proptest! {
        #[test]
        fn a_number_column_of_text_sorts_by_value_not_by_what_it_prints(values in numbers()) {
            let model = TableModel {
                columns: vec![typed_column("n", ColumnKind::Number)],
                rows: values
                    .iter()
                    .enumerate()
                    .map(|(at, value)| TableRow {
                        id: format!("r{at}"),
                        tags: Vec::new(),
                        cells: vec![TableCell::Text(value.map_or("n/a".to_owned(), |value| format!("{value}")))],
                    })
                    .collect(),
            };
            for dir in [SortDir::Asc, SortDir::Desc] {
                prop_assert_eq!(shown(&values, &sorted_on_first(&model, dir)), numeric_order(&values, dir));
            }
        }

        #[test]
        fn a_number_column_of_meters_sorts_by_ratio_not_by_display_text(values in numbers()) {
            let model = TableModel {
                columns: vec![typed_column("load", ColumnKind::Number)],
                rows: values
                    .iter()
                    .enumerate()
                    .map(|(at, value)| TableRow {
                        id: format!("r{at}"),
                        tags: Vec::new(),
                        cells: vec![match value {
                            Some(ratio) => TableCell::Meter { ratio: *ratio, text: format!("{}%", ratio * 100.0), color: None },
                            None => TableCell::Status { label: "none".into(), color: (0, 0, 0) },
                        }],
                    })
                    .collect(),
            };
            for dir in [SortDir::Asc, SortDir::Desc] {
                prop_assert_eq!(shown(&values, &sorted_on_first(&model, dir)), numeric_order(&values, dir));
            }
        }

        #[test]
        fn an_enum_column_offers_its_distinct_values_as_filter_options(
            labels in prop::collection::vec(prop::sample::select(vec!["ok", "warn", "error", "", "node10", "node9"]), 0..30),
        ) {
            let model = TableModel {
                columns: vec![typed_column("state", ColumnKind::Enum)],
                rows: labels
                    .iter()
                    .enumerate()
                    .map(|(at, label)| TableRow {
                        id: format!("r{at}"),
                        tags: Vec::new(),
                        cells: vec![if at % 2 == 0 {
                            TableCell::Status { label: (*label).into(), color: (0, 0, 0) }
                        } else {
                            TableCell::Text((*label).into())
                        }],
                    })
                    .collect(),
            };
            let columns = model.data_columns();
            let state = TableState::new(&columns, TableDefaults::default());
            let view = View::build(&columns, &model.data(), &state, BuildOptions::default());
            let mut distinct: Vec<String> =
                labels.iter().filter(|label| !label.is_empty()).map(|label| (*label).to_owned()).collect();
            distinct.sort_by(|a, b| natural_cmp(a, b));
            distinct.dedup();
            prop_assert_eq!(view.options.get("state").cloned().unwrap_or_default(), distinct);
        }
    }
}
