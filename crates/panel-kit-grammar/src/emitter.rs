//! Emitters: the export side of a grammar. An emitter serializes objects of a
//! site in the wire format its grammar parses, so emitted bytes parse back to
//! the same objects; a set-only site revision is written through it.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value};

use crate::error::GrammarError;
use crate::package::{JsonSpec, Limits, PestEmitter};
use crate::pest_engine::CompiledPest;
use crate::site::{Object, Site};

/// A template hole.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Hole {
    Id,
    Title,
    Kind,
    Tags,
    Fields,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Token {
    Literal(String),
    Hole(Hole),
}

/// A pest line template with its separators.
pub(crate) struct CompiledTemplate {
    tokens: Vec<Token>,
    tags_separator: String,
    fields_separator: String,
    field_assign: String,
}

/// Parse a line template into literals and `{id}`/`{title}`/`{kind}`/`{tags}`/`{fields}` holes.
pub(crate) fn compile_template(spec: &PestEmitter) -> Result<CompiledTemplate, GrammarError> {
    let mut tokens = Vec::new();
    let mut rest = spec.object.as_str();
    while let Some(open) = rest.find('{') {
        if let Some(stray) = rest[..open].find('}') {
            return Err(GrammarError::InvalidTemplate(format!("unmatched '}}' at byte {stray}")));
        }
        if open > 0 {
            tokens.push(Token::Literal(rest[..open].to_owned()));
        }
        let close = rest[open..]
            .find('}')
            .ok_or_else(|| GrammarError::InvalidTemplate("unclosed '{'".to_owned()))?;
        let hole = match &rest[open + 1..open + close] {
            "id" => Hole::Id,
            "title" => Hole::Title,
            "kind" => Hole::Kind,
            "tags" => Hole::Tags,
            "fields" => Hole::Fields,
            other => return Err(GrammarError::InvalidTemplate(format!("unknown hole '{{{other}}}'"))),
        };
        tokens.push(Token::Hole(hole));
        rest = &rest[open + close + 1..];
    }
    if rest.contains('}') {
        return Err(GrammarError::InvalidTemplate("unmatched '}'".to_owned()));
    }
    if !rest.is_empty() {
        tokens.push(Token::Literal(rest.to_owned()));
    }
    if !tokens.contains(&Token::Hole(Hole::Id)) {
        return Err(GrammarError::InvalidTemplate("template has no {id} hole".to_owned()));
    }
    Ok(CompiledTemplate {
        tokens,
        tags_separator: spec.tags_separator.clone(),
        fields_separator: spec.fields_separator.clone(),
        field_assign: spec.field_assign.clone(),
    })
}

impl CompiledTemplate {
    fn render(&self, object: &Object) -> String {
        let mut line = String::new();
        for token in &self.tokens {
            match token {
                Token::Literal(text) => line.push_str(text),
                Token::Hole(Hole::Id) => line.push_str(&object.id),
                Token::Hole(Hole::Title) => line.push_str(&object.title),
                Token::Hole(Hole::Kind) => line.push_str(&object.kind),
                Token::Hole(Hole::Tags) => line.push_str(&object.tags.join(&self.tags_separator)),
                Token::Hole(Hole::Fields) => {
                    let pairs: Vec<String> = object
                        .fields
                        .iter()
                        .map(|(key, value)| format!("{key}{}{value}", self.field_assign))
                        .collect();
                    line.push_str(&pairs.join(&self.fields_separator));
                }
            }
        }
        line.push('\n');
        line
    }

    /// Reject a template whose output for a representative object does not
    /// parse back to that object (lost tags or fields, wrong shape).
    pub(crate) fn check_round_trip(&self, parser: &CompiledPest, limits: &Limits) -> Result<(), GrammarError> {
        let probe = Object {
            id: "x1".to_owned(),
            title: "t1".to_owned(),
            kind: "k1".to_owned(),
            tags: vec!["a1".to_owned(), "b1".to_owned()],
            fields: BTreeMap::from([("f1".to_owned(), "v1".to_owned()), ("f2".to_owned(), "v2".to_owned())]),
        };
        let line = self.render(&probe);
        let site = parser
            .parse(&line, limits)
            .map_err(|error| GrammarError::EmitterLosesData(format!("template output '{}' did not parse: {error}", line.trim_end())))?;
        if site.objects != [probe] || !site.morphisms.is_empty() {
            return Err(GrammarError::EmitterLosesData(format!(
                "template output '{}' parses to a different object",
                line.trim_end()
            )));
        }
        Ok(())
    }

