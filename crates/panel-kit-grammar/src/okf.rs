//! The OKF engine: Open Knowledge Format bundles (YAML frontmatter plus
//! CommonMark) mapped onto a [`Site`]. OKF has no practical pest grammar, so
//! the *rules* are data in the package ([`OkfSpec`]) and this module owns the
//! I/O-free core ported from jump-cannon's `crates/okf-importer`: frontmatter
//! splitting, `OKF_LINK_RULES` walking, CommonMark link extraction, and
//! `resolve_reference`.
//!
//! The engine reads one JSON envelope — `{ bundles: [ { name, files: [ { path,
//! text } ] } ] }` — so it stays sync, pure, and `wasm32`-clean: no
//! filesystem walk, no threads, no clock.

use std::collections::{BTreeMap, HashSet};

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use serde_json::{Map, Value};

use crate::error::GrammarError;
use crate::package::{Limits, OkfSpec};
use crate::site::{Morphism, Object, Site};

/// Reserved by OKF §3.1 at any depth: directory listings and update logs are
/// not concepts.
const RESERVED: [&str; 2] = ["index.md", "log.md"];

/// The field the engine always derives, whatever the package projects: the
/// OKF §5.3 trust tier.
const TRUST: &str = "trust";

/// OKF §5.4 lifecycle values; anything else is a malformed concept.
const STATUSES: [&str; 3] = ["draft", "stable", "deprecated"];

/// An OKF engine bound to its rule tables.
pub struct CompiledOkf {
    spec: OkfSpec,
}

/// A parsed site plus the references that named no concept. A reference that
/// points outside the bundle, at an image, at a non-Markdown file, or at the
/// linking concept itself is not unresolved: it names no concept by design.
pub struct OkfParse {
    /// The mapped site.
    pub site: Site,
    /// References whose target names a Markdown concept path that no object
    /// carries.
    pub unresolved_links: usize,
}

/// Compile an OKF engine, bounding the rule table like an inline pest grammar.
pub fn compile(spec: &OkfSpec, grammar_bytes: usize) -> Result<CompiledOkf, GrammarError> {
    let bytes = serde_json::to_string(spec)
        .map_err(|error| GrammarError::InvalidRecord { record: "okf spec", detail: error.to_string() })?
        .len();
    if bytes > grammar_bytes {
        return Err(GrammarError::GrammarTooLarge { actual: bytes, max: grammar_bytes });
    }
    Ok(CompiledOkf { spec: spec.clone() })
}

/// One bundle's concepts, in envelope order.
struct Bundle {
    name: String,
    concepts: Vec<Concept>,
}

/// One parsed concept document.
struct Concept {
    /// Bundle-qualified id: `<bundle>/<bundle-relative path without .md>`.
    id: String,
    /// Bundle-relative path without `.md`: the base for document-relative links.
    path: String,
    /// Frontmatter `title`, or the file stem when it declares none.
    title: String,
    /// Site kind, from `kinds[concept type]` or `default_kind`.
    kind: String,
    /// Frontmatter `tags`, sorted and deduplicated.
    tags: Vec<String>,
    /// Projected frontmatter plus the derived trust tier.
    fields: BTreeMap<String, String>,
    /// Every link the concept declares, in source order: frontmatter rules
    /// first (rule order, then declaration order), then body links.
    targets: Vec<Target>,
}

/// One link declaration, resolved or not.
struct Target {
    /// The destination as written.
    raw: String,
    /// Whether a relative destination resolves against the linking document
    /// or against the bundle root.
    base: Base,
    /// The morphism kind this rule (or the body) contributes.
    kind: String,
}

/// What a relative destination resolves against.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Base {
    /// The linking document's directory.
    Document,
    /// The bundle root.
    Bundle,
}

