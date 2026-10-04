//! The physics base topos, geometric morphisms between topoi, and the physics
//! engine tuning a regime pulls back from the base.
//!
//! A [`GeometricMorphism`] is a map of sort sets φ (presheaves on a discrete
//! category are sort-indexed families). Its categorical operations
//! (`inverse_image`, `direct_image`, `unit`, `counit`, the three subobject
//! maps) act on the **object** sort map; `morphism_sorts` is the parallel map
//! for generating morphisms. [`Physics`] holds the inverse image `p_R*` of the
//! [`BaseTopos`] engine stalks, keyed by regime sort.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::error::MorphismError;
use crate::topos::Topos;

/// The regime-neutral physics base topos: engine particle/bond sorts and the
/// only authored physics parameters.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BaseTopos {
    /// Spec version.
    pub spec_version: u32,
    /// Stable base topos id (e.g. `topos-physics`).
    pub id: String,
    /// Display regime.
    pub regime: String,
    /// Engine stalks: the authored physics parameters.
    pub stalks: BaseStalks,
}

/// Engine stalks keyed by engine sort.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BaseStalks {
    /// Per engine object (particle) sort.
    pub objects: BTreeMap<String, EngineObjectStalk>,
    /// Per engine morphism (bond) sort.
    pub morphisms: BTreeMap<String, EngineMorphismStalk>,
}

/// Physics parameters for one engine object (particle) sort.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EngineObjectStalk {
    /// Display label.
    pub label: String,
    /// Repulsion weight.
    pub repulsion: f64,
    /// Particle mass.
    pub mass: f64,
}

/// Physics parameters for one engine morphism (bond) sort.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EngineMorphismStalk {
    /// Display label.
    pub label: String,
    /// Spring stiffness.
    pub weight: f64,
    /// Spring rest length.
    pub rest_length: f64,
}

impl BaseTopos {
    /// Decode a base topos from JSON, rejecting unknown keys.
    pub fn from_json_str(text: &str) -> Result<Self, MorphismError> {
        serde_json::from_str(text).map_err(|error| MorphismError::Json(error.to_string()))
    }
}

/// A geometric morphism between topoi over the same site, induced by a map of
/// sort sets φ. `f* ⊣ f_*`; on subobjects `∃_f ⊣ f* ⊣ ∀_f`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeometricMorphism {
    /// Stable morphism id.
    pub id: String,
    /// Domain topos id (E).
    pub domain: String,
    /// Codomain topos id (F).
    pub codomain: String,
    /// φ on object sorts: sorts(E) → sorts(F).
    pub object_sorts: BTreeMap<String, String>,
    /// φ on morphism sorts: sorts(E) → sorts(F).
    pub morphism_sorts: BTreeMap<String, String>,
}

impl GeometricMorphism {
    /// Decode a geometric morphism from JSON, rejecting unknown keys.
    pub fn from_json_str(text: &str) -> Result<Self, MorphismError> {
        serde_json::from_str(text).map_err(|error| MorphismError::Json(error.to_string()))
    }

    /// Validate φ is total on sorts(E), lands in sorts(F), and commutes with the
    /// sortings (`φ ∘ sorting_E == sorting_F` on every site kind).
    pub fn validate(&self, e: &Topos, f: &Topos) -> Result<(), MorphismError> {
        validate_sort_map(
            "object",
            &self.object_sorts,
            &e.object_sort_set(),
            &f.object_sort_set(),
            &e.object_sorting,
            &f.object_sorting,
        )?;
        validate_sort_map(
            "morphism",
            &self.morphism_sorts,
            &e.morphism_sort_set(),
            &f.morphism_sort_set(),
            &e.morphism_sorting,
            &f.morphism_sorting,
        )?;
        Ok(())
    }

    /// `(f* G)(s) = G(φ s)` over object sorts.
    pub fn inverse_image<T: Clone>(&self, g: &BTreeMap<String, T>) -> BTreeMap<String, T> {
        let mut out = BTreeMap::new();
        for (s, t) in &self.object_sorts {
            if let Some(value) = g.get(t) {
                out.insert(s.clone(), value.clone());
            }
        }
        out
    }

    /// `(f_* H)(t) = ∏_{φ s = t} H(s)`, components in sort order.
    pub fn direct_image<T: Clone>(&self, h: &BTreeMap<String, T>) -> BTreeMap<String, Vec<T>> {
        let mut out: BTreeMap<String, Vec<T>> = BTreeMap::new();
        for (s, t) in &self.object_sorts {
            if let Some(value) = h.get(s) {
                out.entry(t.clone()).or_default().push(value.clone());
            }
        }
        out
    }

