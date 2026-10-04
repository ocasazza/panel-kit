//! Hand-written error types for grammar compilation, parsing, topos
//! decoding/validation, and sheaf. No `thiserror`: each variant renders
//! itself through `Display` and the enums implement `std::error::Error`.

use std::fmt;

/// Grammar package decode, compile, and parse failures.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GrammarError {
    /// `serde` rejected the package JSON (includes unknown keys).
    Json(String),
    /// `format_version` was not the supported value.
    UnsupportedVersion {
        /// Version found in the package.
        found: u32,
        /// Version this crate supports.
        supported: u32,
    },
    /// A declared limit was zero and cannot bound anything.
    ZeroLimit {
        /// Limit name.
        name: &'static str,
    },
    /// The inline grammar exceeded the effective byte limit.
    GrammarTooLarge {
        /// Grammar byte length.
        actual: usize,
        /// Effective maximum.
        max: usize,
    },
    /// The input exceeded the effective byte limit.
    InputTooLarge {
        /// Input byte length.
        actual: usize,
        /// Effective maximum.
        max: usize,
    },
    /// `pest_meta` rejected the inline grammar.
    InvalidGrammar(String),
    /// A capture role named a rule absent from the grammar.
    MissingRule {
        /// Capture role (e.g. `node`, `id`).
        role: &'static str,
        /// Rule name that was not found.
        rule: String,
    },
    /// A capture role named a silent rule, which yields no capture.
    SilentRule {
        /// Capture role.
        role: &'static str,
        /// Rule name.
        rule: String,
    },
    /// One rule was bound to two capture roles.
    AmbiguousRule {
        /// Rule name.
        rule: String,
        /// First role it was assigned to.
        first_role: &'static str,
        /// Second role it was assigned to.
        second_role: &'static str,
    },
    /// The parser did not accept the input under the root rule.
    Parse {
        /// Root rule name.
        rule: String,
        /// Parser message.
        message: String,
    },
    /// The root rule matched only a prefix of the input.
    PartialParse {
        /// Root rule name.
        rule: String,
        /// Bytes matched.
        matched: usize,
        /// Total input bytes.
        total: usize,
    },
    /// A record was structurally invalid.
    InvalidRecord {
        /// Record kind (`node`, `edge`, or `property`).
        record: &'static str,
        /// Human-readable detail.
        detail: String,
    },
    /// Two nodes shared one id.
    DuplicateNodeId(String),
    /// Record count exceeded a declared limit.
    RecordLimit {
        /// Record kind (`nodes` or `edges`).
        kind: &'static str,
        /// Count reached.
        actual: usize,
        /// Declared maximum.
        max: usize,
    },
    /// An edge referenced an endpoint that no node declared.
    DanglingEdge {
        /// Source id.
        source: String,
        /// Target id.
        target: String,
        /// Which endpoint(s) were missing.
        missing: &'static str,
    },
    /// A JSON pointer did not resolve, or resolved to the wrong JSON type.
    JsonShape(String),
    /// `emit` was called on a package that declares no emitter.
    NoEmitter,
    /// The emitter is for a different engine than the parser.
    EmitterEngine {
        /// Parser engine.
        parser: &'static str,
        /// Emitter engine.
        emitter: &'static str,
    },
    /// A pest emitter template names an unknown hole or is malformed.
    InvalidTemplate(String),
    /// Emitted output would not parse back to the emitted objects.
    EmitterLosesData(String),
    /// A support id names no object of the site being emitted.
    UnknownSupportObject(String),
}