impl CompiledOkf {
    /// Parse one OKF envelope into a site, enforcing limits.
    pub fn parse(&self, input: &str, limits: &Limits) -> Result<OkfParse, GrammarError> {
        let root: Value =
            serde_json::from_str(input).map_err(|error| GrammarError::JsonShape(error.to_string()))?;
        let records = root
            .pointer("/bundles")
            .ok_or_else(|| GrammarError::JsonShape("pointer '/bundles' did not resolve".to_owned()))?
            .as_array()
            .ok_or_else(|| GrammarError::JsonShape("pointer '/bundles' is not an array".to_owned()))?;
        if records.len() > limits.nodes {
            return Err(GrammarError::RecordLimit { kind: "nodes", actual: records.len(), max: limits.nodes });
        }

        let mut bundles: Vec<Bundle> = Vec::with_capacity(records.len());
        let mut objects: Vec<Object> = Vec::new();
        let mut names: HashSet<String> = HashSet::new();
        let mut ids: HashSet<String> = HashSet::new();
        for record in records {
            bundles.push(self.read_bundle(record, limits, &mut objects, &mut ids, &mut names)?);
        }
        // Bundles sort before concepts: the envelope lists them first, and
        // containment reads bundle → concept.
        objects.splice(0..0, bundles.iter().map(|bundle| bundle_object(bundle, &self.spec)));

        let ids: HashSet<&str> = objects.iter().map(|object| object.id.as_str()).collect();
        let mut morphisms: Vec<Morphism> = Vec::new();
        let mut seen: HashSet<(&str, &str, &str)> = HashSet::new();
        let mut unresolved_links = 0usize;
        for bundle in &bundles {
            // A bundle's containment morphisms, then its concepts' links, each
            // bundle in envelope order: the order the envelope is read in.
            for concept in &bundle.concepts {
                edges_fit(morphisms.len(), limits)?;
                if seen.insert((bundle.name.as_str(), concept.id.as_str(), self.spec.contains_kind.as_str())) {
                    morphisms.push(Morphism {
                        domain: bundle.name.clone(),
                        codomain: concept.id.clone(),
                        kind: Some(self.spec.contains_kind.clone()),
                    });
                }
            }
            for concept in &bundle.concepts {
                for target in &concept.targets {
                    edges_fit(morphisms.len(), limits)?;
                    let Some(path) = resolve_reference(&concept.path, &target.raw, target.base) else {
                        continue;
                    };
                    let candidate = format!("{}/{}", bundle.name, path);
                    let Some(codomain) = ids.get(candidate.as_str()).copied() else {
                        unresolved_links += 1;
                        continue;
                    };
                    if codomain == concept.id {
                        continue;
                    }
                    if seen.insert((concept.id.as_str(), codomain, target.kind.as_str())) {
                        morphisms.push(Morphism {
                            domain: concept.id.clone(),
                            codomain: codomain.to_owned(),
                            kind: Some(target.kind.clone()),
                        });
                    }
                }
            }
        }

        Ok(OkfParse { site: Site { objects, morphisms }, unresolved_links })
    }

    fn read_bundle(
        &self,
        record: &Value,
        limits: &Limits,
        objects: &mut Vec<Object>,
        ids: &mut HashSet<String>,
        names: &mut HashSet<String>,
    ) -> Result<Bundle, GrammarError> {
        let entry = record
            .as_object()
            .ok_or_else(|| invalid("bundle", "record is not a JSON object"))?;
        let name = string_member(entry, "bundle", "name")?;
        if name.is_empty() {
            return Err(invalid("bundle", "name is empty"));
        }
        if !names.insert(name.clone()) {
            return Err(GrammarError::DuplicateNodeId(name));
        }
        // A bundle object carries its bare name, so a name that also spells a
        // concept id would put two objects on one id.
        if !ids.insert(name.clone()) {
            return Err(GrammarError::DuplicateNodeId(name));
        }
        let files = entry
            .get("files")
            .ok_or_else(|| invalid("bundle", "missing files array"))?
            .as_array()
            .ok_or_else(|| invalid("bundle", "files member is not an array"))?;

        let mut concepts = Vec::with_capacity(files.len());
        for file in files {
            let Some(concept) = self.read_concept(&name, file)? else { continue };
            // Bundle objects count against the same node ceiling as concepts.
            if names.len() + objects.len() + 1 > limits.nodes {
                return Err(GrammarError::RecordLimit {
                    kind: "nodes",
                    actual: names.len() + objects.len() + 1,
                    max: limits.nodes,
                });
            }
            if !ids.insert(concept.id.clone()) {
                return Err(GrammarError::DuplicateNodeId(concept.id));
            }
            objects.push(concept.object());
            concepts.push(concept);
        }
        Ok(Bundle { name, concepts })
    }

