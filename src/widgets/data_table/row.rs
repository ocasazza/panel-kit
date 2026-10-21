//! One row as a host hands it to the table: a rendered cell per declared
//! column, plus the sort key and plain text the model reads.

use dioxus::prelude::*;
use panel_kit_core::widgets::data_table::{RowKeys, SortKey};

/// One data-table row. Cells are pushed in column-declaration order.
#[derive(Clone)]
pub struct DataRow {
    /// Stable identity: the DOM key, selection and cursor follow it.
    pub key: String,
    /// Rendered cell content, one per declared column; the table supplies the
    /// `td`.
    pub cells: Vec<Element>,
    /// Sort keys, one per declared column.
    pub keys: Vec<SortKey>,
    /// Plain text per declared column: filters, groups and exports read it.
    pub text: Vec<String>,
    /// Tags drawn in the row's ribbon.
    pub tags: Vec<String>,
    /// Extra classes on the `tr` (e.g. `"pk-dt-muted"` for a finished row).
    pub class: Option<String>,
}

impl PartialEq for DataRow {
    // Cells capture event handlers; never skip a render on a false "equal".
    fn eq(&self, _: &Self) -> bool {
        false
    }
}

impl DataRow {
    /// Empty row; push cells in column order.
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            cells: Vec::new(),
            keys: Vec::new(),
            text: Vec::new(),
            tags: Vec::new(),
            class: None,
        }
    }

    /// The row's tags.
    pub fn tags(mut self, tags: &[&str]) -> Self {
        self.tags = tags.iter().map(|tag| tag.to_string()).collect();
        self
    }

    /// Extra classes on the row.
    pub fn class(mut self, class: impl Into<String>) -> Self {
        self.class = Some(class.into());
        self
    }

    /// A finished row, drawn in the dim ink.
    pub fn muted(self) -> Self {
        self.class("pk-dt-muted")
    }

    /// Text cell: rendered, sorted and filtered by the same string.
    pub fn text(self, value: impl Into<String>) -> Self {
        let value = value.into();
        let element = rsx! { "{value}" };
        self.cell(element, SortKey::text(&value), value)
    }

    /// Text cell wrapped in a span carrying `class`.
    pub fn text_class(self, value: impl Into<String>, class: &'static str) -> Self {
        let value = value.into();
        let element = rsx! { span { class: "{class}", "{value}" } };
        self.cell(element, SortKey::text(&value), value)
    }

    /// Numeric cell rendered as `display`, sorted by `value`; `None` sorts
    /// last either way.
    pub fn num(self, value: Option<f64>, display: impl Into<String>) -> Self {
        self.cell(
            rsx! { {display.into()} },
            SortKey::opt_num(value),
            value.map(fmt_num).unwrap_or_default(),
        )
    }

    /// Any rendered cell with its sort key and plain text.
    pub fn cell(mut self, element: Element, key: SortKey, text: impl Into<String>) -> Self {
        self.cells.push(element);
        self.keys.push(key);
        self.text.push(text.into());
        self
    }

    /// The model's view of this row; `ribbon` false hides its tags from the
    /// tag filter.
    pub fn keys(&self, ribbon: bool) -> RowKeys<'_> {
        RowKeys {
            id: &self.key,
            tags: ribbon.then_some(self.tags.as_slice()),
            keys: &self.keys,
            text: &self.text,
        }
    }
}

fn fmt_num(value: f64) -> String {
    if value.fract() == 0.0 && value.abs() < 1e15 {
        format!("{}", value as i64)
    } else {
        value.to_string()
    }
}
