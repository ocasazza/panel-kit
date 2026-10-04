//! The pest engine: a runtime `.pest` grammar driven by a semantic capture
//! map. Mirrors jump-cannon's `crates/importer/src/pest.rs` capture contract —
//! node/id/title/kind/tag/property/key/value/edge/source/target and the
//! optional edge_kind — but drops namespacing (ids stay raw) and treats a
//! dangling edge endpoint as a hard error.

use std::collections::{HashMap, HashSet};

use pest::iterators::Pair;
use pest_meta::ast::RuleType;
use pest_vm::Vm;

use crate::error::GrammarError;
use crate::site::{Morphism, Site, Object};
use crate::package::{CaptureRules, Limits, PestSpec};

/// A pest grammar compiled into an executable VM plus its capture map.
pub struct CompiledPest {
    vm: Vm,
    root_rule: String,
    captures: CaptureRules,
}

/// Compile and validate a pest grammar: optimize it, check the root and
/// capture rules exist, are non-silent, and are each bound to a distinct rule.
pub fn compile(spec: &PestSpec, grammar_bytes: usize) -> Result<CompiledPest, GrammarError> {
    if spec.grammar.len() > grammar_bytes {
        return Err(GrammarError::GrammarTooLarge {
            actual: spec.grammar.len(),
            max: grammar_bytes,
        });
    }
    let (_, optimized) = pest_meta::parse_and_optimize(&spec.grammar)
        .map_err(|errors| GrammarError::InvalidGrammar(join_errors(&errors)))?;
    validate_bindings(spec, &optimized)?;
    Ok(CompiledPest {
        vm: Vm::new(optimized),
        root_rule: spec.root_rule.clone(),
        captures: spec.captures.clone(),
    })
}