    /// Parse one `{ path, text }` record, or `None` when the path is not a
    /// concept document: not Markdown, or a reserved OKF filename.
    fn read_concept(&self, bundle: &str, file: &Value) -> Result<Option<Concept>, GrammarError> {
        let entry = file
            .as_object()
            .ok_or_else(|| invalid("file", "record is not a JSON object"))?;
        let path = string_member(entry, "file", "path")?;
        let Some(relative) = path.strip_suffix(".md").filter(|stem| !stem.is_empty()) else {
            return Ok(None);
        };
        if is_reserved(&path) {
            return Ok(None);
        }
        let text = string_member(entry, "file", "text")?;

        let (frontmatter, body) = split_frontmatter(&text)
            .ok_or_else(|| invalid("concept", format!("{path} has missing or unterminated YAML frontmatter")))?;
        let yaml: Value = serde_saphyr::from_str(frontmatter)
            .map_err(|error| invalid("concept", format!("{path} has invalid YAML frontmatter: {error}")))?;
        let mapping = yaml
            .as_object()
            .ok_or_else(|| invalid("concept", format!("frontmatter of {path} must be a YAML mapping")))?;

        let concept_type = yaml_string(mapping, "type")
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| {
                invalid("concept", format!("frontmatter of {path} requires a non-empty string `type`"))
            })?
            .to_owned();
        if let Some(status) = mapping.get("status") {
            if !status.as_str().is_some_and(|status| STATUSES.contains(&status)) {
                return Err(invalid(
                    "concept",
                    format!("frontmatter `status` of {path} must be draft, stable, or deprecated"),
                ));
            }
        }
        let title = match yaml_string(mapping, "title").filter(|value| !value.trim().is_empty()) {
            Some(title) => title.to_owned(),
            None => relative.rsplit('/').next().unwrap_or(relative).to_owned(),
        };
        let kind = self
            .spec
            .kinds
            .get(&concept_type)
            .cloned()
            .unwrap_or_else(|| self.spec.default_kind.clone());
        let mut tags = frontmatter_tags(mapping, &path)?;
        tags.sort();
        tags.dedup();

        let mut fields = BTreeMap::new();
        for key in &self.spec.fields {
            if let Some(text) = mapping.get(key).and_then(field_text) {
                fields.insert(key.clone(), text);
            }
        }
        // The trust tier is derived, so it always wins over a projected
        // `trust` key.
        fields.insert(TRUST.to_owned(), trust_tier(mapping).to_owned());

        let document = Value::Object(mapping.clone());
        let mut targets = self.frontmatter_targets(&document);
        targets.extend(self.body_targets(body));
        let id = format!("{bundle}/{relative}");
        Ok(Some(Concept { id, path: relative.to_owned(), title, kind, tags, fields, targets }))
    }

    /// Walk each declared frontmatter pointer, in package order, binding every
    /// leaf string it names to that rule's morphism kind. OKF §6.2 permits an
    /// absolute URL, a bundle-relative `/path`, or a document-relative path in
    /// one field, so the base follows the text.
    fn frontmatter_targets(&self, mapping: &Value) -> Vec<Target> {
        let mut targets = Vec::new();
        for (pointer, kind) in &self.spec.link_rules {
            for leaf in path_strings(mapping, pointer) {
                let trimmed = leaf.trim();
                if trimmed.is_empty() {
                    continue;
                }
                let base = if trimmed.starts_with("./") || trimmed.starts_with("../") {
                    Base::Document
                } else {
                    Base::Bundle
                };
                targets.push(Target { raw: trimmed.to_owned(), base, kind: kind.clone() });
            }
        }
        targets
    }

    /// CommonMark links in the body, sorted and deduplicated, bound to
    /// `body_links`. Every body link is document-relative.
    fn body_targets(&self, body: &str) -> Vec<Target> {
        let mut links = markdown_links(body);
        links.sort();
        links.dedup();
        links
            .into_iter()
            .map(|raw| Target { raw, base: Base::Document, kind: self.spec.body_links.clone() })
            .collect()
    }
}

