//! Topos specifications: one UI/UX regime over a site.
//!
//! A topos is a functor from the site's kinds to a regime's discrete sort
//! category (`object_sorting` / `morphism_sorting`) together with [`Stalks`]
//! keyed by regime sort (label, badge, assembly-stage prose) and a set of
//! [`Sheaf`]s keyed by panel binding id. Physics tuning is not authored here: a
//! regime's repulsion/mass/weight/rest-length come from the physics base via a
//! [`Physics`] (the inverse image `p_R*`). The same site under two topoi yields
//! different panels and labels but the same physics section.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use panel_kit_core::spec::WorkspaceSpec;
use panel_kit_core::widgets::ContentSpec;

use crate::edit::{EditInput, RowEdits, SectionEdit, SiteRevision, WriteIntent};
use crate::error::{EditError, SheafError, ToposError};
use crate::morphism::Physics;
use crate::package::CompiledGrammar;
use crate::sheaf::{GlobalSections, RowScope, Sheaf, SortSpace};
use crate::site::Site;

/// Supported topos spec version.
pub const SPEC_VERSION: u32 = 1;

/// A complete topos: sortings, stalks, sheaves, and the workspace it drives.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Topos {
    /// Spec version; must equal [`SPEC_VERSION`].
    pub spec_version: u32,
    /// Stable topos id.
    pub id: String,
    /// Display regime.
    pub regime: String,
    /// Site object kind to regime sort.
    pub object_sorting: BTreeMap<String, String>,
    /// Site morphism kind to regime sort.
    pub morphism_sorting: BTreeMap<String, String>,
    /// Regime sort to stalk, plus assembly-stage prose.
    pub stalks: Stalks,
    /// Panel-binding id to sheaf map.
    pub sheaves: BTreeMap<String, Sheaf>,
    /// Edit id to section edit; table sheaves offer them per row.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub edits: BTreeMap<String, SectionEdit>,
    /// The workspace (panels + layout) this topos presents.
    pub workspace: WorkspaceSpec,
}

/// The regime's reading of object and morphism sorts, plus stage prose.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Stalks {
    /// Per object-sort stalk.
    pub objects: BTreeMap<String, ObjectStalk>,
    /// Per morphism-sort stalk.
    pub morphisms: BTreeMap<String, MorphismStalk>,
    /// Assembly stage labels for this regime.
    pub stages: Stages,
}

/// How one object sort reads in this regime.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectStalk {
    /// Display label for this sort.
    pub label: String,
    /// Badge kind string (`tag`, `doctype`, `folder`, `author`, `entity`,
    /// `date`, `status`, `generic`).
    pub badge: String,
}

/// How one morphism sort reads in this regime.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MorphismStalk {
    /// Display label for this sort.
    pub label: String,
}

/// Assembly-stage prose for the six topological stages.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Stages {
    /// Path (β1 = 0, max degree ≤ 2).
    pub chain: String,
    /// Tree with a branch (β1 = 0).
    pub branched: String,
    /// Single cycle (β1 = 1, F = 0).
    pub ring: String,
    /// Triangulated patch with boundary (χ = 1).
    pub sheet: String,
    /// Triangulated tube with boundary (χ = 0).
    pub tube: String,
    /// Closed triangulated surface (boundary = 0).
    pub membrane: String,
}

/// Per-object `(id, engine_type, repulsion, mass)` and per-morphism
/// `(domain, codomain, engine_type, weight, rest_length)` over the whole site.
/// Identical under any two regimes related by a base-commuting morphism.
#[derive(Clone, Debug, PartialEq)]
pub struct PhysicsSection {
    /// `(id, engine_type, repulsion, mass)` in site object order.
    pub objects: Vec<(String, String, f64, f64)>,
    /// `(domain, codomain, engine_type, weight, rest_length)` in site morphism order.
    pub morphisms: Vec<(String, String, String, f64, f64)>,
}

impl Topos {
    /// Decode a topos from JSON, rejecting unknown keys.
    pub fn from_json_str(text: &str) -> Result<Self, ToposError> {
        serde_json::from_str(text).map_err(|error| ToposError::Json(error.to_string()))
    }