    /// Unit `η: G → f_* f* G`.
    pub fn unit<T: Clone>(&self, g: &BTreeMap<String, T>) -> BTreeMap<String, Vec<T>> {
        self.direct_image(&self.inverse_image(g))
    }

    /// Counit `ε: f* f_* H → H`: pick each object sort's fiber component.
    pub fn counit<T: Clone>(&self, fh: &BTreeMap<String, Vec<T>>) -> BTreeMap<String, T> {
        let mut out = BTreeMap::new();
        for (s, fiber) in fh {
            if let Some(t) = self.object_sorts.get(s) {
                let index = self.fiber_index(s, t);
                if let Some(value) = fiber.get(index) {
                    out.insert(s.clone(), value.clone());
                }
            }
        }
        out
    }

    /// `f*(V)`: preimage of a subobject (set of codomain sorts).
    pub fn pullback_subobject(&self, v: &BTreeSet<String>) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        for (s, t) in &self.object_sorts {
            if v.contains(t.as_str()) {
                out.insert(s.clone());
            }
        }
        out
    }

    /// `∃_f(U)`: image of a subobject (set of domain sorts).
    pub fn image(&self, u: &BTreeSet<String>) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        for (s, t) in &self.object_sorts {
            if u.contains(s.as_str()) {
                out.insert(t.clone());
            }
        }
        out
    }

    /// `∀_f(U) = { t | φ⁻¹(t) ⊆ U }` over sorts with nonempty fiber.
    pub fn universal_image(&self, u: &BTreeSet<String>) -> BTreeSet<String> {
        let mut fibers: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for (s, t) in &self.object_sorts {
            fibers.entry(t.as_str()).or_default().push(s.as_str());
        }
        let mut out = BTreeSet::new();
        for (t, fiber) in fibers {
            if fiber.iter().all(|s| u.contains(*s)) {
                out.insert(t.to_owned());
            }
        }
        out
    }

    /// Compose `self: E → F` with `other: F → G` into `E → G`.
    pub fn compose(&self, other: &GeometricMorphism) -> Result<GeometricMorphism, MorphismError> {
        if self.codomain != other.domain {
            return Err(MorphismError::ComposeMismatch {
                codomain: self.codomain.clone(),
                domain: other.domain.clone(),
            });
        }
        Ok(GeometricMorphism {
            id: format!("{}∘{}", other.id, self.id),
            domain: self.domain.clone(),
            codomain: other.codomain.clone(),
            object_sorts: compose_map("object", &self.object_sorts, &other.object_sorts)?,
            morphism_sorts: compose_map("morphism", &self.morphism_sorts, &other.morphism_sorts)?,
        })
    }

    /// Whether `p_target ∘ self == p_source` on every mapped sort.
    pub fn commutes_over(&self, p_source: &GeometricMorphism, p_target: &GeometricMorphism) -> bool {
        self.validate_over(p_source, p_target).is_ok()
    }

    /// Validating form of [`commutes_over`](Self::commutes_over).
    pub fn validate_over(
        &self,
        p_source: &GeometricMorphism,
        p_target: &GeometricMorphism,
    ) -> Result<(), MorphismError> {
        commute_over_map(
            "object",
            &self.object_sorts,
            &p_source.object_sorts,
            &p_target.object_sorts,
        )?;
        commute_over_map(
            "morphism",
            &self.morphism_sorts,
            &p_source.morphism_sorts,
            &p_target.morphism_sorts,
        )?;
        Ok(())
    }

    /// Rank of `s` within the sorted fiber `φ⁻¹(t)`.
    fn fiber_index(&self, s: &str, t: &str) -> usize {
        self.object_sorts
            .iter()
            .filter(|(key, value)| value.as_str() == t && key.as_str() < s)
            .count()
    }
}

fn validate_sort_map(
    part: &'static str,
    phi: &BTreeMap<String, String>,
    e_sorts: &BTreeSet<String>,
    f_sorts: &BTreeSet<String>,
    sorting_e: &BTreeMap<String, String>,
    sorting_f: &BTreeMap<String, String>,
) -> Result<(), MorphismError> {
    for s in e_sorts {
        let image = phi
            .get(s)
            .ok_or_else(|| MorphismError::NotTotal { part, sort: s.clone() })?;
        if !f_sorts.contains(image) {
            return Err(MorphismError::OutsideCodomain {
                part,
                sort: s.clone(),
                image: image.clone(),
            });
        }
    }
    for (kind, e_sort) in sorting_e {
        let via = phi
            .get(e_sort)
            .ok_or_else(|| MorphismError::NotTotal { part, sort: e_sort.clone() })?;
        match sorting_f.get(kind) {
            Some(f_sort) if f_sort == via => {}
            _ => return Err(MorphismError::NotCommuting { part, at: kind.clone() }),
        }
    }
    Ok(())
}