fn join_errors<E: std::fmt::Display>(errors: &[E]) -> String {
    errors
        .iter()
        .map(|error| error.to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

fn validate_bindings(spec: &PestSpec, optimized: &[pest_meta::optimizer::OptimizedRule]) -> Result<(), GrammarError> {
    let mut rules: HashMap<&str, RuleType> = HashMap::new();
    for rule in optimized {
        rules.insert(rule.name.as_str(), rule.ty);
    }
    bound_rule("root", &spec.root_rule, &rules)?;

    let mut assigned: HashMap<&str, &'static str> = HashMap::new();
    let optional = spec
        .captures
        .edge_kind
        .iter()
        .map(|rule| ("edge_kind", rule.as_str()));
    for (role, name) in spec.captures.required().into_iter().chain(optional) {
        bound_rule(role, name, &rules)?;
        if let Some(first_role) = assigned.insert(name, role) {
            return Err(GrammarError::AmbiguousRule {
                rule: name.to_owned(),
                first_role,
                second_role: role,
            });
        }
    }
    Ok(())
}

fn bound_rule(role: &'static str, name: &str, rules: &HashMap<&str, RuleType>) -> Result<(), GrammarError> {
    let rule_type = rules.get(name).ok_or_else(|| GrammarError::MissingRule {
        role,
        rule: name.to_owned(),
    })?;
    if *rule_type == RuleType::Silent {
        return Err(GrammarError::SilentRule {
            role,
            rule: name.to_owned(),
        });
    }
    Ok(())
}

impl CompiledPest {
    /// Parse input under the root rule and map its captures to a site.
    pub fn parse(&self, input: &str, limits: &Limits) -> Result<Site, GrammarError> {
        let root_rule = self.root_rule.as_str();
        let mut pairs = self.vm.parse(root_rule, input).map_err(|error| GrammarError::Parse {
            rule: root_rule.to_owned(),
            message: error.to_string(),
        })?;
        let root = pairs.next().ok_or_else(|| GrammarError::Parse {
            rule: root_rule.to_owned(),
            message: "parser returned no root capture".to_owned(),
        })?;
        if root.as_rule() != root_rule {
            return Err(GrammarError::Parse {
                rule: root_rule.to_owned(),
                message: format!("parser returned unexpected top-level rule '{}'", root.as_rule()),
            });
        }
        let span = root.as_span();
        if span.start() != 0 || span.end() != input.len() {
            return Err(GrammarError::PartialParse {
                rule: root_rule.to_owned(),
                matched: span.end().saturating_sub(span.start()),
                total: input.len(),
            });
        }

        let mut mapped = Mapped::default();
        self.collect(root, &mut mapped, limits)?;

        for edge in &mapped.edges {
            let source = mapped.ids.contains(&edge.domain);
            let target = mapped.ids.contains(&edge.codomain);
            if !source || !target {
                let missing = match (source, target) {
                    (false, false) => "source and target",
                    (false, true) => "source",
                    (true, false) => "target",
                    (true, true) => unreachable!(),
                };
                return Err(GrammarError::DanglingEdge {
                    source: edge.domain.clone(),
                    target: edge.codomain.clone(),
                    missing,
                });
            }
        }

        Ok(Site {
            objects: mapped.nodes,
            morphisms: mapped.edges,
        })
    }

    fn collect<'a>(&self, pair: Pair<'a, &'a str>, mapped: &mut Mapped, limits: &Limits) -> Result<(), GrammarError> {
        let rule = pair.as_rule();
        let captures = &self.captures;

        if rule == captures.node {
            let count = mapped.nodes.len() + 1;
            if count > limits.nodes {
                return Err(GrammarError::RecordLimit {
                    kind: "nodes",
                    actual: count,
                    max: limits.nodes,
                });
            }
            let node = map_node(pair, captures)?;
            if !mapped.ids.insert(node.id.clone()) {
                return Err(GrammarError::DuplicateNodeId(node.id));
            }
            mapped.nodes.push(node);
            return Ok(());
        }

        if rule == captures.edge {
            let count = mapped.edges.len() + 1;
            if count > limits.edges {
                return Err(GrammarError::RecordLimit {
                    kind: "edges",
                    actual: count,
                    max: limits.edges,
                });
            }
            mapped.edges.push(map_edge(pair, captures)?);
            return Ok(());
        }

        for child in pair.into_inner() {
            self.collect(child, mapped, limits)?;
        }
        Ok(())
    }
}

#[derive(Default)]
struct Mapped {
    nodes: Vec<Object>,
    edges: Vec<Morphism>,
    ids: HashSet<String>,
}

fn map_node<'a>(pair: Pair<'a, &'a str>, captures: &CaptureRules) -> Result<Object, GrammarError> {
    let mut id = None;
    let mut title = None;
    let mut kind = None;
    let mut tags = Vec::new();
    let mut fields = std::collections::BTreeMap::new();
    for child in pair.into_inner() {
        collect_node_fields(child, captures, &mut id, &mut title, &mut kind, &mut tags, &mut fields)?;
    }
    let id = id.ok_or_else(|| invalid("node", "missing id capture"))?;
    if id.is_empty() {
        return Err(invalid("node", "id capture is empty"));
    }
    let title = title.unwrap_or_else(|| id.clone());
    Ok(Object {
        id,
        title,
        kind: kind.unwrap_or_default(),
        tags,
        fields,
    })
}

#[allow(clippy::too_many_arguments)]
fn collect_node_fields<'a>(
    pair: Pair<'a, &'a str>,
    captures: &CaptureRules,
    id: &mut Option<String>,
    title: &mut Option<String>,
    kind: &mut Option<String>,
    tags: &mut Vec<String>,
    fields: &mut std::collections::BTreeMap<String, String>,
) -> Result<(), GrammarError> {
    let rule = pair.as_rule();
    if rule == captures.id {
        return set_scalar(id, pair.as_str(), "node", "id");
    }
    if rule == captures.title {
        return set_scalar(title, pair.as_str(), "node", "title");
    }
    if rule == captures.kind {
        return set_scalar(kind, pair.as_str(), "node", "kind");
    }
    if rule == captures.tag {
        if pair.as_str().is_empty() {
            return Err(invalid("node", "tag capture is empty"));
        }
        tags.push(pair.as_str().to_owned());
        return Ok(());
    }
    if rule == captures.property {
        let (key, value) = map_property(pair, captures)?;
        if fields.insert(key.clone(), value).is_some() {
            return Err(invalid("node", format!("duplicate property key '{key}'")));
        }
        return Ok(());
    }
    if rule == captures.node || rule == captures.edge {
        return Err(invalid("node", format!("nested site record '{rule}' is not allowed")));
    }
    for child in pair.into_inner() {
        collect_node_fields(child, captures, id, title, kind, tags, fields)?;
    }
    Ok(())
}

