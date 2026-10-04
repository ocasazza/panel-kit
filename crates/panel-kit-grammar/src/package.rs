//! Grammar packages: the declarative, JSON-authored import description shared
//! by both engines, plus the compiled handle that parses input into a [`Site`].
//!
//! A package is engine-tagged (`pest` or `json`). The envelope (metadata,
//! limits) is identical across engines; the `parser` body is engine-specific.
//! Limits are clamped to [`HARD_LIMITS`] at compile time and enforced while
//! mapping — records are rejected, never truncated.

use serde::{Deserialize, Serialize};

use crate::error::GrammarError;
use crate::site::Site;
use crate::{json_engine, pest_engine};

/// Supported package format version.
pub const FORMAT_VERSION: u32 = 1;

/// Ceiling applied to every declared limit. A package may request less; a
/// larger request is clamped down to these values.
pub const HARD_LIMITS: Limits = Limits {
    grammar_bytes: 64 * 1024,
    input_bytes: 256 * 1024 * 1024,
    nodes: 1_000_000,
    edges: 500_000,
};

/// A complete, JSON-authored grammar package.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GrammarPackage {
    /// Package format version; must equal [`FORMAT_VERSION`].
    pub format_version: u32,
    /// Identity and release metadata.
    pub metadata: Metadata,
    /// Declared storage limits.
    pub limits: Limits,
    /// Engine-specific parser configuration.
    pub parser: ParserSpec,
}

/// Identity and release metadata.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Metadata {
    /// Stable package id, also the compiled grammar id.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Package version string.
    pub version: String,
    /// One-line description.
    pub description: String,
}

/// Storage limits declared by a package. Clamped to [`HARD_LIMITS`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Limits {
    /// Maximum inline grammar byte length (pest engine).
    pub grammar_bytes: usize,
    /// Maximum input byte length.
    pub input_bytes: usize,
    /// Maximum node records.
    pub nodes: usize,
    /// Maximum edge records.
    pub edges: usize,
}

impl Limits {
    /// Reject zero limits and clamp each to [`HARD_LIMITS`].
    fn effective(self) -> Result<Limits, GrammarError> {
        for (name, value) in [
            ("grammar_bytes", self.grammar_bytes),
            ("input_bytes", self.input_bytes),
            ("nodes", self.nodes),
            ("edges", self.edges),
        ] {
            if value == 0 {
                return Err(GrammarError::ZeroLimit { name });
            }
        }
        Ok(Limits {
            grammar_bytes: self.grammar_bytes.min(HARD_LIMITS.grammar_bytes),
            input_bytes: self.input_bytes.min(HARD_LIMITS.input_bytes),
            nodes: self.nodes.min(HARD_LIMITS.nodes),
            edges: self.edges.min(HARD_LIMITS.edges),
        })
    }
}

/// Engine-tagged parser configuration.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "engine", rename_all = "snake_case")]
pub enum ParserSpec {
    /// Runtime pest grammar with a semantic capture map.
    Pest(PestSpec),
    /// Declarative JSON pointer + field map.
    Json(JsonSpec),
}

/// Pest engine configuration.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PestSpec {
    /// Rule invoked for the complete input.
    pub root_rule: String,
    /// Inline `.pest` grammar source.
    pub grammar: String,
    /// Semantic capture-rule bindings.
    pub captures: CaptureRules,
}

/// Capture-rule bindings mapping pest rule names to canonical site roles.
/// Mirrors jump-cannon's `crates/importer/src/pest.rs` capture contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaptureRules {
    /// Rule matching a whole node record.
    pub node: String,
    /// Rule capturing a node's id.
    pub id: String,
    /// Rule capturing a node's title.
    pub title: String,
    /// Rule capturing a node's kind.
    pub kind: String,
    /// Rule capturing one tag.
    pub tag: String,
    /// Rule matching one property.
    pub property: String,
    /// Rule capturing a property key.
    pub key: String,
    /// Rule capturing a property value.
    pub value: String,
    /// Rule matching a whole edge record.
    pub edge: String,
    /// Rule capturing an edge source.
    pub source: String,
    /// Rule capturing an edge target.
    pub target: String,
    /// Optional rule capturing an edge kind; unbound leaves edges untyped.
    #[serde(default)]
    pub edge_kind: Option<String>,
}

