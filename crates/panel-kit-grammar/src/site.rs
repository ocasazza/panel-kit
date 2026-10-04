//! Renderer-neutral site intermediate representation.
//!
//! Every grammar engine maps input bytes into this one shape. Object and
//! morphism order is the source order each engine observes, so two engines over
//! order-equivalent inputs compare equal with `PartialEq`. Properties are a
//! `BTreeMap`, so field order never affects equality.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// A parsed site: objects then morphisms, each in source order.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Site {
    /// Objects in source order.
    pub objects: Vec<Object>,
    /// Morphisms in source order.
    pub morphisms: Vec<Morphism>,
}

/// One site object with its metadata and string-valued properties.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Object {
    /// Stable local identifier; morphism endpoints reference this.
    pub id: String,
    /// Human-readable title.
    pub title: String,
    /// Object kind (e.g. `planner`, `coder`, `memory`); its sort in a regime.
    pub kind: String,
    /// Tags in capture order.
    pub tags: Vec<String>,
    /// Declared properties, stored as raw strings keyed by name.
    pub fields: BTreeMap<String, String>,
}

/// One directed generating morphism, optionally typed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Morphism {
    /// Domain object id.
    pub domain: String,
    /// Codomain object id.
    pub codomain: String,
    /// Morphism kind when captured; `None` for untyped morphisms.
    pub kind: Option<String>,
}

impl Site {
    /// Whether an object with `id` exists.
    pub(crate) fn has_object(&self, id: &str) -> bool {
        self.objects.iter().any(|object| object.id == id)
    }

    /// Outgoing morphism count for `id`.
    pub(crate) fn out_degree(&self, id: &str) -> usize {
        self.morphisms.iter().filter(|morphism| morphism.domain == id).count()
    }

    /// Incoming morphism count for `id`.
    pub(crate) fn in_degree(&self, id: &str) -> usize {
        self.morphisms.iter().filter(|morphism| morphism.codomain == id).count()
    }

    /// Total (incoming plus outgoing) degree for `id`.
    pub(crate) fn degree(&self, id: &str) -> usize {
        self.out_degree(id) + self.in_degree(id)
    }
}

impl Object {
    /// Parse a property as `f64`, or `None` when absent or non-numeric.
    pub fn number(&self, key: &str) -> Option<f64> {
        self.fields.get(key).and_then(|value| value.parse::<f64>().ok())
    }
}