impl fmt::Display for GrammarError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(message) => write!(f, "invalid grammar package JSON: {message}"),
            Self::UnsupportedVersion { found, supported } => write!(
                f,
                "unsupported grammar format_version {found}; this crate supports {supported}"
            ),
            Self::ZeroLimit { name } => write!(f, "limit '{name}' must be greater than zero"),
            Self::GrammarTooLarge { actual, max } => {
                write!(f, "inline grammar is {actual} bytes; limit is {max} bytes")
            }
            Self::InputTooLarge { actual, max } => {
                write!(f, "input is {actual} bytes; limit is {max} bytes")
            }
            Self::InvalidGrammar(message) => write!(f, "invalid pest grammar: {message}"),
            Self::MissingRule { role, rule } => {
                write!(f, "capture rule '{rule}' for role {role} does not exist in the grammar")
            }
            Self::SilentRule { role, rule } => {
                write!(f, "capture rule '{rule}' for role {role} is silent and produces no capture")
            }
            Self::AmbiguousRule { rule, first_role, second_role } => write!(
                f,
                "capture rule '{rule}' is assigned to both {first_role} and {second_role}"
            ),
            Self::Parse { rule, message } => {
                write!(f, "input did not match root rule '{rule}': {message}")
            }
            Self::PartialParse { rule, matched, total } => write!(
                f,
                "root rule '{rule}' matched {matched} of {total} bytes; the root must consume all input"
            ),
            Self::InvalidRecord { record, detail } => {
                write!(f, "invalid {record} record: {detail}")
            }
            Self::DuplicateNodeId(id) => write!(f, "duplicate node id '{id}'"),
            Self::RecordLimit { kind, actual, max } => {
                write!(f, "matched {actual} {kind}; limit is {max}")
            }
            Self::DanglingEdge { source, target, missing } => {
                write!(f, "edge '{source}' -> '{target}' has missing {missing}")
            }
            Self::JsonShape(message) => write!(f, "json engine: {message}"),
            Self::NoEmitter => write!(f, "grammar package declares no emitter"),
            Self::EmitterEngine { parser, emitter } => {
                write!(f, "emitter engine '{emitter}' does not match parser engine '{parser}'")
            }
            Self::InvalidTemplate(message) => write!(f, "invalid emitter template: {message}"),
            Self::EmitterLosesData(message) => {
                write!(f, "emitted output does not parse back to the emitted objects: {message}")
            }
            Self::UnknownSupportObject(id) => write!(f, "support object '{id}' is not in the site"),
        }
    }
}

impl std::error::Error for GrammarError {}

/// Topos decode and validation failures.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ToposError {
    /// `serde` rejected the topos JSON (includes unknown keys).
    Json(String),
    /// `spec_version` was not the supported value.
    UnsupportedVersion {
        /// Version found in the spec.
        found: u32,
        /// Version this crate supports.
        supported: u32,
    },
    /// The embedded workspace failed its own validation.
    Workspace(String),
    /// A panel binding had no sheaf of the same content kind.
    MissingSheaf {
        /// Binding id with no sheaf.
        binding: String,
    },
    /// A sheaf was never referenced by a panel binding.
    UnreferencedSheaf {
        /// Sheaf id.
        id: String,
    },
    /// A sheaf produced a different content kind than its panel.
    KindMismatch {
        /// Binding id.
        binding: String,
        /// The panel's content kind.
        content_kind: String,
        /// The sheaf's output kind.
        sheaf_kind: &'static str,
    },
    /// A sheaf restricts to a sort the regime does not declare.
    RestrictOutsideSorts {
        /// Sheaf binding id.
        binding: String,
        /// The undeclared sort.
        sort: String,
    },
    /// A section edit is malformed.
    InvalidEdit {
        /// Edit id.
        edit: String,
        /// What is wrong.
        detail: String,
    },
    /// A sheaf references an edit the topos does not declare.
    UnknownEdit {
        /// Sheaf binding id.
        binding: String,
        /// The undeclared edit id.
        edit: String,
    },
}

impl fmt::Display for ToposError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(message) => write!(f, "invalid topos JSON: {message}"),
            Self::UnsupportedVersion { found, supported } => write!(
                f,
                "unsupported topos spec_version {found}; this crate supports {supported}"
            ),
            Self::Workspace(message) => write!(f, "invalid workspace: {message}"),
            Self::MissingSheaf { binding } => {
                write!(f, "panel binding '{binding}' has no sheaf")
            }
            Self::UnreferencedSheaf { id } => {
                write!(f, "sheaf '{id}' is not referenced by any panel binding")
            }
            Self::KindMismatch { binding, content_kind, sheaf_kind } => write!(
                f,
                "binding '{binding}' is authored as content kind '{content_kind}' but its sheaf produces '{sheaf_kind}'"
            ),
            Self::RestrictOutsideSorts { binding, sort } => {
                write!(f, "sheaf '{binding}' restricts to undeclared sort '{sort}'")
            }
            Self::InvalidEdit { edit, detail } => write!(f, "edit '{edit}': {detail}"),
            Self::UnknownEdit { binding, edit } => {
                write!(f, "sheaf '{binding}' references undeclared edit '{edit}'")
            }
        }
    }
}