fn compose_map(
    part: &'static str,
    first: &BTreeMap<String, String>,
    second: &BTreeMap<String, String>,
) -> Result<BTreeMap<String, String>, MorphismError> {
    let mut out = BTreeMap::new();
    for (s, mid) in first {
        let t = second
            .get(mid)
            .ok_or_else(|| MorphismError::ComposeUndefined { part, sort: mid.clone() })?;
        out.insert(s.clone(), t.clone());
    }
    Ok(out)
}

fn commute_over_map(
    part: &'static str,
    f: &BTreeMap<String, String>,
    p_source: &BTreeMap<String, String>,
    p_target: &BTreeMap<String, String>,
) -> Result<(), MorphismError> {
    for (s, image) in f {
        match (p_source.get(s), p_target.get(image)) {
            (Some(direct), Some(via)) if direct == via => {}
            _ => return Err(MorphismError::NotCommuting { part, at: s.clone() }),
        }
    }
    Ok(())
}

/// A regime's engine tuning: the inverse image `p_R*` of the base stalks, keyed
/// by regime sort. Two regimes related by a base-commuting morphism induce the
/// same [`PhysicsSection`](crate::topos::PhysicsSection).
#[derive(Clone, Debug, PartialEq)]
pub struct Physics {
    objects: BTreeMap<String, ObjectTuning>,
    morphisms: BTreeMap<String, MorphismTuning>,
}

/// Engine tuning for one regime object sort.
#[derive(Clone, Debug, PartialEq)]
pub struct ObjectTuning {
    /// Engine particle sort this regime sort maps to.
    pub engine_type: String,
    /// Repulsion from the base.
    pub repulsion: f64,
    /// Mass from the base.
    pub mass: f64,
}

/// Engine tuning for one regime morphism sort.
#[derive(Clone, Debug, PartialEq)]
pub struct MorphismTuning {
    /// Engine bond sort this regime sort maps to.
    pub engine_type: String,
    /// Spring stiffness from the base.
    pub weight: f64,
    /// Rest length from the base.
    pub rest_length: f64,
}

impl Physics {
    /// Build a regime's tuning as `p_R*` of the base stalks. Validates that the
    /// physics morphism goes `regime → base` and is total over the regime's
    /// declared sorts, each landing on a base engine sort.
    pub fn new(base: &BaseTopos, p: &GeometricMorphism, regime: &Topos) -> Result<Self, MorphismError> {
        if p.domain != regime.id {
            return Err(MorphismError::DomainMismatch {
                expected: regime.id.clone(),
                found: p.domain.clone(),
            });
        }
        if p.codomain != base.id {
            return Err(MorphismError::CodomainMismatch {
                expected: base.id.clone(),
                found: p.codomain.clone(),
            });
        }

        let mut objects = BTreeMap::new();
        for sort in regime.stalks.objects.keys() {
            let engine = p
                .object_sorts
                .get(sort)
                .ok_or_else(|| MorphismError::NotTotal { part: "object", sort: sort.clone() })?;
            let base_stalk = base.stalks.objects.get(engine).ok_or_else(|| {
                MorphismError::UnknownBaseSort { part: "object", sort: engine.clone() }
            })?;
            objects.insert(
                sort.clone(),
                ObjectTuning {
                    engine_type: engine.clone(),
                    repulsion: base_stalk.repulsion,
                    mass: base_stalk.mass,
                },
            );
        }

        let mut morphisms = BTreeMap::new();
        for sort in regime.stalks.morphisms.keys() {
            let engine = p
                .morphism_sorts
                .get(sort)
                .ok_or_else(|| MorphismError::NotTotal { part: "morphism", sort: sort.clone() })?;
            let base_stalk = base.stalks.morphisms.get(engine).ok_or_else(|| {
                MorphismError::UnknownBaseSort { part: "morphism", sort: engine.clone() }
            })?;
            morphisms.insert(
                sort.clone(),
                MorphismTuning {
                    engine_type: engine.clone(),
                    weight: base_stalk.weight,
                    rest_length: base_stalk.rest_length,
                },
            );
        }

        Ok(Physics { objects, morphisms })
    }

    /// Tuning for a regime object sort.
    pub fn object(&self, sort: &str) -> Option<&ObjectTuning> {
        self.objects.get(sort)
    }

    /// Tuning for a regime morphism sort.
    pub fn morphism(&self, sort: &str) -> Option<&MorphismTuning> {
        self.morphisms.get(sort)
    }

    /// Empty tuning, for sheaves that resolve no physics expressions.
    #[cfg(test)]
    pub(crate) fn empty() -> Self {
        Physics { objects: BTreeMap::new(), morphisms: BTreeMap::new() }
    }
}
