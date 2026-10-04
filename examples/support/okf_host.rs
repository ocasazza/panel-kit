//! Host façade for the OKF graph-topoi demos (web and TUI).
//!
//! One grammar (`okf`) parses the pinned Open Knowledge Format snapshot into
//! a site; two regime topoi (`okf-catalog`, `okf-lineage`) take global
//! sections of it, and the geometric morphism `f` between them transports the
//! selection by `∃f` forward and `f*` back. Both regimes pull their physics
//! tuning from the physics base and `f` commutes over it, so the graph is one
//! graph read two ways: nodes and their order are shared, while each node's
//! sort and colour are the regime's.
//!
//! Four bindings are host overlays rather than sheaf content — the selected
//! concept, the layout transport, `f` itself, and the repository/grammar
//! tables — and the external `graph` panel's body is [`OkfHost::graph`]. Specs
//! arrive as JSON text, so this file has no build-time inputs: the web example
//! embeds them and the host-contract test reads them at run time.

use std::collections::{BTreeMap, BTreeSet};

use panel_kit_core::badge::Rgb;
use panel_kit_core::spec::{ExternalComponentSpec, WorkspaceSpec};
use panel_kit_core::theme::{Color, ColorTokens, ThemeTokens};
use panel_kit_core::widgets::graph::{GraphEdge, GraphLayout, GraphModel, GraphNode, LayoutAlgorithm};
use panel_kit_core::widgets::{ContentSpec, TextModel};
use panel_kit_grammar::{
    BaseTopos, GeometricMorphism, GlobalSections, GrammarPackage, Object, ParserSpec, Physics,
    Section, Site, Topos,
};

/// OKF catalog regime topos id (the default regime).
pub const CATALOG: &str = "topos-okf-catalog";
/// OKF lineage regime topos id.
pub const LINEAGE: &str = "topos-okf-lineage";

/// The host overlay listing the selected concept's attributes and neighbours.
pub const SELECTED: &str = "okf.selected";
/// The host overlay stating the layout transport along `f`.
pub const LAYOUT: &str = "okf.layout";
/// The host overlay rendering `f` against the active subobject.
pub const MORPHISM: &str = "okf.morphism";
/// The host overlay listing the repository and the grammar's rule tables.
pub const REPOSITORY: &str = "okf.repository";
/// The binding the external graph panel's body is mounted under.
pub const GRAPH: &str = "okf.graph";

/// The Nix-authored JSON a host is built from.
pub struct OkfSpecs<'a> {
    /// The `okf` grammar package.
    pub grammar: &'a str,
    /// The knowledge-catalog envelope.
    pub data: &'a str,
    /// `topos-okf-catalog`.
    pub catalog: &'a str,
    /// `topos-okf-lineage`.
    pub lineage: &'a str,
    /// `topos-physics`.
    pub base: &'a str,
    /// f: topos-okf-catalog -> topos-okf-lineage.
    pub morphism: &'a str,
    /// p_catalog into the base.
    pub catalog_physics: &'a str,
    /// p_lineage into the base.
    pub lineage_physics: &'a str,
}

/// One fibre of φ: the codomain sort, the domain sorts it merges, and how many
/// site nodes the fibre covers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PhiFibre {
    /// The codomain sort.
    pub codomain_sort: String,
    /// The domain sorts φ sends here, in sort order.
    pub domain_sorts: Vec<String>,
    /// Site nodes whose catalog sort lies in the fibre.
    pub nodes: usize,
}

/// What one topos switch did: the regimes either side of `f` and φ's fibres.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToposTransition {
    /// The regime left.
    pub from: String,
    /// The regime entered.
    pub to: String,
    /// φ's fibres over the codomain's object sorts, in sort order.
    pub fibres: Vec<PhiFibre>,
}

/// One regime: its validated spec and its `p_R*` physics tuning.
struct Regime {
    topos: Topos,
    physics: Physics,
}