/// Refuse to grow past the morphism ceiling rather than truncate.
fn edges_fit(count: usize, limits: &Limits) -> Result<(), GrammarError> {
    if count > limits.edges {
        return Err(GrammarError::RecordLimit { kind: "edges", actual: count, max: limits.edges });
    }
    Ok(())
}

impl Concept {
    /// The site object this concept contributes.
    fn object(&self) -> Object {
        Object { id: self.id.clone(), title: self.title.clone(), kind: self.kind.clone(), tags: self.tags.clone(), fields: self.fields.clone() }
    }
}

fn bundle_object(bundle: &Bundle, spec: &OkfSpec) -> Object {
    Object {
        id: bundle.name.clone(),
        title: bundle.name.clone(),
        kind: spec.bundle_kind.clone(),
        tags: Vec::new(),
        fields: BTreeMap::new(),
    }
}

/// Walk a dotted YAML pointer and collect every leaf string. A `[*]` segment
/// iterates the named field as a sequence; a bare segment recurses into a
/// mapping by key; landing on a scalar or a sequence collects its elements.
fn path_strings(value: &Value, pointer: &str) -> Vec<String> {
    let mut out = Vec::new();
    walk_path(value, pointer, &mut out);
    out
}

fn walk_path(value: &Value, pointer: &str, out: &mut Vec<String>) {
    let (segment, rest) = pointer.split_once('.').map_or((pointer, ""), |(head, tail)| (head, tail));

    if let Some(field) = segment.strip_suffix("[*]") {
        let sequence = if field.is_empty() {
            value.as_array()
        } else {
            mapping_get(value, field).and_then(Value::as_array)
        };
        let Some(sequence) = sequence else { return };
        for item in sequence {
            if rest.is_empty() {
                collect_scalars(item, out);
            } else {
                walk_path(item, rest, out);
            }
        }
        return;
    }

    if rest.is_empty() {
        match mapping_get(value, segment) {
            Some(child) => collect_scalars(child, out),
            None => collect_scalars(value, out),
        }
        return;
    }

    if let Some(child) = mapping_get(value, segment) {
        walk_path(child, rest, out);
    }
}

fn mapping_get<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    value.as_object().and_then(|map| map.get(key))
}

fn collect_scalars(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::String(text) => out.push(text.clone()),
        Value::Array(items) => {
            for item in items {
                collect_scalars(item, out);
            }
        }
        _ => {}
    }
}

/// Whether `path` names an OKF reserved file at any depth (§3.1).
fn is_reserved(path: &str) -> bool {
    path.rsplit('/').next().is_some_and(|name| RESERVED.contains(&name))
}

/// Split `---\n`-delimited YAML frontmatter from the Markdown body.
fn split_frontmatter(raw: &str) -> Option<(&str, &str)> {
    let rest = raw.strip_prefix("---\n").or_else(|| raw.strip_prefix("---\r\n"))?;
    let mut offset = 0;
    for line in rest.split_inclusive('\n') {
        if line.trim_end_matches(['\r', '\n']) == "---" {
            return Some((&rest[..offset], &rest[offset + line.len()..]));
        }
        offset += line.len();
    }
    None
}

/// Standard CommonMark link destinations. A link whose text contains an image
/// is prose, not a reference.
fn markdown_links(body: &str) -> Vec<String> {
    let mut pending: Option<(String, bool)> = None;
    let mut links = Vec::new();
    for event in Parser::new_ext(body, Options::all()) {
        match event {
            Event::Start(Tag::Link { dest_url, .. }) => pending = Some((dest_url.into_string(), false)),
            Event::Start(Tag::Image { .. }) => {
                if let Some(link) = &mut pending {
                    link.1 = true;
                }
            }
            Event::End(TagEnd::Link) => {
                if let Some((destination, contains_image)) = pending.take() {
                    if !contains_image {
                        links.push(destination);
                    }
                }
            }
            _ => {}
        }
    }
    links
}