    /// The embedded workspace spec as JSON, for host spec resolvers.
    pub fn workspace_json(&self) -> String {
        serde_json::to_string(&self.workspace).expect("WorkspaceSpec serializes to JSON")
    }

    /// The regime's object sorts (keys of the object stalks).
    pub(crate) fn object_sort_set(&self) -> BTreeSet<String> {
        self.stalks.objects.keys().cloned().collect()
    }

    /// The regime's morphism sorts (keys of the morphism stalks).
    pub(crate) fn morphism_sort_set(&self) -> BTreeSet<String> {
        self.stalks.morphisms.keys().cloned().collect()
    }

    /// Every object/morphism kind present in the site is sorted, and every sort
    /// it maps to has a stalk.
    pub fn check_site(&self, site: &Site) -> Result<(), SheafError> {
        for object in &site.objects {
            let sort = self
                .object_sorting
                .get(&object.kind)
                .ok_or_else(|| SheafError::UnsortedObjectKind(object.kind.clone()))?;
            if !self.stalks.objects.contains_key(sort) {
                return Err(SheafError::MissingObjectStalk(sort.clone()));
            }
        }
        for morphism in &site.morphisms {
            if let Some(kind) = &morphism.kind {
                let sort = self
                    .morphism_sorting
                    .get(kind)
                    .ok_or_else(|| SheafError::UnsortedMorphismKind(kind.clone()))?;
                if !self.stalks.morphisms.contains_key(sort) {
                    return Err(SheafError::MissingMorphismStalk(sort.clone()));
                }
            }
        }
        Ok(())
    }

    /// Γ of every sheaf: resolve each against the site through `physics`.
    pub fn global_sections(&self, site: &Site, physics: &Physics) -> Result<GlobalSections, SheafError> {
        self.check_site(site)?;
        let resolver = self.resolver(physics);
        let mut map = BTreeMap::new();
        let mut rows = BTreeMap::new();
        for (id, sheaf) in &self.sheaves {
            map.insert(id.clone(), sheaf.evaluate(&resolver, site)?);
            if let Sheaf::Table(table) = sheaf {
                if !table.edits.is_empty() && table.scope == RowScope::Objects {
                    let restricted = sheaf.restricted_site(&self.object_sorting, &self.morphism_sorting, site);
                    let objects = &restricted.as_ref().unwrap_or(site).objects;
                    let offered = objects
                        .iter()
                        .map(|object| RowEdits {
                            object: object.id.clone(),
                            edits: table
                                .edits
                                .iter()
                                .filter(|edit| self.applies(edit, &object.kind))
                                .cloned()
                                .collect(),
                        })
                        .collect();
                    rows.insert(id.clone(), offered);
                }
            }
        }
        Ok(GlobalSections::from_parts(map, rows))
    }

    /// Whether edit `edit` applies to objects of site kind `kind`.
    fn applies(&self, edit: &str, kind: &str) -> bool {
        let sort = self.object_sorting.get(kind);
        self.edits
            .get(edit)
            .is_some_and(|edit| sort.is_some_and(|sort| edit.sorts.contains(sort)))
    }

    /// Apply a set-only edit. Morphisms and every other object are unchanged;
    /// the support is the edited object when a value changed, else empty.
    pub fn revise(&self, site: &Site, edit_id: &str, input: &EditInput) -> Result<(Site, SiteRevision), EditError> {
        let edit = self
            .edits
            .get(edit_id)
            .ok_or_else(|| EditError::UnknownEdit(edit_id.to_owned()))?;
        let index = site
            .objects
            .iter()
            .position(|object| object.id == input.object)
            .ok_or_else(|| EditError::UnknownObject(input.object.clone()))?;
        let object = &site.objects[index];
        let sort = self.object_sorting.get(&object.kind).cloned().unwrap_or_default();
        if !edit.sorts.contains(&sort) {
            return Err(EditError::NotApplicable {
                edit: edit_id.to_owned(),
                object: object.id.clone(),
                sort,
            });
        }
        if input.values.is_empty() {
            return Err(EditError::EmptyEdit(edit_id.to_owned()));
        }
        let mut fields = object.fields.clone();
        for (field, raw) in &input.values {
            let field_type = edit.set.get(field).ok_or_else(|| EditError::UnknownField {
                edit: edit_id.to_owned(),
                field: field.clone(),
            })?;
            let value = field_type.check(raw).map_err(|detail| EditError::InvalidValue {
                field: field.clone(),
                detail,
            })?;
            fields.insert(field.clone(), value);
        }
        let support = if fields == object.fields {
            BTreeSet::new()
        } else {
            BTreeSet::from([object.id.clone()])
        };
        let mut revised = site.clone();
        revised.objects[index].fields = fields;
        Ok((revised, SiteRevision { edit: edit_id.to_owned(), support }))
    }