impl Regime {
    /// Decode, validate, and pull the physics tuning back from the base.
    fn load(base: &BaseTopos, topos: &str, physics: &str) -> Result<Self, String> {
        let topos = Topos::from_json_str(topos).map_err(|e| e.to_string())?;
        topos.validate().map_err(|e| format!("{}: {e}", topos.id))?;
        let p = GeometricMorphism::from_json_str(physics).map_err(|e| e.to_string())?;
        let physics = Physics::new(base, &p, &topos).map_err(|e| format!("{}: {e}", topos.id))?;
        Ok(Regime { topos, physics })
    }

    /// The solver its external graph panel names, or `none`.
    fn algorithm(&self) -> &'static str {
        self.topos
            .workspace
            .panels
            .iter()
            .find_map(|panel| match &panel.content {
                ContentSpec::External { layout, .. } => Some(algorithm_name(&layout.algorithm)),
                _ => None,
            })
            .unwrap_or("none")
    }

    /// The regime object sort of `kind`, falling back to the site kind.
    fn sort_of<'a>(&'a self, kind: &'a str) -> &'a str {
        self.topos.object_sorting.get(kind).map_or(kind, String::as_str)
    }

    /// The regime morphism sort of `kind`, or `None` for an untyped morphism.
    fn morphism_sort(&self, kind: Option<&str>) -> Option<&str> {
        kind.and_then(|kind| self.topos.morphism_sorting.get(kind)).map(String::as_str)
    }
}

/// The four host overlays, rebuilt on every selection or regime change.
struct Overlays {
    selected: Section,
    layout: Section,
    morphism: Section,
    repository: Section,
}

impl Overlays {
    fn empty() -> Self {
        let blank = || Section::Text(TextModel { text: String::new() });
        Self { selected: blank(), layout: blank(), morphism: blank(), repository: blank() }
    }
}

/// The `okf` package's rule tables, kept for the repository overlay.
struct OkfRuleTables {
    default_kind: String,
    bundle_kind: String,
    body_links: String,
    kinds: BTreeMap<String, String>,
    link_rules: BTreeMap<String, String>,
}

/// Both OKF regimes over one parsed knowledge-catalog site.
pub struct OkfHost {
    rules: OkfRuleTables,
    repository: String,
    rev: String,
    site: Site,
    unresolved_links: usize,
    regimes: [Regime; 2],
    morphism: GeometricMorphism,
    sections: [GlobalSections; 2],
    active: usize,
    selected: Option<String>,
    subobject: BTreeSet<String>,
    overlays: Overlays,
}

impl OkfHost {
    /// Compile the grammar, parse the snapshot, take both regimes' sections,
    /// and validate `f` and its commutation with the base.
    pub fn load(specs: &OkfSpecs<'_>) -> Result<Self, String> {
        let package = GrammarPackage::from_json_str(specs.grammar).map_err(|e| e.to_string())?;
        let rules = match &package.parser {
            ParserSpec::Okf(spec) => OkfRuleTables {
                default_kind: spec.default_kind.clone(),
                bundle_kind: spec.bundle_kind.clone(),
                body_links: spec.body_links.clone(),
                kinds: spec.kinds.clone(),
                link_rules: spec.link_rules.clone(),
            },
            other => return Err(format!("the okf host needs the okf engine, not {other:?}")),
        };
        let grammar = package.compile().map_err(|e| e.to_string())?;
        let report = grammar.parse_reported(specs.data).map_err(|e| e.to_string())?;

        let base = BaseTopos::from_json_str(specs.base).map_err(|e| e.to_string())?;
        let regimes = [
            Regime::load(&base, specs.catalog, specs.catalog_physics)?,
            Regime::load(&base, specs.lineage, specs.lineage_physics)?,
        ];
        let morphism = GeometricMorphism::from_json_str(specs.morphism).map_err(|e| e.to_string())?;
        morphism.validate(&regimes[0].topos, &regimes[1].topos).map_err(|e| e.to_string())?;
        let p_catalog =
            GeometricMorphism::from_json_str(specs.catalog_physics).map_err(|e| e.to_string())?;
        let p_lineage =
            GeometricMorphism::from_json_str(specs.lineage_physics).map_err(|e| e.to_string())?;
        morphism.validate_over(&p_catalog, &p_lineage).map_err(|e| e.to_string())?;

        let sections = [
            regimes[0].topos.global_sections(&report.site, &regimes[0].physics).map_err(|e| e.to_string())?,
            regimes[1].topos.global_sections(&report.site, &regimes[1].physics).map_err(|e| e.to_string())?,
        ];
        let mut host = Self {
            rules,
            repository: envelope_header(specs.data, "repository"),
            rev: envelope_header(specs.data, "rev"),
            site: report.site,
            unresolved_links: report.unresolved_links,
            regimes,
            morphism,
            sections,
            active: 0,
            selected: None,
            subobject: BTreeSet::new(),
            overlays: Overlays::empty(),
        };
        host.recompute();
        Ok(host)
    }