impl std::error::Error for ToposError {}

/// Sheaf evaluation failures.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SheafError {
    /// A value expression was not recognized.
    BadExpression(String),
    /// An object property required for a numeric channel was missing.
    MissingField {
        /// Object id.
        object: String,
        /// Field name.
        field: String,
    },
    /// A value expression that must be numeric was not.
    NotNumeric {
        /// The expression.
        expr: String,
        /// The non-numeric value produced.
        value: String,
    },
    /// A present object kind is not mapped by the regime's object sorting.
    UnsortedObjectKind(String),
    /// A present morphism kind is not mapped by the regime's morphism sorting.
    UnsortedMorphismKind(String),
    /// An object regime sort has no stalk in this topos.
    MissingObjectStalk(String),
    /// A morphism regime sort has no stalk in this topos.
    MissingMorphismStalk(String),
    /// An object regime sort has no physics tuning (engine type/repulsion/mass).
    MissingObjectPhysics(String),
    /// A morphism regime sort has no physics tuning (weight/rest length).
    MissingMorphismPhysics(String),
    /// A `weight` expression was evaluated on an untyped morphism.
    UntypedMorphism,
    /// An stalks badge string did not name a known badge kind.
    BadBadge(String),
    /// A flamegraph root id did not name any object.
    MissingRoot(String),
    /// A table or status cell was used in a scope that cannot supply it.
    BadCell {
        /// Scope the cell was evaluated in.
        scope: &'static str,
        /// Cell detail.
        detail: &'static str,
    },
}

impl fmt::Display for SheafError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadExpression(expr) => write!(f, "unrecognized value expression '{expr}'"),
            Self::MissingField { object, field } => {
                write!(f, "object '{object}' has no field '{field}'")
            }
            Self::NotNumeric { expr, value } => {
                write!(f, "expression '{expr}' is not numeric: '{value}'")
            }
            Self::UnsortedObjectKind(kind) => {
                write!(f, "object kind '{kind}' is not sorted by this regime")
            }
            Self::UnsortedMorphismKind(kind) => {
                write!(f, "morphism kind '{kind}' is not sorted by this regime")
            }
            Self::MissingObjectStalk(sort) => {
                write!(f, "object sort '{sort}' has no stalk")
            }
            Self::MissingMorphismStalk(sort) => {
                write!(f, "morphism sort '{sort}' has no stalk")
            }
            Self::MissingObjectPhysics(sort) => {
                write!(f, "object sort '{sort}' has no physics tuning")
            }
            Self::MissingMorphismPhysics(sort) => {
                write!(f, "morphism sort '{sort}' has no physics tuning")
            }
            Self::UntypedMorphism => write!(f, "weight is undefined on an untyped morphism"),
            Self::BadBadge(badge) => write!(f, "unknown badge kind '{badge}'"),
            Self::MissingRoot(id) => write!(f, "flamegraph root '{id}' names no object"),
            Self::BadCell { scope, detail } => {
                write!(f, "cell {detail} is not available in {scope} scope")
            }
        }
    }
}

impl std::error::Error for SheafError {}

/// Geometric-morphism decode, validation, composition, and physics failures.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MorphismError {
    /// `serde` rejected the morphism JSON (includes unknown keys).
    Json(String),
    /// A domain sort has no image under the sort map (φ not total).
    NotTotal {
        /// `object` or `morphism`.
        part: &'static str,
        /// The unmapped domain sort.
        sort: String,
    },
    /// A sort maps to an image outside the codomain's sorts.
    OutsideCodomain {
        /// `object` or `morphism`.
        part: &'static str,
        /// The domain sort.
        sort: String,
        /// The image sort absent from the codomain.
        image: String,
    },
    /// The morphism does not commute with the sortings / the base.
    NotCommuting {
        /// `object` or `morphism`.
        part: &'static str,
        /// The site kind or domain sort where commuting fails.
        at: String,
    },
    /// `compose` was called on morphisms whose codomain/domain disagree.
    ComposeMismatch {
        /// The left morphism's codomain id.
        codomain: String,
        /// The right morphism's domain id.
        domain: String,
    },
    /// `compose` hit a middle sort the second morphism does not map.
    ComposeUndefined {
        /// `object` or `morphism`.
        part: &'static str,
        /// The middle sort with no second image.
        sort: String,
    },
    /// A sort mapped to a base engine sort absent from the base topos.
    UnknownBaseSort {
        /// `object` or `morphism`.
        part: &'static str,
        /// The missing base engine sort.
        sort: String,
    },
    /// A physics morphism's domain id is not the regime topos id.
    DomainMismatch {
        /// The regime topos id.
        expected: String,
        /// The morphism's domain id.
        found: String,
    },
    /// A physics morphism's codomain id is not the base topos id.
    CodomainMismatch {
        /// The base topos id.
        expected: String,
        /// The morphism's codomain id.
        found: String,
    },
}