    /// The bytes a host writes for `revision`, emitted by the edit's grammar.
    pub fn write_intent(
        &self,
        grammar: &CompiledGrammar,
        revised: &Site,
        revision: &SiteRevision,
    ) -> Result<WriteIntent, EditError> {
        let edit = self
            .edits
            .get(&revision.edit)
            .ok_or_else(|| EditError::UnknownEdit(revision.edit.clone()))?;
        if edit.grammar != grammar.id() {
            return Err(EditError::GrammarMismatch {
                edit: revision.edit.clone(),
                expected: edit.grammar.clone(),
                actual: grammar.id().to_owned(),
            });
        }
        let bytes = grammar.emit(revised, &revision.support).map_err(EditError::Emit)?;
        Ok(WriteIntent {
            grammar: edit.grammar.clone(),
            edit: revision.edit.clone(),
            bytes,
            support: revision.support.clone(),
        })
    }

    /// The per-object/per-morphism engine parameters over the whole site.
    pub fn physics_section(&self, site: &Site, physics: &Physics) -> PhysicsSection {
        self.resolver(physics).physics_section(site)
    }

    fn resolver<'a>(&'a self, physics: &'a Physics) -> Resolver<'a> {
        Resolver {
            object_sorting: &self.object_sorting,
            morphism_sorting: &self.morphism_sorting,
            stalks: &self.stalks,
            physics,
        }
    }

    /// Validate the version, the embedded workspace, and the binding ↔
    /// sheaf ↔ content-kind agreement: every sheaf-backed panel
    /// binding has a sheaf of the same kind, and every sheaf is
    /// referenced by some panel binding.
    pub fn validate(&self) -> Result<(), ToposError> {
        if self.spec_version != SPEC_VERSION {
            return Err(ToposError::UnsupportedVersion {
                found: self.spec_version,
                supported: SPEC_VERSION,
            });
        }
        self.workspace
            .validate()
            .map_err(|errors| ToposError::Workspace(errors.to_string()))?;

        let mut referenced = BTreeSet::new();
        for panel in &self.workspace.panels {
            let Some((id, content_kind)) = sheaf_binding(&panel.content) else {
                continue;
            };
            match self.sheaves.get(id) {
                None => {
                    return Err(ToposError::MissingSheaf {
                        binding: id.to_owned(),
                    })
                }
                Some(sheaf) => {
                    if sheaf.kind() != content_kind {
                        return Err(ToposError::KindMismatch {
                            binding: id.to_owned(),
                            content_kind: content_kind.to_owned(),
                            sheaf_kind: sheaf.kind(),
                        });
                    }
                }
            }
            referenced.insert(id.to_owned());
        }

        for id in self.sheaves.keys() {
            if !referenced.contains(id) {
                return Err(ToposError::UnreferencedSheaf { id: id.clone() });
            }
        }
        self.validate_restrictions_and_edits()
    }