    /// Regime ids in switcher order.
    pub fn topos_ids(&self) -> [&'static str; 2] {
        [CATALOG, LINEAGE]
    }

    /// The regime whose sections, graph, and workspace `resolve`, `graph`, and
    /// `workspace_spec` return.
    pub fn topos_id(&self) -> &str {
        &self.regimes[self.active].topos.id
    }

    /// Make `id` the active regime without transporting the selection; false
    /// when no regime has that id.
    pub fn set_topos(&mut self, id: &str) -> bool {
        let Some(index) = self.regimes.iter().position(|regime| regime.topos.id == id) else {
            return false;
        };
        self.active = index;
        self.subobject = self.selected_sorts();
        self.recompute();
        true
    }

    /// The active regime's workspace spec.
    pub fn workspace_spec(&self) -> &WorkspaceSpec {
        &self.regimes[self.active].topos.workspace
    }

    /// The active regime's workspace spec as JSON, for host spec resolvers.
    pub fn workspace_json(&self) -> String {
        self.regimes[self.active].topos.workspace_json()
    }

    /// The external components the active regime's workspace declares.
    pub fn components(&self) -> &[ExternalComponentSpec] {
        &self.regimes[self.active].topos.workspace.components
    }

    /// The parsed knowledge-catalog site.
    pub fn site(&self) -> &Site {
        &self.site
    }

    /// References the engine could not resolve to a concept of the site.
    pub fn unresolved_links(&self) -> usize {
        self.unresolved_links
    }

    /// The active regime's section for `binding`. The four host overlays win
    /// over the placeholder sheaves the workspace declares for them.
    pub fn resolve(&self, binding: &str) -> Option<&Section> {
        match binding {
            SELECTED => Some(&self.overlays.selected),
            LAYOUT => Some(&self.overlays.layout),
            MORPHISM => Some(&self.overlays.morphism),
            REPOSITORY => Some(&self.overlays.repository),
            other => self.sections[self.active].get(other),
        }
    }

    /// The selected node id.
    pub fn selected(&self) -> Option<&str> {
        self.selected.as_deref()
    }

    /// Select `node`, or clear the selection when it names no node of the site.
    pub fn select(&mut self, node: Option<&str>) {
        let next = node.filter(|id| self.object(id).is_some()).map(str::to_owned);
        if next != self.selected {
            self.selected = next;
            self.subobject = self.selected_sorts();
            self.recompute();
        }
    }

    /// Move the selection by `step` nodes in site order, wrapping around.
    pub fn cycle_selection(&mut self, step: isize) {
        let count = self.site.objects.len();
        if count == 0 {
            return;
        }
        let current = self
            .selected
            .as_deref()
            .and_then(|id| self.site.objects.iter().position(|object| object.id == id))
            .map_or(-step, |index| index as isize);
        let next = (current + step).rem_euclid(count as isize) as usize;
        self.selected = Some(self.site.objects[next].id.clone());
        self.subobject = self.selected_sorts();
        self.recompute();
    }

