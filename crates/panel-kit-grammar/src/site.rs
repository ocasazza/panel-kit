//! Renderer-neutral site intermediate representation.
//!
//! Every grammar engine maps input bytes into this one shape. Object and
//! morphism order is the source order each engine observes, so two engines over
//! order-equivalent inputs compare equal with `PartialEq`. Properties are a
//! `BTreeMap`, so field order never affects equality.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::error::SiteError;

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

    /// The full subsite on objects whose kind sorts into `sorts`: those objects
    /// and every morphism between two of them. Unsorted kinds are dropped.
    pub fn restrict_objects(&self, sorting: &BTreeMap<String, String>, sorts: &BTreeSet<String>) -> Site {
        let objects: Vec<Object> = self
            .objects
            .iter()
            .filter(|object| sorting.get(&object.kind).is_some_and(|sort| sorts.contains(sort)))
            .cloned()
            .collect();
        let kept: HashSet<&str> = objects.iter().map(|object| object.id.as_str()).collect();
        let morphisms = self
            .morphisms
            .iter()
            .filter(|morphism| kept.contains(morphism.domain.as_str()) && kept.contains(morphism.codomain.as_str()))
            .cloned()
            .collect();
        Site { objects, morphisms }
    }

    /// Every object, and the morphisms whose kind sorts into `sorts`. An
    /// untyped morphism has no sort and lies outside every restriction.
    pub fn restrict_morphisms(&self, sorting: &BTreeMap<String, String>, sorts: &BTreeSet<String>) -> Site {
        let morphisms = self
            .morphisms
            .iter()
            .filter(|morphism| {
                morphism
                    .kind
                    .as_ref()
                    .and_then(|kind| sorting.get(kind))
                    .is_some_and(|sort| sorts.contains(sort))
            })
            .cloned()
            .collect();
        Site { objects: self.objects.clone(), morphisms }
    }

    /// The pushout of `self ← R → other`, where `R` is the discrete site of the
    /// objects of `along_kind` both sides present under one id. Identified
    /// objects must be equal; any other shared id is an error.
    pub fn glue(&self, other: &Site, along_kind: &str) -> Result<Site, SiteError> {
        let mine: HashMap<&str, &Object> = self.objects.iter().map(|object| (object.id.as_str(), object)).collect();
        let mut objects = self.objects.clone();
        for object in &other.objects {
            match mine.get(object.id.as_str()) {
                None => objects.push(object.clone()),
                Some(existing) if existing.kind == along_kind && object.kind == along_kind => {
                    if *existing != object {
                        return Err(SiteError::GlueDisagrees { id: object.id.clone() });
                    }
                }
                Some(_) => return Err(SiteError::IdCollision { id: object.id.clone() }),
            }
        }
        let mut morphisms = self.morphisms.clone();
        morphisms.extend(other.morphisms.iter().cloned());
        Ok(Site { objects, morphisms })
    }
}

impl Object {
    /// Parse a property as `f64`, or `None` when absent or non-numeric.
    pub fn number(&self, key: &str) -> Option<f64> {
        self.fields.get(key).and_then(|value| value.parse::<f64>().ok())
    }
}
