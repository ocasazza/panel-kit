//! Section edits: typed changes to object data, declared by a topos over its
//! regime sorts. Applying one is a site revision (identity on generators);
//! the grammar's emitter serializes its support for the host to write.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

/// One declared edit.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SectionEdit {
    /// Display label for the action.
    pub label: String,
    /// Regime object sorts the edit applies to.
    pub sorts: BTreeSet<String>,
    /// Fields the edit may set, each with its type.
    pub set: BTreeMap<String, FieldType>,
    /// Id of the grammar whose emitter writes the revision.
    pub grammar: String,
}

/// The values a field accepts. Values are stored in the site's string field
/// model, so both engines round-trip without new value types.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum FieldType {
    /// One of a fixed set of words.
    Enum {
        /// Accepted words.
        values: Vec<String>,
    },
    /// Free text up to `max` characters.
    Text {
        /// Maximum length in characters.
        max: usize,
    },
    /// A decimal integer in `[min, max]`, stored in canonical form.
    Int {
        /// Least accepted value.
        min: i64,
        /// Greatest accepted value.
        max: i64,
    },
    /// Newline-separated lines, at most `max` of them.
    Lines {
        /// Maximum number of lines.
        max: usize,
    },
}

impl FieldType {
    /// The stored form of `value`, or why it does not fit.
    pub fn check(&self, value: &str) -> Result<String, String> {
        match self {
            Self::Enum { values } => {
                if values.iter().any(|word| word == value) {
                    Ok(value.to_owned())
                } else {
                    Err(format!("must be one of {}", values.join(", ")))
                }
            }
            Self::Text { max } => {
                let length = value.chars().count();
                if length <= *max {
                    Ok(value.to_owned())
                } else {
                    Err(format!("is {length} characters; at most {max}"))
                }
            }
            Self::Int { min, max } => match value.trim().parse::<i64>() {
                Ok(number) if (*min..=*max).contains(&number) => Ok(number.to_string()),
                Ok(number) => Err(format!("{number} is outside [{min}, {max}]")),
                Err(_) => Err("is not an integer".to_owned()),
            },
            Self::Lines { max } => {
                let count = if value.is_empty() { 0 } else { value.split('\n').count() };
                if count <= *max {
                    Ok(value.to_owned())
                } else {
                    Err(format!("has {count} lines; at most {max}"))
                }
            }
        }
    }

    /// Why this declaration can accept nothing, if it cannot.
    pub(crate) fn defect(&self) -> Option<String> {
        match self {
            Self::Enum { values } if values.is_empty() => Some("enum has no values".to_owned()),
            Self::Enum { values } if values.iter().collect::<BTreeSet<_>>().len() != values.len() => {
                Some("enum repeats a value".to_owned())
            }
            Self::Int { min, max } if min > max => Some(format!("int range [{min}, {max}] is empty")),
            Self::Text { max: 0 } | Self::Lines { max: 0 } => Some("max = 0 admits no value".to_owned()),
            _ => None,
        }
    }
}

/// What a host submits: the object to edit and the fields to set.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditInput {
    /// Object id.
    pub object: String,
    /// Field name to raw value.
    pub values: BTreeMap<String, String>,
}

/// A site revision `ρ: S → S′` from a set-only edit: the identity on
/// generators; `support` holds the objects whose data changed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SiteRevision {
    /// The edit that produced it.
    pub edit: String,
    /// Objects whose data changed; empty when every value was already set.
    pub support: BTreeSet<String>,
}

/// The emitted revision a host writes; no transport detail. The host binds
/// `grammar` to wherever that grammar's input lives.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WriteIntent {
    /// Grammar id whose emitter produced `bytes`.
    pub grammar: String,
    /// Edit id.
    pub edit: String,
    /// Emitted support objects in the grammar's wire format.
    pub bytes: Vec<u8>,
    /// The revision's support.
    pub support: BTreeSet<String>,
}

/// The object behind one row of an object-scoped table, and the edits a host
/// may offer on it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RowEdits {
    /// Object id.
    pub object: String,
    /// Edit ids applicable to the object's sort, in the sheaf's order.
    pub edits: Vec<String>,
}