impl fmt::Display for MorphismError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(message) => write!(f, "invalid morphism JSON: {message}"),
            Self::NotTotal { part, sort } => {
                write!(f, "{part} sort map is not total: '{sort}' has no image")
            }
            Self::OutsideCodomain { part, sort, image } => write!(
                f,
                "{part} sort '{sort}' maps to '{image}', which is not a codomain sort"
            ),
            Self::NotCommuting { part, at } => {
                write!(f, "{part} sort map does not commute at '{at}'")
            }
            Self::ComposeMismatch { codomain, domain } => write!(
                f,
                "cannot compose: left codomain '{codomain}' != right domain '{domain}'"
            ),
            Self::ComposeUndefined { part, sort } => {
                write!(f, "cannot compose: {part} sort '{sort}' has no second image")
            }
            Self::UnknownBaseSort { part, sort } => {
                write!(f, "{part} engine sort '{sort}' is absent from the base topos")
            }
            Self::DomainMismatch { expected, found } => write!(
                f,
                "physics morphism domain '{found}' is not the regime topos '{expected}'"
            ),
            Self::CodomainMismatch { expected, found } => write!(
                f,
                "physics morphism codomain '{found}' is not the base topos '{expected}'"
            ),
        }
    }
}

impl std::error::Error for MorphismError {}

/// Site construction failures (gluing).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SiteError {
    /// Two objects identified by the gluing differ.
    GlueDisagrees {
        /// Object id.
        id: String,
    },
    /// Two objects share an id outside the gluing kind.
    IdCollision {
        /// Object id.
        id: String,
    },
}

impl fmt::Display for SiteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GlueDisagrees { id } => write!(f, "glued object '{id}' differs between the two sites"),
            Self::IdCollision { id } => write!(f, "object id '{id}' occurs in both sites outside the gluing kind"),
        }
    }
}

impl std::error::Error for SiteError {}

/// Section edit failures: applying an edit, or building its write intent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditError {
    /// The topos declares no edit with this id.
    UnknownEdit(String),
    /// The site has no object with this id.
    UnknownObject(String),
    /// The object's regime sort is not one the edit applies to.
    NotApplicable {
        /// Edit id.
        edit: String,
        /// Object id.
        object: String,
        /// The object's regime sort.
        sort: String,
    },
    /// The input sets a field the edit does not declare.
    UnknownField {
        /// Edit id.
        edit: String,
        /// Field name.
        field: String,
    },
    /// A value does not satisfy its field type.
    InvalidValue {
        /// Field name.
        field: String,
        /// What is wrong.
        detail: String,
    },
    /// The input sets no fields.
    EmptyEdit(String),
    /// The edit is bound to a different grammar than the one given.
    GrammarMismatch {
        /// Edit id.
        edit: String,
        /// Grammar the edit names.
        expected: String,
        /// Grammar supplied.
        actual: String,
    },
    /// The grammar could not emit the revised objects.
    Emit(GrammarError),
}

impl fmt::Display for EditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownEdit(edit) => write!(f, "no edit '{edit}'"),
            Self::UnknownObject(id) => write!(f, "no object '{id}'"),
            Self::NotApplicable { edit, object, sort } => {
                write!(f, "edit '{edit}' does not apply to object '{object}' of sort '{sort}'")
            }
            Self::UnknownField { edit, field } => write!(f, "edit '{edit}' declares no field '{field}'"),
            Self::InvalidValue { field, detail } => write!(f, "field '{field}': {detail}"),
            Self::EmptyEdit(edit) => write!(f, "edit '{edit}' was given no fields to set"),
            Self::GrammarMismatch { edit, expected, actual } => {
                write!(f, "edit '{edit}' is bound to grammar '{expected}', not '{actual}'")
            }
            Self::Emit(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for EditError {}
