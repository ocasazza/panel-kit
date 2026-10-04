//! The JSON engine: resolve two JSON pointers to node and edge arrays, then
//! map each object's members onto the canonical site by name. Numbers are
//! stringified with `serde_json`'s shortest round-trip form so a JSON field and
//! the same value written as pest capture text compare equal.

use std::collections::{BTreeMap, HashSet};

use serde_json::Value;

use crate::error::GrammarError;
use crate::site::{Morphism, Site, Object};
use crate::package::{JsonEdgeMap, JsonNodeMap, JsonSpec, Limits};

/// A JSON engine bound to its field map.
pub struct CompiledJson {
    spec: JsonSpec,
}

/// Compile a JSON engine. The field map needs no further validation.
pub fn compile(spec: &JsonSpec) -> CompiledJson {
    CompiledJson { spec: spec.clone() }
}

impl CompiledJson {
    /// Parse a JSON document into a site.
    pub fn parse(&self, input: &str, limits: &Limits) -> Result<Site, GrammarError> {
        let root: Value =
            serde_json::from_str(input).map_err(|error| GrammarError::JsonShape(error.to_string()))?;

        let nodes_value = array_at(&root, &self.spec.nodes)?;
        if nodes_value.len() > limits.nodes {
            return Err(GrammarError::RecordLimit {
                kind: "nodes",
                actual: nodes_value.len(),
                max: limits.nodes,
            });
        }

        let mut nodes = Vec::with_capacity(nodes_value.len());
        let mut ids = HashSet::new();
        for value in nodes_value {
            let node = map_node(value, &self.spec.node)?;
            if !ids.insert(node.id.clone()) {
                return Err(GrammarError::DuplicateNodeId(node.id));
            }
            nodes.push(node);
        }

        let edges = match (&self.spec.edges, &self.spec.edge) {
            (Some(pointer), Some(map)) => {
                let edges_value = array_at(&root, pointer)?;
                if edges_value.len() > limits.edges {
                    return Err(GrammarError::RecordLimit {
                        kind: "edges",
                        actual: edges_value.len(),
                        max: limits.edges,
                    });
                }
                let mut edges = Vec::with_capacity(edges_value.len());
                for value in edges_value {
                    edges.push(map_edge(value, map)?);
                }
                edges
            }
            (Some(_), None) => {
                return Err(GrammarError::JsonShape(
                    "edges pointer is set but no edge member map was provided".to_owned(),
                ))
            }
            (None, _) => Vec::new(),
        };

        for edge in &edges {
            let source = ids.contains(&edge.domain);
            let target = ids.contains(&edge.codomain);
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

        Ok(Site { objects: nodes, morphisms: edges })
    }
}

fn array_at<'a>(root: &'a Value, pointer: &str) -> Result<&'a Vec<Value>, GrammarError> {
    root.pointer(pointer)
        .ok_or_else(|| GrammarError::JsonShape(format!("pointer '{pointer}' did not resolve")))?
        .as_array()
        .ok_or_else(|| GrammarError::JsonShape(format!("pointer '{pointer}' is not an array")))
}

fn map_node(value: &Value, map: &JsonNodeMap) -> Result<Object, GrammarError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid("node", "record is not a JSON object"))?;

    let id = required_string(object, &map.id, "node", "id")?;
    if id.is_empty() {
        return Err(invalid("node", "id member is empty"));
    }
    let title = match object.get(&map.title) {
        Some(value) => string_member(value, "node", "title")?,
        None => id.clone(),
    };
    let kind = match object.get(&map.kind) {
        Some(value) => string_member(value, "node", "kind")?,
        None => String::new(),
    };

    let mut tags = Vec::new();
    if let Some(member) = &map.tags {
        if let Some(value) = object.get(member) {
            let array = value
                .as_array()
                .ok_or_else(|| invalid("node", format!("tags member '{member}' is not an array")))?;
            for entry in array {
                tags.push(string_member(entry, "node", "tag")?);
            }
        }
    }

    let mut fields = BTreeMap::new();
    for member in &map.fields {
        if let Some(value) = object.get(member) {
            if let Some(text) = scalar_text(value) {
                fields.insert(member.clone(), text);
            } else if !value.is_null() {
                return Err(invalid("node", format!("field '{member}' is not a scalar")));
            }
        }
    }

    Ok(Object {
        id,
        title,
        kind,
        tags,
        fields,
    })
}

fn map_edge(value: &Value, map: &JsonEdgeMap) -> Result<Morphism, GrammarError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid("edge", "record is not a JSON object"))?;

    let source = required_string(object, &map.source, "edge", "source")?;
    let target = required_string(object, &map.target, "edge", "target")?;
    if source.is_empty() || target.is_empty() {
        return Err(invalid("edge", "source and target members must be non-empty"));
    }

    let kind = match &map.kind {
        Some(member) => match object.get(member) {
            Some(value) => {
                let text = string_member(value, "edge", "kind")?;
                if text.is_empty() {
                    return Err(invalid("edge", "kind member is empty"));
                }
                Some(text)
            }
            None => None,
        },
        None => None,
    };

    Ok(Morphism { domain: source, codomain: target, kind })
}

fn required_string(
    object: &serde_json::Map<String, Value>,
    member: &str,
    record: &'static str,
    role: &'static str,
) -> Result<String, GrammarError> {
    let value = object
        .get(member)
        .ok_or_else(|| invalid(record, format!("missing {role} member '{member}'")))?;
    string_member(value, record, role)
}

fn string_member(value: &Value, record: &'static str, role: &'static str) -> Result<String, GrammarError> {
    value
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| invalid(record, format!("{role} must be a string")))
}

/// Render a scalar JSON value as the string a pest capture would carry.
fn scalar_text(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(flag) => Some(flag.to_string()),
        _ => None,
    }
}

fn invalid(record: &'static str, detail: impl Into<String>) -> GrammarError {
    GrammarError::InvalidRecord {
        record,
        detail: detail.into(),
    }
}