    /// The active subobject: the regime sorts covering the selected node,
    /// transported by [`switch_topos`](Self::switch_topos). Empty when nothing
    /// is selected.
    pub fn active_sorts(&self) -> BTreeSet<String> {
        self.subobject.clone()
    }

    /// Cross to the other regime along `f` (forward) or `f*` (back). The site
    /// is shared, so the selected node survives; only its regime reading moves,
    /// and φ's merges grow the subobject on the way back.
    pub fn switch_topos(&mut self) -> ToposTransition {
        let from = self.regimes[self.active].topos.id.clone();
        let sorts = self.subobject.clone();
        self.active = 1 - self.active;
        self.subobject = if self.active == 1 {
            self.morphism.image(&sorts)
        } else {
            self.morphism.pullback_subobject(&sorts)
        };
        self.recompute();
        ToposTransition { from, to: self.regimes[self.active].topos.id.clone(), fibres: self.fibres() }
    }

    /// φ's fibres over the codomain's object sorts, with node counts read off
    /// the domain's sorts.
    pub fn fibres(&self) -> Vec<PhiFibre> {
        let catalog = &self.regimes[0];
        self.regimes[1]
            .topos
            .stalks
            .objects
            .keys()
            .map(|codomain_sort| {
                let domain_sorts: Vec<String> = self
                    .morphism
                    .object_sorts
                    .iter()
                    .filter(|(_, image)| *image == codomain_sort)
                    .map(|(sort, _)| sort.clone())
                    .collect();
                let fibre: BTreeSet<&str> = domain_sorts.iter().map(String::as_str).collect();
                let nodes = self
                    .site
                    .objects
                    .iter()
                    .filter(|object| fibre.contains(catalog.sort_of(&object.kind)))
                    .count();
                PhiFibre { codomain_sort: codomain_sort.clone(), domain_sorts, nodes }
            })
            .collect()
    }

    /// The renderer-neutral graph over the whole site: nodes in site order,
    /// sorted and coloured by the active regime, edges typed by the active
    /// regime's morphism sorts and tuned by its physics.
    pub fn graph(&self) -> GraphModel {
        let regime = &self.regimes[self.active];
        let colors = ThemeTokens::dark().colors;
        let mut degree: BTreeMap<&str, usize> = BTreeMap::new();
        for morphism in &self.site.morphisms {
            *degree.entry(morphism.domain.as_str()).or_default() += 1;
            *degree.entry(morphism.codomain.as_str()).or_default() += 1;
        }
        let index: BTreeMap<&str, u32> =
            self.site.objects.iter().enumerate().map(|(at, object)| (object.id.as_str(), at as u32)).collect();

        let nodes = self
            .site
            .objects
            .iter()
            .map(|object| {
                let sort = regime.sort_of(&object.kind).to_owned();
                let tuning = regime.physics.object(&sort);
                GraphNode {
                    id: object.id.clone(),
                    label: object.title.clone(),
                    size: radius(degree.get(object.id.as_str()).copied().unwrap_or(0)),
                    color: rgb(color_of(&colors, &sort)),
                    sort,
                    mass: tuning.map_or(1.0, |tuning| tuning.mass),
                    repulsion: tuning.map_or(0.0, |tuning| tuning.repulsion),
                }
            })
            .collect();

        let edges = self
            .site
            .morphisms
            .iter()
            .map(|morphism| {
                let kind = regime.morphism_sort(morphism.kind.as_deref()).unwrap_or_default().to_owned();
                let tuning = regime.physics.morphism(&kind);
                GraphEdge {
                    source: index.get(morphism.domain.as_str()).copied().unwrap_or(0),
                    target: index.get(morphism.codomain.as_str()).copied().unwrap_or(0),
                    kind,
                    weight: tuning.map_or(1.0, |tuning| tuning.weight),
                    rest_length: tuning.map_or(1.0, |tuning| tuning.rest_length),
                }
            })
            .collect();
        GraphModel { nodes, edges }
    }