    pub(crate) fn emit(&self, objects: &[&Object]) -> Vec<u8> {
        objects.iter().map(|object| self.render(object)).collect::<String>().into_bytes()
    }
}

/// Emit `objects` as records at the spec's object pointer, through its member
/// map, with an empty morphism array where the spec reads one. A field or tag
/// the map does not carry would be lost on re-parse, so it is an error.
pub(crate) fn emit_json(spec: &JsonSpec, objects: &[&Object]) -> Result<Vec<u8>, GrammarError> {
    let map = &spec.node;
    let declared: BTreeSet<&str> = map.fields.iter().map(String::as_str).collect();
    let mut records = Vec::with_capacity(objects.len());
    for object in objects {
        let mut record = Map::new();
        record.insert(map.id.clone(), Value::String(object.id.clone()));
        record.insert(map.title.clone(), Value::String(object.title.clone()));
        record.insert(map.kind.clone(), Value::String(object.kind.clone()));
        match &map.tags {
            Some(member) => {
                record.insert(member.clone(), Value::Array(object.tags.iter().cloned().map(Value::String).collect()));
            }
            None if !object.tags.is_empty() => {
                return Err(GrammarError::EmitterLosesData(format!(
                    "object '{}' has tags but the grammar maps no tags member",
                    object.id
                )))
            }
            None => {}
        }
        for (key, value) in &object.fields {
            if !declared.contains(key.as_str()) {
                return Err(GrammarError::EmitterLosesData(format!(
                    "object '{}' field '{key}' is not in the grammar's field map",
                    object.id
                )));
            }
            record.insert(key.clone(), Value::String(value.clone()));
        }
        records.push(Value::Object(record));
    }

    let mut root = Value::Null;
    put(&mut root, &spec.nodes, Value::Array(records))?;
    if let Some(pointer) = &spec.edges {
        put(&mut root, pointer, Value::Array(Vec::new()))?;
    }
    serde_json::to_vec(&root).map_err(|error| GrammarError::JsonShape(error.to_string()))
}

/// Place `value` at JSON `pointer` inside `root`, creating objects on the way.
fn put(root: &mut Value, pointer: &str, value: Value) -> Result<(), GrammarError> {
    if pointer.is_empty() {
        if !root.is_null() {
            return Err(GrammarError::JsonShape("root pointer overlaps another pointer".to_owned()));
        }
        *root = value;
        return Ok(());
    }
    let path = pointer
        .strip_prefix('/')
        .ok_or_else(|| GrammarError::JsonShape(format!("pointer '{pointer}' does not start with '/'")))?;
    let tokens: Vec<String> = path.split('/').map(|token| token.replace("~1", "/").replace("~0", "~")).collect();
    let mut cursor = root;
    for (index, token) in tokens.iter().enumerate() {
        if cursor.is_null() {
            *cursor = Value::Object(Map::new());
        }
        let object = cursor
            .as_object_mut()
            .ok_or_else(|| GrammarError::JsonShape(format!("pointer '{pointer}' overlaps another pointer")))?;
        if index + 1 == tokens.len() {
            if object.contains_key(token) {
                return Err(GrammarError::JsonShape(format!("pointer '{pointer}' overlaps another pointer")));
            }
            object.insert(token.clone(), value);
            return Ok(());
        }
        cursor = object.entry(token.clone()).or_insert(Value::Null);
    }
    Ok(())
}

/// The support's objects in site order, or the first support id the site lacks.
pub(crate) fn support_objects<'a>(site: &'a Site, support: &BTreeSet<String>) -> Result<Vec<&'a Object>, GrammarError> {
    for id in support {
        if !site.has_object(id) {
            return Err(GrammarError::UnknownSupportObject(id.clone()));
        }
    }
    Ok(site.objects.iter().filter(|object| support.contains(&object.id)).collect())
}