    /// Every restriction names declared sorts; every edit is well-formed and
    /// every table's offered edits exist and sit on an object-scoped table.
    fn validate_restrictions_and_edits(&self) -> Result<(), ToposError> {
        for (id, edit) in &self.edits {
            let invalid = |detail: String| ToposError::InvalidEdit {
                edit: id.clone(),
                detail,
            };
            if edit.sorts.is_empty() {
                return Err(invalid("applies to no sort".to_owned()));
            }
            if let Some(sort) = edit.sorts.iter().find(|sort| !self.stalks.objects.contains_key(*sort)) {
                return Err(invalid(format!("sort '{sort}' is not a regime object sort")));
            }
            if edit.set.is_empty() {
                return Err(invalid("sets no fields".to_owned()));
            }
            if let Some((field, defect)) = edit.set.iter().find_map(|(field, ty)| ty.defect().map(|d| (field, d))) {
                return Err(invalid(format!("field '{field}': {defect}")));
            }
            if edit.grammar.is_empty() {
                return Err(invalid("names no grammar".to_owned()));
            }
        }
        for (binding, sheaf) in &self.sheaves {
            if let Some((sorts, space)) = sheaf.restriction() {
                let declared = match space {
                    SortSpace::Objects => self.object_sort_set(),
                    SortSpace::Morphisms => self.morphism_sort_set(),
                };
                if let Some(sort) = sorts.iter().find(|sort| !declared.contains(*sort)) {
                    return Err(ToposError::RestrictOutsideSorts {
                        binding: binding.clone(),
                        sort: sort.clone(),
                    });
                }
            }
            if let Sheaf::Table(table) = sheaf {
                if let Some(edit) = table.edits.iter().find(|edit| !self.edits.contains_key(*edit)) {
                    return Err(ToposError::UnknownEdit {
                        binding: binding.clone(),
                        edit: edit.clone(),
                    });
                }
                if let (Some(edit), RowScope::Morphisms) = (table.edits.first(), table.scope) {
                    return Err(ToposError::InvalidEdit {
                        edit: edit.clone(),
                        detail: format!("offered on morphism-scoped table '{binding}'"),
                    });
                }
            }
        }
        Ok(())
    }
}

/// Resolution context: the sortings, stalks, and physics a sheaf reads to turn
/// a site into content.
pub(crate) struct Resolver<'a> {
    pub(crate) object_sorting: &'a BTreeMap<String, String>,
    pub(crate) morphism_sorting: &'a BTreeMap<String, String>,
    pub(crate) stalks: &'a Stalks,
    pub(crate) physics: &'a Physics,
}