    /// The layout the active regime's external graph panel asks for.
    pub fn graph_layout(&self) -> GraphLayout {
        self.regimes[self.active]
            .topos
            .workspace
            .panels
            .iter()
            .find_map(|panel| match &panel.content {
                ContentSpec::External { layout, .. } => Some(layout.clone()),
                _ => None,
            })
            .unwrap_or_default()
    }

    fn object(&self, id: &str) -> Option<&Object> {
        self.site.objects.iter().find(|object| object.id == id)
    }

    /// The active regime's reading of the selected node's sorts.
    fn selected_sorts(&self) -> BTreeSet<String> {
        self.selected
            .as_deref()
            .and_then(|id| self.object(id))
            .map(|object| BTreeSet::from([self.regimes[self.active].sort_of(&object.kind).to_owned()]))
            .unwrap_or_default()
    }

    fn recompute(&mut self) {
        self.overlays.selected = Section::Text(TextModel { text: self.selected_text() });
        self.overlays.layout = Section::Text(TextModel { text: self.layout_text() });
        self.overlays.morphism = Section::Text(TextModel { text: self.morphism_text() });
        self.overlays.repository = Section::Text(TextModel { text: self.repository_text() });
    }

    /// The selected concept's attributes, then its neighbours under the active
    /// regime's morphism sorts.
    fn selected_text(&self) -> String {
        let regime = &self.regimes[self.active];
        let mut lines = vec!["Selected concept".to_owned()];
        let Some(id) = self.selected.as_deref() else {
            lines.push("nothing selected: press n or p, or click a node.".to_owned());
            return lines.join("\n");
        };
        let Some(object) = self.object(id) else {
            lines.push(format!("{id} is not a node of this site"));
            return lines.join("\n");
        };
        lines.push(format!("{}  [{} · {}]", object.id, object.kind, regime.sort_of(&object.kind)));
        lines.push(format!("title: {}", object.title));
        for (key, value) in &object.fields {
            lines.push(format!("{key}: {value}"));
        }
        if !object.tags.is_empty() {
            lines.push(format!("tags: {}", object.tags.join(", ")));
        }
        let neighbours: Vec<&panel_kit_grammar::Morphism> = self
            .site
            .morphisms
            .iter()
            .filter(|morphism| morphism.domain == *id || morphism.codomain == *id)
            .collect();
        lines.push(format!("neighbours ({}):", neighbours.len()));
        for morphism in neighbours {
            let forward = morphism.domain == *id;
            lines.push(format!(
                "  {} {} [{}]",
                if forward { "->" } else { "<-" },
                if forward { &morphism.codomain } else { &morphism.domain },
                regime.morphism_sort(morphism.kind.as_deref()).unwrap_or("untyped"),
            ));
        }
        lines.join("\n")
    }