fn map_property<'a>(pair: Pair<'a, &'a str>, captures: &CaptureRules) -> Result<(String, String), GrammarError> {
    let mut key = None;
    let mut value = None;
    collect_property_fields(pair, captures, &mut key, &mut value)?;
    let key = key.ok_or_else(|| invalid("property", "missing key capture"))?;
    let value = value.ok_or_else(|| invalid("property", "missing value capture"))?;
    if key.is_empty() {
        return Err(invalid("property", "key capture is empty"));
    }
    Ok((key, value))
}

fn collect_property_fields<'a>(
    pair: Pair<'a, &'a str>,
    captures: &CaptureRules,
    key: &mut Option<String>,
    value: &mut Option<String>,
) -> Result<(), GrammarError> {
    let rule = pair.as_rule();
    if rule == captures.key {
        return set_scalar(key, pair.as_str(), "property", "key");
    }
    if rule == captures.value {
        return set_scalar(value, pair.as_str(), "property", "value");
    }
    for child in pair.into_inner() {
        collect_property_fields(child, captures, key, value)?;
    }
    Ok(())
}

fn map_edge<'a>(pair: Pair<'a, &'a str>, captures: &CaptureRules) -> Result<Morphism, GrammarError> {
    let mut source = None;
    let mut target = None;
    let mut kind = None;
    for child in pair.into_inner() {
        collect_edge_fields(child, captures, &mut source, &mut target, &mut kind)?;
    }
    let source = source.ok_or_else(|| invalid("edge", "missing source capture"))?;
    let target = target.ok_or_else(|| invalid("edge", "missing target capture"))?;
    if source.is_empty() || target.is_empty() {
        return Err(invalid("edge", "source and target captures must be non-empty"));
    }
    if let Some(edge_kind) = &kind {
        if edge_kind.is_empty() {
            return Err(invalid("edge", "edge_kind capture is empty"));
        }
    }
    Ok(Morphism { domain: source, codomain: target, kind })
}

fn collect_edge_fields<'a>(
    pair: Pair<'a, &'a str>,
    captures: &CaptureRules,
    source: &mut Option<String>,
    target: &mut Option<String>,
    kind: &mut Option<String>,
) -> Result<(), GrammarError> {
    let rule = pair.as_rule();
    if rule == captures.source {
        return set_scalar(source, pair.as_str(), "edge", "source");
    }
    if rule == captures.target {
        return set_scalar(target, pair.as_str(), "edge", "target");
    }
    if let Some(edge_kind_rule) = &captures.edge_kind {
        if rule == edge_kind_rule.as_str() {
            return set_scalar(kind, pair.as_str(), "edge", "edge_kind");
        }
    }
    if rule == captures.node || rule == captures.edge {
        return Err(invalid("edge", format!("nested site record '{rule}' is not allowed")));
    }
    for child in pair.into_inner() {
        collect_edge_fields(child, captures, source, target, kind)?;
    }
    Ok(())
}

fn set_scalar(slot: &mut Option<String>, value: &str, record: &'static str, role: &'static str) -> Result<(), GrammarError> {
    if slot.replace(value.to_owned()).is_some() {
        return Err(invalid(record, format!("multiple {role} captures")));
    }
    Ok(())
}

fn invalid(record: &'static str, detail: impl Into<String>) -> GrammarError {
    GrammarError::InvalidRecord {
        record,
        detail: detail.into(),
    }
}