impl Resolver<'_> {
    /// Regime sort for an object kind.
    pub(crate) fn object_sort(&self, kind: &str) -> Option<&str> {
        self.object_sorting.get(kind).map(String::as_str)
    }

    /// Label for an object sort, falling back to the raw sort.
    pub(crate) fn sort_object_label(&self, sort: &str) -> String {
        self.stalks
            .objects
            .get(sort)
            .map(|stalk| stalk.label.clone())
            .unwrap_or_else(|| sort.to_owned())
    }

    /// Badge-kind string for an object sort, defaulting to `generic`.
    pub(crate) fn sort_object_badge(&self, sort: &str) -> &str {
        self.stalks
            .objects
            .get(sort)
            .map(|stalk| stalk.badge.as_str())
            .unwrap_or("generic")
    }

    /// Regime label for an object kind (through its sort).
    pub(crate) fn object_label(&self, kind: &str) -> String {
        match self.object_sort(kind) {
            Some(sort) => self.sort_object_label(sort),
            None => kind.to_owned(),
        }
    }

    /// Badge-kind string for an object kind (through its sort).
    pub(crate) fn object_badge(&self, kind: &str) -> &str {
        match self.object_sorting.get(kind) {
            Some(sort) => self.sort_object_badge(sort),
            None => "generic",
        }
    }

    /// Repulsion for an object kind, via its sort's physics tuning.
    pub(crate) fn object_repulsion(&self, kind: &str) -> Result<f64, SheafError> {
        let sort = self
            .object_sorting
            .get(kind)
            .ok_or_else(|| SheafError::UnsortedObjectKind(kind.to_owned()))?;
        self.physics
            .object(sort)
            .map(|tuning| tuning.repulsion)
            .ok_or_else(|| SheafError::MissingObjectPhysics(sort.clone()))
    }

    /// Mass for an object kind, via its sort's physics tuning.
    pub(crate) fn object_mass(&self, kind: &str) -> Result<f64, SheafError> {
        let sort = self
            .object_sorting
            .get(kind)
            .ok_or_else(|| SheafError::UnsortedObjectKind(kind.to_owned()))?;
        self.physics
            .object(sort)
            .map(|tuning| tuning.mass)
            .ok_or_else(|| SheafError::MissingObjectPhysics(sort.clone()))
    }

    /// Regime label for a morphism kind; untyped morphisms read as `untyped`.
    pub(crate) fn morphism_label(&self, kind: Option<&str>) -> String {
        match kind {
            Some(kind) => match self.morphism_sorting.get(kind) {
                Some(sort) => self
                    .stalks
                    .morphisms
                    .get(sort)
                    .map(|stalk| stalk.label.clone())
                    .unwrap_or_else(|| sort.clone()),
                None => kind.to_owned(),
            },
            None => "untyped".to_owned(),
        }
    }

    /// Spring weight for a morphism kind, via its sort's physics tuning.
    pub(crate) fn morphism_weight(&self, kind: Option<&str>) -> Result<f64, SheafError> {
        match kind {
            Some(kind) => {
                let sort = self
                    .morphism_sorting
                    .get(kind)
                    .ok_or_else(|| SheafError::UnsortedMorphismKind(kind.to_owned()))?;
                self.physics
                    .morphism(sort)
                    .map(|tuning| tuning.weight)
                    .ok_or_else(|| SheafError::MissingMorphismPhysics(sort.clone()))
            }
            None => Err(SheafError::UntypedMorphism),
        }
    }

    /// Rest length for a morphism kind, via its sort's physics tuning.
    pub(crate) fn morphism_rest_length(&self, kind: Option<&str>) -> Result<f64, SheafError> {
        match kind {
            Some(kind) => {
                let sort = self
                    .morphism_sorting
                    .get(kind)
                    .ok_or_else(|| SheafError::UnsortedMorphismKind(kind.to_owned()))?;
                self.physics
                    .morphism(sort)
                    .map(|tuning| tuning.rest_length)
                    .ok_or_else(|| SheafError::MissingMorphismPhysics(sort.clone()))
            }
            None => Err(SheafError::UntypedMorphism),
        }
    }

    /// The per-object/per-morphism engine parameters over the whole site.
    pub(crate) fn physics_section(&self, site: &Site) -> PhysicsSection {
        let mut objects = Vec::with_capacity(site.objects.len());
        for object in &site.objects {
            let tuning = self
                .object_sorting
                .get(&object.kind)
                .and_then(|sort| self.physics.object(sort));
            let (engine_type, repulsion, mass) = match tuning {
                Some(tuning) => (tuning.engine_type.clone(), tuning.repulsion, tuning.mass),
                None => (String::new(), 0.0, 0.0),
            };
            objects.push((object.id.clone(), engine_type, repulsion, mass));
        }
        let mut morphisms = Vec::with_capacity(site.morphisms.len());
        for morphism in &site.morphisms {
            let tuning = morphism
                .kind
                .as_deref()
                .and_then(|kind| self.morphism_sorting.get(kind))
                .and_then(|sort| self.physics.morphism(sort));
            let (engine_type, weight, rest_length) = match tuning {
                Some(tuning) => (tuning.engine_type.clone(), tuning.weight, tuning.rest_length),
                None => (String::new(), 0.0, 0.0),
            };
            morphisms.push((
                morphism.domain.clone(),
                morphism.codomain.clone(),
                engine_type,
                weight,
                rest_length,
            ));
        }
        PhysicsSection { objects, morphisms }
    }
}

/// The sheaf-backed binding of a panel: `Some((id, content_kind))` for the
/// content kinds sourced from a host binding, `None` otherwise.
fn sheaf_binding(content: &ContentSpec) -> Option<(&str, &'static str)> {
    match content {
        ContentSpec::Text { source, .. } => source.binding_id().map(|id| (id, "text")),
        ContentSpec::Badges { source } => source.binding_id().map(|id| (id, "badges")),
        ContentSpec::Table { source } => source.binding_id().map(|id| (id, "table")),
        ContentSpec::TimeSeries { source, .. } => source.binding_id().map(|id| (id, "time_series")),
        ContentSpec::Gauges { source } => source.binding_id().map(|id| (id, "gauges")),
        ContentSpec::Flamegraph { source } => source.binding_id().map(|id| (id, "flamegraph")),
        ContentSpec::Boxplot { source } => source.binding_id().map(|id| (id, "boxplot")),
        ContentSpec::Meter { source } => source.binding_id().map(|id| (id, "meter")),
        ContentSpec::Status { source } => source.binding_id().map(|id| (id, "status")),
        ContentSpec::Custom { .. } | ContentSpec::Editor { .. } | ContentSpec::Spinner { .. } => None,
    }
}