    /// `f` read as a table over the domain's sorts, plus the adjoint triple
    /// against the active subobject.
    fn morphism_text(&self) -> String {
        let catalog = &self.regimes[0].topos;
        let lineage = &self.regimes[1].topos;
        let exists = self.morphism.image(&self.subobject);
        let universal = self.morphism.universal_image(&self.subobject);
        let back_image = self.morphism.pullback_subobject(&exists);
        let back = self.morphism.pullback_subobject(&self.subobject);
        let mut lines = vec![format!("f: {} -> {}", self.morphism.domain, self.morphism.codomain)];
        lines.push("sort | f(sort) | U | f*∃f".to_owned());
        for sort in catalog.stalks.objects.keys() {
            let image = self.morphism.object_sorts.get(sort).cloned().unwrap_or_default();
            lines.push(format!(
                "{} | {} | {} | {}",
                sort_label(catalog, sort),
                sort_label(lineage, &image),
                mark(self.subobject.contains(sort)),
                mark(back_image.contains(sort)),
            ));
        }
        lines.push(String::new());
        lines.push(format!("U = {}", names(&self.subobject)));
        lines.push(format!("∃f(U) = {}", names(&exists)));
        lines.push(format!("∀f(U) = {}", names(&universal)));
        lines.push(format!("f*(U) = {}", names(&back)));
        lines.push(format!("∀f∘f*(U) = {}", names(&self.morphism.universal_image(&back))));
        lines.push(format!("f*∃f(U) = {}", names(&back_image)));
        if self.subobject.is_subset(&back_image) && back_image.len() > self.subobject.len() {
            lines.push(format!("f*∃f(U) ⊋ U: f merges {} (information lost)", merges(&self.morphism, &exists)));
        }
        lines.join("\n")
    }

    /// The layout transport: each regime's solver, φ's fibres, and the tuning
    /// the codomain pulls back from the base.
    fn layout_text(&self) -> String {
        let lineage = &self.regimes[1];
        let mut lines = vec![
            format!(
                "layout: {} -> {}   (both grouped by sort, the codomain solved from the current positions)",
                self.regimes[0].algorithm(),
                lineage.algorithm(),
            ),
            String::new(),
            "φ fibres (catalog sorts -> lineage role : nodes):".to_owned(),
        ];
        for fibre in self.fibres() {
            lines.push(format!(
                "  {} -> {} : {} nodes",
                fibre.domain_sorts.join(", "),
                fibre.codomain_sort,
                fibre.nodes
            ));
        }
        lines.push(String::new());
        lines.push(format!("pulled back into {} by p_lineage*:", LINEAGE));
        for sort in lineage.topos.stalks.objects.keys() {
            if let Some(tuning) = lineage.physics.object(sort) {
                lines.push(format!(
                    "  {sort}: repulsion {}, mass {} [{}]",
                    tuning.repulsion, tuning.mass, tuning.engine_type
                ));
            }
        }
        for sort in lineage.topos.stalks.morphisms.keys() {
            if let Some(tuning) = lineage.physics.morphism(sort) {
                lines.push(format!(
                    "  {sort}: weight {}, rest {} [{}]",
                    tuning.weight, tuning.rest_length, tuning.engine_type
                ));
            }
        }
        lines.push("p_lineage . f = p_catalog: both regimes induce one physics section".to_owned());
        lines.join("\n")
    }

    /// The repository, the snapshot's shape, and the grammar's rule tables.
    fn repository_text(&self) -> String {
        let mut lines = vec![format!("repository: {}", self.repository), format!("rev: {}", self.rev)];
        let bundles: Vec<&Object> = self.site.objects.iter().filter(|object| object.kind == "bundle").collect();
        let listed: Vec<String> = bundles
            .iter()
            .map(|bundle| {
                let prefix = format!("{}/", bundle.id);
                let concepts =
                    self.site.objects.iter().filter(|object| object.id.starts_with(&prefix)).count();
                format!("{} ({concepts})", bundle.id)
            })
            .collect();
        lines.push(format!("bundles: {}", listed.join(", ")));
        lines.push(format!(
            "site: {} concepts, {} morphisms, {} unresolved links",
            self.site.objects.len() - bundles.len(),
            self.site.morphisms.len(),
            self.unresolved_links
        ));
        lines.push(format!(
            "grammar okf: default {}, bundles {}, body links {}",
            self.rules.default_kind, self.rules.bundle_kind, self.rules.body_links
        ));
        lines.push(format!(
            "kinds: {}",
            self.rules.kinds.iter().map(|(ty, kind)| format!("{ty} -> {kind}")).collect::<Vec<_>>().join(", ")
        ));
        lines.push(format!(
            "link rules: {}",
            self.rules.link_rules.iter().map(|(pointer, kind)| format!("{pointer} -> {kind}")).collect::<Vec<_>>().join(", ")
        ));
        lines.join("\n")
    }
}

