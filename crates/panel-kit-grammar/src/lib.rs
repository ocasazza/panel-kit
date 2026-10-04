//! Renderer-neutral grammars and topos sheaves for panel-kit.
//!
//! A **grammar** is a data-import package: it turns input bytes into a
//! renderer-neutral [`Site`]. Two engines share one envelope — [`pest`](crate::package::ParserSpec::Pest)
//! (a runtime `.pest` grammar with a semantic capture map, mirroring
//! jump-cannon's importer) and [`json`](crate::package::ParserSpec::Json)
//! (JSON pointers plus a field map). Different grammars over equivalent input
//! produce equal sites.
//!
//! A **topos** is one UI/UX regime over a site: a [`Topos`] mapping site kinds
//! to regime sorts, pairing a workspace with [`Stalks`] (what each sort means)
//! and [`Sheaf`]s (binding id -> declarative query -> content model). Physics
//! tuning is not authored per regime; it is the inverse image `p_R*` of the
//! physics [`BaseTopos`] carried by a [`GeometricMorphism`] (see [`Physics`]),
//! so the same site under two regimes yields different panels but one physics
//! section.
//!
//! Everything here is sync, pure, and `wasm32`-clean: no filesystem, network,
//! or threads.

#![forbid(unsafe_code)]

pub mod error;
pub mod morphism;
pub mod site;
mod json_engine;
pub mod package;
mod pest_engine;
pub mod sheaf;
pub mod topos;

pub use error::{GrammarError, MorphismError, SheafError, ToposError};
pub use morphism::{
    BaseStalks, BaseTopos, EngineMorphismStalk, EngineObjectStalk, GeometricMorphism,
    MorphismTuning, ObjectTuning, Physics,
};
pub use package::{
    CaptureRules, CompiledGrammar, GrammarPackage, JsonEdgeMap, JsonNodeMap, JsonSpec, Limits,
    Metadata, ParserSpec, PestSpec, FORMAT_VERSION, HARD_LIMITS,
};
pub use site::{Morphism, Object, Site};
pub use sheaf::{
    assembly, Agg, AssemblySheaf, BadgeGroup, BadgesSheaf, BoxplotSheaf, CellAlign, CellSpec,
    ColWidth, ColumnSpec, ComponentInvariants, FlamegraphSheaf, GaugesSheaf, GlobalSections,
    GroupScope, MeterSheaf, PhysicsScope, PhysicsSheaf, RowScope, Section, Sheaf, Stage,
    StatusSheaf, TableSheaf, TextSheaf, TimeSeriesSheaf,
};
pub use topos::{MorphismStalk, ObjectStalk, PhysicsSection, Stages, Stalks, Topos, SPEC_VERSION};

#[cfg(test)]
mod tests;