impl CaptureRules {
    /// The eleven required roles as `(role, rule)` pairs.
    pub(crate) fn required(&self) -> [(&'static str, &str); 11] {
        [
            ("node", &self.node),
            ("id", &self.id),
            ("title", &self.title),
            ("kind", &self.kind),
            ("tag", &self.tag),
            ("property", &self.property),
            ("key", &self.key),
            ("value", &self.value),
            ("edge", &self.edge),
            ("source", &self.source),
            ("target", &self.target),
        ]
    }
}

/// JSON engine configuration.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JsonSpec {
    /// JSON pointer to the nodes array.
    pub nodes: String,
    /// JSON pointer to the morphisms array; absent means a records-only site.
    #[serde(default)]
    pub edges: Option<String>,
    /// Object member mapping.
    pub node: JsonNodeMap,
    /// Morphism member mapping; required only when `edges` is present.
    #[serde(default)]
    pub edge: Option<JsonEdgeMap>,
}

/// Member names mapping a JSON node object onto a [`Object`](crate::site::Object).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JsonNodeMap {
    /// Member holding the id.
    pub id: String,
    /// Member holding the title.
    pub title: String,
    /// Member holding the kind.
    pub kind: String,
    /// Member holding the tags array; `None` leaves tags empty.
    #[serde(default)]
    pub tags: Option<String>,
    /// Members projected into node fields, in output-independent order.
    pub fields: Vec<String>,
}

/// Member names mapping a JSON edge object onto an [`Morphism`](crate::site::Morphism).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JsonEdgeMap {
    /// Member holding the source id.
    pub source: String,
    /// Member holding the target id.
    pub target: String,
    /// Member holding the edge kind; `None` leaves edges untyped.
    #[serde(default)]
    pub kind: Option<String>,
}

impl GrammarPackage {
    /// Decode a package from JSON, rejecting unknown keys.
    pub fn from_json_str(text: &str) -> Result<Self, GrammarError> {
        serde_json::from_str(text).map_err(|error| GrammarError::Json(error.to_string()))
    }

    /// Validate the envelope and build a parser ready to map input.
    pub fn compile(&self) -> Result<CompiledGrammar, GrammarError> {
        if self.format_version != FORMAT_VERSION {
            return Err(GrammarError::UnsupportedVersion {
                found: self.format_version,
                supported: FORMAT_VERSION,
            });
        }
        let limits = self.limits.effective()?;
        let engine = match &self.parser {
            ParserSpec::Pest(spec) => {
                CompiledEngine::Pest(pest_engine::compile(spec, limits.grammar_bytes)?)
            }
            ParserSpec::Json(spec) => CompiledEngine::Json(json_engine::compile(spec)),
        };
        Ok(CompiledGrammar {
            id: self.metadata.id.clone(),
            limits,
            engine,
        })
    }
}

/// A compiled grammar bound to its effective limits, ready to parse input.
pub struct CompiledGrammar {
    id: String,
    limits: Limits,
    engine: CompiledEngine,
}

enum CompiledEngine {
    Pest(pest_engine::CompiledPest),
    Json(json_engine::CompiledJson),
}

impl CompiledGrammar {
    /// The compiled package id.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Parse one UTF-8 input into a [`Site`], enforcing limits.
    pub fn parse(&self, input: &str) -> Result<Site, GrammarError> {
        if input.len() > self.limits.input_bytes {
            return Err(GrammarError::InputTooLarge {
                actual: input.len(),
                max: self.limits.input_bytes,
            });
        }
        match &self.engine {
            CompiledEngine::Pest(engine) => engine.parse(input, &self.limits),
            CompiledEngine::Json(engine) => engine.parse(input, &self.limits),
        }
    }
}