/// A regime's display label for one of its object sorts.
fn sort_label(topos: &Topos, sort: &str) -> String {
    topos.stalks.objects.get(sort).map_or_else(|| sort.to_owned(), |stalk| stalk.label.clone())
}

/// `∈` when the sort is in the subobject, `·` when it is not.
fn mark(present: bool) -> &'static str {
    if present {
        "∈"
    } else {
        "·"
    }
}

/// A sort set as `{a, b}`, or `∅` when empty.
fn names(sorts: &BTreeSet<String>) -> String {
    if sorts.is_empty() {
        "∅".to_owned()
    } else {
        format!("{{{}}}", sorts.iter().cloned().collect::<Vec<_>>().join(", "))
    }
}

/// The non-trivial fibres of `exists`: `a, b -> t`, joined by `; `.
fn merges(morphism: &GeometricMorphism, exists: &BTreeSet<String>) -> String {
    let mut parts = Vec::new();
    for target in exists {
        let fibre: Vec<&str> = morphism
            .object_sorts
            .iter()
            .filter(|(_, image)| *image == target)
            .map(|(sort, _)| sort.as_str())
            .collect();
        if fibre.len() > 1 {
            parts.push(format!("{} -> {target}", fibre.join(", ")));
        }
    }
    parts.join("; ")
}

/// The solver's spec name, as `LayoutAlgorithm` serialises it.
fn algorithm_name(algorithm: &LayoutAlgorithm) -> &'static str {
    match algorithm {
        LayoutAlgorithm::Force => "force",
        LayoutAlgorithm::Circle => "circle",
        LayoutAlgorithm::Grid => "grid",
        LayoutAlgorithm::Concentric => "concentric",
        LayoutAlgorithm::Spectral => "spectral",
        LayoutAlgorithm::Dagre => "dagre",
        LayoutAlgorithm::Fcose => "fcose",
        LayoutAlgorithm::Sphere => "sphere",
    }
}

/// The palette slot a regime sort wears. Looked up by sort name, so a regime's
/// colouring is stable across switches and each regime paints its own sorts.
fn color_of(colors: &ColorTokens, sort: &str) -> Color {
    match sort {
        // okf-catalog: the normalised OKF types.
        "bundle" => colors.dim,
        "table" => colors.blue,
        "dataset" => colors.green,
        "policy" => colors.yellow,
        "metric" => colors.pink,
        "computation" => colors.red,
        "skill" => colors.accent,
        "reference" => colors.badge_info,
        "concept" => colors.line2,
        "log" => colors.line,
        // okf-lineage: the roles phi reads them as.
        "source" => colors.green,
        "derived" => colors.yellow,
        "operator" => colors.red,
        "context" => colors.dim,
        _ => colors.fg,
    }
}

fn rgb(color: Color) -> Rgb {
    (color.r, color.g, color.b)
}

/// Drawn radius in renderer units: incident morphisms, compressed.
fn radius(degree: usize) -> f32 {
    (2.0 + (degree as f32).sqrt() * 1.6).min(14.0)
}

/// The envelope's top-level string member `key`, read without a JSON parser
/// because this façade depends on std alone: the envelope is Nix-authored and
/// carries `repository` and `rev` ahead of `bundles`.
fn envelope_header(envelope: &str, key: &str) -> String {
    let needle = format!("\"{key}\"");
    let Some(at) = envelope.find(&needle) else {
        return String::new();
    };
    let Some(open) = envelope[at + needle.len()..].find(':') else {
        return String::new();
    };
    envelope[at + needle.len() + open + 1..]
        .trim_start()
        .strip_prefix('"')
        .map(|value| value.chars().take_while(|character| *character != '"').collect())
        .unwrap_or_default()
}