/// Turn a link destination into a bundle-relative concept path, or `None` for
/// anything that cannot name one: external, protocol-relative, query-bearing,
/// non-Markdown, or escaping the bundle.
fn resolve_reference(source_path: &str, raw: &str, base: Base) -> Option<String> {
    let without_fragment = raw.split('#').next()?.trim();
    if without_fragment.is_empty()
        || without_fragment.contains('?')
        || without_fragment.starts_with("//")
        || has_uri_scheme(without_fragment)
    {
        return None;
    }
    let decoded = percent_decode(without_fragment)?;
    if decoded.contains('\0') || decoded.contains('\\') || decoded.starts_with("//") || has_uri_scheme(&decoded) {
        return None;
    }
    let root_relative = decoded.starts_with('/');
    let mut segments: Vec<&str> = Vec::new();
    if !root_relative && base == Base::Document {
        segments.extend(
            source_path
                .rsplit_once('/')
                .map_or("", |(parent, _)| parent)
                .split('/')
                .filter(|segment| !segment.is_empty()),
        );
    }
    for segment in decoded.trim_start_matches('/').split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                segments.pop()?;
            }
            value => segments.push(value),
        }
    }
    let path = segments.join("/");
    path.strip_suffix(".md").map(str::to_owned)
}

/// Whether `value` starts with a URI scheme (`https:`, `mailto:`, `jira:`).
fn has_uri_scheme(value: &str) -> bool {
    let before_slash = value.split('/').next().unwrap_or(value);
    before_slash.find(':').is_some_and(|index| {
        let scheme = &before_slash.as_bytes()[..index];
        scheme.first().is_some_and(u8::is_ascii_alphabetic)
            && scheme[1..].iter().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'-' | b'.'))
    })
}

/// Decode `%XX` escapes; `None` when an escape is not a byte or the result is
/// not UTF-8, so a malformed destination simply names no concept.
fn percent_decode(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            out.push(u8::from_str_radix(value.get(index + 1..index + 3)?, 16).ok()?);
            index += 3;
        } else {
            out.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// The OKF §5.3 trust tier, derived from `verified` and `generated`.
fn trust_tier(mapping: &Map<String, Value>) -> &'static str {
    if verified_actors(mapping).iter().any(|actor| actor.starts_with("human:")) {
        "human-reviewed"
    } else if mapping.get("generated").is_some_and(|value| !value.is_null()) {
        "machine-generated"
    } else {
        "unattested"
    }
}

/// The `by` actor of every `verified` entry. A bare `{ by, at }` mapping counts
/// as a one-element list (§5.2).
fn verified_actors(mapping: &Map<String, Value>) -> Vec<&str> {
    let entries: Vec<&Map<String, Value>> = match mapping.get("verified") {
        Some(Value::Object(entry)) => vec![entry],
        Some(Value::Array(entries)) => entries.iter().filter_map(Value::as_object).collect(),
        _ => Vec::new(),
    };
    let mut actors: Vec<&str> = entries
        .iter()
        .filter_map(|entry| entry.get("by").and_then(Value::as_str))
        .filter(|actor| !actor.trim().is_empty())
        .collect();
    actors.sort_unstable();
    actors.dedup();
    actors
}

fn frontmatter_tags(mapping: &Map<String, Value>, path: &str) -> Result<Vec<String>, GrammarError> {
    let invalid = || invalid("concept", format!("frontmatter `tags` of {path} must be a list of non-empty strings"));
    match mapping.get("tags") {
        None => Ok(Vec::new()),
        Some(Value::Array(items)) => items
            .iter()
            .map(|item| item.as_str().filter(|tag| !tag.trim().is_empty()).map(str::to_owned).ok_or_else(invalid))
            .collect(),
        Some(_) => Err(invalid()),
    }
}

/// Project one frontmatter value into a field string: scalars as the text a
/// pest capture would carry, lists and maps as compact JSON.
fn field_text(value: &Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(flag) => Some(flag.to_string()),
        other => Some(serde_json::to_string(other).ok()?),
    }
}

fn yaml_string<'a>(mapping: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    mapping.get(key).and_then(Value::as_str)
}

fn string_member(
    object: &Map<String, Value>,
    record: &'static str,
    member: &'static str,
) -> Result<String, GrammarError> {
    object
        .get(member)
        .ok_or_else(|| invalid(record, format!("missing {member} member")))?
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| invalid(record, format!("{member} must be a string")))
}

fn invalid(record: &'static str, detail: impl Into<String>) -> GrammarError {
    GrammarError::InvalidRecord { record, detail: detail.into() }
}
