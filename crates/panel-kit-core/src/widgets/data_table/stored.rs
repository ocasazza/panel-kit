//! A stored table payload, read leniently from any serde format: each field
//! is checked on its own, so one malformed field cannot cost the rest.

use std::collections::BTreeMap;

use serde::de::IgnoredAny;
use serde::{Deserialize, Deserializer};

use super::state::{Density, ViewMode};

/// One stored field: absent, a value of the expected shape, or junk.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Field<T> {
    /// The payload does not name it.
    #[default]
    Absent,
    /// A value of the expected shape.
    Value(T),
    /// Something else; the current value stays.
    Junk,
}

impl<T> Field<T> {
    /// The value, when the field holds one.
    pub fn value(&self) -> Option<&T> {
        match self {
            Field::Value(value) => Some(value),
            Field::Absent | Field::Junk => None,
        }
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Raw<T> {
    Value(T),
    Junk(IgnoredAny),
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Field<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(match Raw::<T>::deserialize(deserializer)? {
            Raw::Value(value) => Field::Value(value),
            Raw::Junk(_) => Field::Junk,
        })
    }
}

/// A number, or text that parses as one.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum Numeric {
    /// A number.
    Number(f64),
    /// Text, parsed when read.
    Text(String),
}

impl Numeric {
    /// The finite number it spells.
    pub fn get(&self) -> Option<f64> {
        match self {
            Numeric::Number(n) => Some(*n),
            Numeric::Text(text) => text.trim().parse().ok(),
        }
        .filter(|n: &f64| n.is_finite())
    }
}

/// A stored column filter: text, a list of values, or `{min, max}`.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum StoredFilter {
    /// Contains-text.
    Text(String),
    /// One of these values; entries that are not text are dropped.
    Values(Vec<Field<String>>),
    /// Numeric bounds.
    Range {
        /// Lower bound.
        #[serde(default)]
        min: Field<Numeric>,
        /// Upper bound.
        #[serde(default)]
        max: Field<Numeric>,
    },
}

/// What [`TableState::restore`](super::TableState::restore) reads: the
/// [`PersistedTable`](super::PersistedTable) shape with every field lenient.
/// Unknown fields are ignored.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct StoredTable {
    /// `[column, "asc"|"desc"]` pairs.
    pub sort: Field<Vec<Field<(String, String)>>>,
    /// Filters by column id.
    pub filters: Field<BTreeMap<String, Field<StoredFilter>>>,
    /// The quick filter.
    pub quick: Field<String>,
    /// Whether the filter row was open.
    pub filters_open: Field<bool>,
    /// Column order.
    pub order: Field<Vec<Field<String>>>,
    /// Hidden column ids.
    pub hidden: Field<Vec<Field<String>>>,
    /// Dragged widths.
    pub widths: Field<BTreeMap<String, Field<Numeric>>>,
    /// Grouping column; `null` ungroups.
    pub group_by: Field<Option<String>>,
    /// Density.
    pub density: Field<Density>,
    /// Rows or cards.
    pub view: Field<ViewMode>,
}
