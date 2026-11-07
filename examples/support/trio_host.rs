//! Host façade over `panel_kit_grammar` for the trio demo: agent traces and OKF
//! knowledge objects are **one site**, read in three regimes.
//!
//! One site document (`nix/data/trace.lines`, or the equivalent `trace.json`)
//! carries the sessions — planner, coder, reviewer, memory, with
//! `continues`/`shares`/`consolidates` — and the OKF objects beside them —
//! table, dataset, metric, computation, policy, skill, bundle, reference,
//! concept, log, with `derived_from`/`computes`/`executed_by`/`attested_by`/
//! `references`/`contains` — joined by two cross-morphisms the trace data
//! carries: `produces` (a session ↦ the object it produced) and `verifies` (a
//! reviewer's session ↦ the object it attests). Both grammars read the merged
//! site, so `g` re-reads it, not a second one.
//!
//! Three regimes are three views of that one site and are **always** all
//! available: `agentic`, `membrane`, `okf-catalog`, related by four geometric
//! morphisms — `f: agentic → membrane`, `ψ: okf-catalog → membrane`,
//! `χ: okf-catalog → agentic`, and `ω: agentic → okf-catalog`.
//!
//! # The sorting rule
//!
//! A regime sorts a kind **natively** when its topos is the identity on that
//! kind (`Topos::native_object_kinds`: the session kinds are native in
//! `agentic`, the OKF kinds in `okf-catalog`, and no kind in `membrane`).
//! Otherwise the kind is pushed along the morphism from the regime that does
//! sort it natively — the composite sorting functor. So every regime reads every
//! kind: sessions reach `membrane` along `f` and `okf-catalog` along `ω`;
//! knowledge objects reach `agentic` along `χ` and `membrane` along `ψ`.
//!
//! # The laws
//!
//! [`TrioHost::load`] checks each morphism against its domain and codomain,
//! checks the triangle `f ∘ χ = ψ` and the square `ψ ∘ ω = f`, checks that
//! each of the four commutes over the one physics base, and checks **per kind**
//! that the physics pulled back along every path from the kind's native regime
//! to the base is the same engine and the same numbers. A host that loads is a
//! host whose physics is one pullback and whose presentation may move along the
//! triangle and the square without touching the assembly.
//!
//! # Data transport
//!
//! A morphism carries attributes as well as sorts: `χ`/`ψ` map the agentic
//! `latency_ms`/`energy` of a knowledge object onto its `stale_after`, and `ω`
//! maps the catalog's `trust` onto a session's `energy` and its `stale_after`
//! onto the session's `latency_ms`; a field no map names crosses unchanged.
//! [`TrioHost::resolve`] therefore answers every binding of every regime with
//! real content over the merged site — the Morphism overlay states the maps.
//!
//! Only `std`, `panel_kit_core`, and `panel_kit_grammar` are used: this file is
//! `#[path]`-included by the root-crate and `panel-kit-tui` examples and by the
//! host-contract test.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use panel_kit_core::badge::Rgb;
use panel_kit_core::spec::{ExternalComponentSpec, WorkspaceSpec};
use panel_kit_core::theme::{Color, ColorTokens, ThemeTokens};
use panel_kit_core::widgets::graph::{GraphEdge, GraphLayout, GraphModel, GraphNode, LayoutAlgorithm};
use panel_kit_core::widgets::data_table::ColumnKind;
use panel_kit_core::widgets::table::{ColumnWidth, TableCell, TableColumn, TableModel, TableRow, TextAlign};
use panel_kit_core::widgets::{ContentSpec, TextModel};
use panel_kit_grammar::{
    assembly, AttributeMaps, BaseTopos, CompiledGrammar, ComponentInvariants, GeometricMorphism,
    GrammarPackage, Morphism, Object, Physics, Section, Site, Stage, Stalks, Topos,
};

/// Agentic-trace regime topos id.
pub const TOPOS_AGENTIC: &str = "topos-agentic";
/// Lipid self-assembly regime topos id.
pub const TOPOS_MEMBRANE: &str = "topos-membrane";
/// OKF catalog regime topos id.
pub const TOPOS_OKF_CATALOG: &str = "topos-okf-catalog";
/// Line-format site grammar id.
pub const GRAMMAR_LINES: &str = "trace-lines";
/// JSON-format site grammar id.
pub const GRAMMAR_JSON: &str = "trace-json";

/// The host overlay stating the routes taken, φ per kind, and the attribute
/// maps the route transports.
pub const MORPHISM: &str = "trio.morphism";
/// The host overlay stating the tuning the active regime pulls back.
pub const PHYSICS: &str = "trio.physics";
/// The host overlay listing the selected object.
pub const SELECTED: &str = "trio.selected";
/// The OKF catalog workspace's own Selected panel; it mounts [`SELECTED`].
pub const SELECTED_OKF: &str = "okf.selected";
/// The host overlay carrying the active grammar's own source document: the
/// `.pest` grammar behind `trace-lines`, the pointer map behind `trace-json`.
pub const GRAMMAR: &str = "trio.grammar";

/// The Morphism editor's binding in the agentic regime.
pub const MORPHISM_AGENTIC: &str = "agentic.morphism";
/// The Morphism editor's binding in the membrane regime.
pub const MORPHISM_MEMBRANE: &str = "membrane.morphism";
/// The Morphism editor's binding in the OKF catalog regime.
pub const MORPHISM_OKF: &str = "okf.morphism";

/// The Nix-authored JSON a trio host is built from: three regime topoi, the one
/// physics base, the four morphisms between the regimes, the three morphisms
/// from a regime into the base, and the merged site's two grammars and data.
pub struct TrioSpecs<'a> {
    /// `topos-agentic`.
    pub agentic: &'a str,
    /// `topos-membrane`.
    pub membrane: &'a str,
    /// `topos-okf-catalog`.
    pub okf_catalog: &'a str,
    /// `topos-physics`: the one base every regime pulls its tuning back from.
    pub base: &'a str,
    /// f: `topos-agentic` → `topos-membrane`.
    pub f: &'a str,
    /// ψ: `topos-okf-catalog` → `topos-membrane`.
    pub psi: &'a str,
    /// χ: `topos-okf-catalog` → `topos-agentic`.
    pub chi: &'a str,
    /// ω: `topos-agentic` → `topos-okf-catalog`.
    pub omega: &'a str,
    /// p_agentic into the base.
    pub agentic_physics: &'a str,
    /// p_membrane into the base.
    pub membrane_physics: &'a str,
    /// p_catalog into the base.
    pub catalog_physics: &'a str,
    /// The `trace-lines` grammar package.
    pub trace_lines_grammar: &'a str,
    /// The line-format site data.
    pub trace_lines_data: &'a str,
    /// The `trace-json` grammar package.
    pub trace_json_grammar: &'a str,
    /// The JSON-format site data, the same site as the line format.
    pub trace_json_data: &'a str,
}

/// One of the three regimes the one site is read in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RegimeId {
    /// Planner, coder, reviewer sessions and memory stores; the site kinds it
    /// sorts natively.
    Agentic,
    /// Lipids: heads, tails, solvent.
    Membrane,
    /// The normalised OKF types; the rest of the site's native kinds.
    OkfCatalog,
}

impl RegimeId {
    /// Every regime, in header order.
    pub const ALL: [RegimeId; 3] = [RegimeId::Agentic, RegimeId::Membrane, RegimeId::OkfCatalog];

    /// Display label.
    pub const fn label(self) -> &'static str {
        match self {
            RegimeId::Agentic => "agentic",
            RegimeId::Membrane => "membrane",
            RegimeId::OkfCatalog => "okf-catalog",
        }
    }

    /// The topos id this regime is specified by.
    pub const fn topos_id(self) -> &'static str {
        match self {
            RegimeId::Agentic => TOPOS_AGENTIC,
            RegimeId::Membrane => TOPOS_MEMBRANE,
            RegimeId::OkfCatalog => TOPOS_OKF_CATALOG,
        }
    }

    /// The binding the regime's external graph panel mounts.
    pub const fn graph_binding(self) -> &'static str {
        match self {
            RegimeId::Agentic => "agentic.graph",
            RegimeId::Membrane => "membrane.graph",
            RegimeId::OkfCatalog => "okf.graph",
        }
    }

    const fn index(self) -> usize {
        self as usize
    }
}

impl std::fmt::Display for RegimeId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.label())
    }
}

impl std::fmt::Display for MorphismId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.symbol())
    }
}

/// One of the four morphisms between the trio's regimes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MorphismId {
    /// f: `topos-agentic` → `topos-membrane`.
    F,
    /// ψ: `topos-okf-catalog` → `topos-membrane`.
    Psi,
    /// χ: `topos-okf-catalog` → `topos-agentic`.
    Chi,
    /// ω: `topos-agentic` → `topos-okf-catalog`.
    Omega,
}

impl MorphismId {
    /// Every morphism, in the order the triangle and the square name them.
    pub const ALL: [MorphismId; 4] =
        [MorphismId::F, MorphismId::Psi, MorphismId::Chi, MorphismId::Omega];

    /// The symbol the specs and panels use.
    pub const fn symbol(self) -> &'static str {
        match self {
            MorphismId::F => "f",
            MorphismId::Psi => "ψ",
            MorphismId::Chi => "χ",
            MorphismId::Omega => "ω",
        }
    }

    /// The morphism's own name: symbol, domain, codomain.
    pub const fn name(self) -> &'static str {
        match self {
            MorphismId::F => "f: topos-agentic -> topos-membrane",
            MorphismId::Psi => "ψ: topos-okf-catalog -> topos-membrane",
            MorphismId::Chi => "χ: topos-okf-catalog -> topos-agentic",
            MorphismId::Omega => "ω: topos-agentic -> topos-okf-catalog",
        }
    }
}

/// The morphism path from a kind's native regime to the regime reading it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Route {
    /// The regime that sorts the kind natively; every route starts there.
    pub native: RegimeId,
    /// The regime the kind is read in.
    pub regime: RegimeId,
    /// The morphisms taken from the native regime, in application order.
    pub path: Vec<MorphismId>,
    /// The other route the diagram's laws make the same map, when the regime
    /// has one: `χ` then `f` lands exactly where `ψ` does, and `ω` then `ψ`
    /// exactly where `f` does.
    pub composite: Option<Vec<MorphismId>>,
}

impl Route {
    /// Whether the kind is read in the regime that sorts it natively.
    pub fn is_native(&self) -> bool {
        self.path.is_empty()
    }

    /// One line naming the route: the regimes either side and the morphisms
    /// between them.
    pub fn text(&self) -> String {
        if self.is_native() {
            return format!("{} is native here", self.native.label());
        }
        format!("{} --{}--> {}", self.native.label(), path_text(&self.path), self.regime.label())
    }
}

/// A path as `χ then f`.
fn path_text(path: &[MorphismId]) -> String {
    path.iter().map(|step| step.symbol()).collect::<Vec<_>>().join(" then ")
}

/// One grammar: its compiled parser, the data it is read against, and the
/// source document an editor panel shows.
struct GrammarEntry {
    id: &'static str,
    grammar: CompiledGrammar,
    data: String,
    source: String,
    language: &'static str,
}

impl GrammarEntry {
    fn compile(id: &'static str, package: &str, data: &str) -> Result<Self, String> {
        let parsed =
            GrammarPackage::from_json_str(package).map_err(|error| format!("{id}: {error}"))?;
        let source = parsed.source_document();
        let language = parsed.source_language();
        let grammar = parsed.compile().map_err(|error| format!("{id}: {error}"))?;
        if grammar.id() != id {
            return Err(format!("{id}: the package compiles the grammar {}", grammar.id()));
        }
        Ok(GrammarEntry { id, grammar, data: data.to_owned(), source, language })
    }
}

/// One regime: its validated spec, its `p_R*` tuning, the morphism that pulls
/// the tuning back from the base, and the bonds its assembly counts.
struct Regime {
    id: RegimeId,
    topos: Topos,
    physics: Physics,
    into_base: GeometricMorphism,
    assembly_sorts: BTreeSet<String>,
    assembly_kinds: BTreeSet<String>,
    graph: Section,
}

impl Regime {
    /// Decode, validate, and pull the tuning back from the one base.
    fn load(base: &BaseTopos, topos: &str, p: GeometricMorphism, id: RegimeId) -> Result<Self, String> {
        let topos = Topos::from_json_str(topos).map_err(|error| error.to_string())?;
        topos.validate().map_err(|error| format!("{}: {error}", topos.id))?;
        let physics =
            Physics::new(base, &p, &topos).map_err(|error| format!("{}: {error}", topos.id))?;
        let graph = Section::Text(TextModel {
            text: format!(
                "{} mounts the host's {} model.\nThe body is TrioHost::graph(): the site's objects \
                 in site order, sorted and coloured by {}, tuned by {} over one physics section.\n\
                 Solved with the {} layout.",
                topos.id,
                id.graph_binding(),
                id.label(),
                topos.id,
                solver_name(&graph_layout_of(&topos).algorithm),
            ),
        });
        Ok(Regime {
            id,
            topos,
            physics,
            into_base: p,
            assembly_sorts: assembly_sorts(id),
            assembly_kinds: assembly_kinds(id),
            graph,
        })
    }

    /// The regime object sort of `kind`.
    fn sort_of<'a>(&'a self, kind: &'a str) -> &'a str {
        self.topos.object_sorting.get(kind).map_or(kind, String::as_str)
    }

    /// The regime morphism sort of `kind`, or `None` for an untyped morphism.
    fn morphism_sort(&self, kind: Option<&str>) -> Option<&str> {
        kind.and_then(|kind| self.topos.morphism_sorting.get(kind)).map(String::as_str)
    }

    /// The display label the regime gives one of its sorts.
    fn sort_label(&self, sort: &str) -> String {
        self.topos
            .stalks
            .objects
            .get(sort)
            .map_or_else(|| sort.to_owned(), |stalk| stalk.label.clone())
    }

    /// The base engine sort this regime's `p_R*` sends one of its sorts to.
    fn engine_object(&self, sort: &str) -> Option<&str> {
        self.into_base.object_sorts.get(sort).map(String::as_str)
    }

    /// The base engine sort this regime's `p_R*` sends one of its morphism
    /// sorts to.
    fn engine_morphism(&self, sort: &str) -> Option<&str> {
        self.into_base.morphism_sorts.get(sort).map(String::as_str)
    }
}

/// One regime's resolved panels over the site.
#[derive(Default)]
struct Projection {
    sections: BTreeMap<String, Section>,
    unreadable: BTreeSet<String>,
}

/// The host overlays, rebuilt on every selection, regime, or grammar change.
/// `morphism` states the routes in prose; `morphism_spec` is the active
/// route's morphism document, which the Morphism editor mounts.
struct Overlays {
    selected: Section,
    morphism: Section,
    morphism_spec: Section,
    grammar: Section,
    physics: Section,
}

impl Overlays {
    fn empty() -> Self {
        let blank = || Section::Text(TextModel { text: String::new() });
        Self {
            selected: blank(),
            morphism: blank(),
            morphism_spec: blank(),
            grammar: blank(),
            physics: blank(),
        }
    }
}

/// One site in three regimes under the four morphisms that relate them, with
/// every panel of every regime already resolved.
pub struct TrioHost {
    base: BaseTopos,
    regimes: [Regime; 3],
    f: GeometricMorphism,
    psi: GeometricMorphism,
    chi: GeometricMorphism,
    omega: GeometricMorphism,
    grammars: [GrammarEntry; 2],
    grammar: usize,
    site: Site,
    unresolved_links: usize,
    projections: [Projection; 3],
    attributes: [AttributeMaps; 3],
    regime: RegimeId,
    selected: Option<String>,
    subobject: BTreeSet<String>,
    overlays: Overlays,
}

impl TrioHost {
    /// Compile the site's two grammars, decode and validate the base, the
    /// three regimes and the three morphisms into the base, check `f`, `ψ`,
    /// `χ`, and `ω` against their domain and codomain, check the triangle
    /// `f ∘ χ = ψ` and the square `ψ ∘ ω = f`, check that each of the four
    /// commutes over the base, check per kind that the physics pulled back
    /// along every path to the base agrees, and take every regime's panels.
    ///
    /// The laws come in the order they constrain each other: the triangle and
    /// the square first, because a morphism is pinned by the two sortings it
    /// has to agree with, then the sortings, then commutation with the base.
    pub fn load(specs: &TrioSpecs<'_>) -> Result<Self, String> {
        let base = BaseTopos::from_json_str(specs.base).map_err(|error| error.to_string())?;
        let morphism = |label: &str, json: &str| {
            GeometricMorphism::from_json_str(json)
                .map_err(|error| format!("{label} does not decode: {error}"))
        };
        let f = morphism(MorphismId::F.name(), specs.f)?;
        let psi = morphism(MorphismId::Psi.name(), specs.psi)?;
        let chi = morphism(MorphismId::Chi.name(), specs.chi)?;
        let omega = morphism(MorphismId::Omega.name(), specs.omega)?;
        let p_agentic = morphism("p_agentic", specs.agentic_physics)?;
        let p_membrane = morphism("p_membrane", specs.membrane_physics)?;
        let p_catalog = morphism("p_catalog", specs.catalog_physics)?;
        let regimes = [
            Regime::load(&base, specs.agentic, p_agentic.clone(), RegimeId::Agentic)?,
            Regime::load(&base, specs.membrane, p_membrane.clone(), RegimeId::Membrane)?,
            Regime::load(&base, specs.okf_catalog, p_catalog.clone(), RegimeId::OkfCatalog)?,
        ];

        // The diagram comes first. Each morphism is pinned by the two sortings
        // it has to agree with, so a document that is not the one the others
        // compose to is reported against those — which says which document is
        // wrong and where — rather than against a sorting law it breaks
        // downstream of that disagreement.
        let triangle = chi
            .compose(&f)
            .map_err(|error| format!("the triangle f ∘ χ = ψ does not compose: {error}"))?;
        check_diagram(&triangle, &psi, &MorphismId::Psi, "f ∘ χ = ψ", "χ then f")?;
        let square = omega
            .compose(&psi)
            .map_err(|error| format!("the square ψ ∘ ω = f does not compose: {error}"))?;
        check_diagram(&square, &f, &MorphismId::F, "ψ ∘ ω = f", "ω then ψ")?;

        f.validate(&regimes[0].topos, &regimes[1].topos)
            .map_err(|error| format!("f is not a morphism of its topos specs: {error}"))?;
        psi.validate(&regimes[2].topos, &regimes[1].topos)
            .map_err(|error| format!("ψ is not a morphism of its topos specs: {error}"))?;
        chi.validate(&regimes[2].topos, &regimes[0].topos)
            .map_err(|error| format!("χ is not a morphism of its topos specs: {error}"))?;
        omega
            .validate(&regimes[0].topos, &regimes[2].topos)
            .map_err(|error| format!("ω is not a morphism of its topos specs: {error}"))?;

        f.validate_over(&p_agentic, &p_membrane)
            .map_err(|error| format!("p_membrane ∘ f = p_agentic over the base fails: {error}"))?;
        psi.validate_over(&p_catalog, &p_membrane)
            .map_err(|error| format!("p_membrane ∘ ψ = p_catalog over the base fails: {error}"))?;
        chi.validate_over(&p_catalog, &p_agentic)
            .map_err(|error| format!("p_agentic ∘ χ = p_catalog over the base fails: {error}"))?;
        omega
            .validate_over(&p_agentic, &p_catalog)
            .map_err(|error| format!("p_catalog ∘ ω = p_agentic over the base fails: {error}"))?;

        let grammars = [
            GrammarEntry::compile(GRAMMAR_LINES, specs.trace_lines_grammar, specs.trace_lines_data)?,
            GrammarEntry::compile(GRAMMAR_JSON, specs.trace_json_grammar, specs.trace_json_data)?,
        ];
        let parsed = parse_site(&grammars[0])?;
        let site = parsed.site;

        let morphisms = [&f, &psi, &chi, &omega];
        let all = [&regimes[0], &regimes[1], &regimes[2]];
        let kinds = site_kind_set(&site);
        let attributes = [
            attribute_maps(&regimes[0], all, morphisms, kinds.clone()),
            attribute_maps(&regimes[1], all, morphisms, kinds.clone()),
            attribute_maps(&regimes[2], all, morphisms, kinds),
        ];
        let projections = [
            project(&regimes[0], &site, &attributes[0]),
            project(&regimes[1], &site, &attributes[1]),
            project(&regimes[2], &site, &attributes[2]),
        ];

        let mut host = Self {
            base,
            regimes,
            f: f.clone(),
            psi: psi.clone(),
            chi: chi.clone(),
            omega: omega.clone(),
            grammars,
            grammar: 0,
            site: site.clone(),
            unresolved_links: parsed.unresolved_links,
            projections,
            attributes,
            regime: RegimeId::Agentic,
            selected: None,
            subobject: BTreeSet::new(),
            overlays: Overlays::empty(),
        };
        check_physics_per_kind(&host.base, &host.regimes, &site, [&f, &psi, &chi, &omega])?;
        host.subobject = host.selected_sorts();
        host.recompute();
        Ok(host)
    }

    /// The parsed objects of the one site.
    pub fn site(&self) -> &Site {
        &self.site
    }

    /// The regime the site is read in.
    pub fn regime(&self) -> RegimeId {
        self.regime
    }

    /// The route the active regime takes for the site kind `kind`: empty when
    /// this regime sorts it natively, otherwise the morphisms from the regime
    /// that does, together with the route the diagram's laws make the same
    /// map.
    pub fn route(&self, kind: &str) -> Route {
        self.route_to(kind, self.regime)
    }

    pub fn route_to(&self, kind: &str, regime: RegimeId) -> Route {
        self.route_from(self.native_regime(kind), regime)
    }

    /// The route from a regime that sorts kinds natively to `regime`: the
    /// morphisms of the diagram, and the other route the laws make the same
    /// map when the regime has one (`χ` then `f` lands where `ψ` does, and `ω`
    /// then `ψ` lands where `f` does).
    pub fn route_from(&self, native: RegimeId, regime: RegimeId) -> Route {
        let composite = match (native, regime) {
            (RegimeId::OkfCatalog, RegimeId::Membrane) => {
                Some(vec![MorphismId::Chi, MorphismId::F])
            }
            (RegimeId::Agentic, RegimeId::Membrane) => {
                Some(vec![MorphismId::Omega, MorphismId::Psi])
            }
            _ => None,
        };
        Route { native, regime, path: self.path(native, regime), composite }
    }

    /// Every native regime on the site, with how many kinds and objects it
    /// sorts natively, in regime order.
    pub fn native_regimes(&self) -> Vec<(RegimeId, usize, usize)> {
        RegimeId::ALL
            .into_iter()
            .map(|regime| {
                let kinds = self
                    .site
                    .objects
                    .iter()
                    .filter(|object| self.native_regime(&object.kind) == regime)
                    .map(|object| object.kind.clone())
                    .collect::<BTreeSet<_>>();
                let objects = self
                    .site
                    .objects
                    .iter()
                    .filter(|object| kinds.contains(&object.kind))
                    .count();
                (regime, kinds.len(), objects)
            })
            .filter(|(_, kinds, _)| *kinds > 0)
            .collect()
    }

    /// One line naming, per native regime, how many objects it sorts natively
    /// and the route from there to the active regime.
    pub fn route_summary(&self) -> String {
        let mut parts = Vec::new();
        for (native, _, objects) in self.native_regimes() {
            let route = self.route_from(native, self.regime);
            parts.push(format!("{objects} objects {}", route.text()));
        }
        parts.join(" · ")
    }

    /// The regime that sorts `kind` natively — the one whose topos is the
    /// identity on it — or the agentic regime when no topos is.
    pub fn native_regime(&self, kind: &str) -> RegimeId {
        native_regime_of(&[&self.regimes[0], &self.regimes[1], &self.regimes[2]], kind)
    }

    /// The grammar ids that read the site, in cycle order.
    pub fn grammar_ids(&self) -> &'static [&'static str] {
        &[GRAMMAR_LINES, GRAMMAR_JSON]
    }

    /// The grammar that read the site.
    pub fn grammar_id(&self) -> &'static str {
        self.grammars[self.grammar].id
    }

    /// Read the site with another of its grammars; false when that grammar
    /// cannot read the data.
    pub fn set_grammar(&mut self, id: &str) -> bool {
        match self.grammars.iter().position(|entry| entry.id == id) {
            Some(index) => self.use_grammar(index),
            None => false,
        }
    }

    /// Advance to the next grammar of the site.
    pub fn cycle_grammar(&mut self) {
        let next = (self.grammar + 1) % self.grammars.len();
        self.use_grammar(next);
    }

    /// Monaco language id the active grammar's source document is written in:
    /// `pest` for the line grammar, `json` for the pointer map behind
    /// `trace-json`.
    pub fn grammar_language(&self) -> &'static str {
        self.grammars[self.grammar].language
    }

    /// Read the site in `regime`, transporting the active subobject along the
    /// route of the selected kind: `f*` back towards the native regime, `∃`
    /// out to the next one. Every regime reads every kind, so this always
    /// succeeds.
    pub fn set_regime(&mut self, regime: RegimeId) {
        if regime != self.regime {
            self.transport_to(regime);
            self.regime = regime;
            self.recompute();
        }
    }

    /// The active regime's workspace spec.
    pub fn workspace_spec(&self) -> &WorkspaceSpec {
        &self.regime_state().topos.workspace
    }

    /// The active regime's workspace spec as JSON, for host spec resolvers.
    pub fn workspace_json(&self) -> String {
        self.regime_state().topos.workspace_json()
    }

    /// The external components the active regime's workspace declares.
    pub fn components(&self) -> &[ExternalComponentSpec] {
        &self.regime_state().topos.workspace.components
    }

    /// Resolved content for one binding id: the external graph panel's
    /// placeholder, one of the host overlays, else the active regime's sheaf
    /// over the site, its foreign kinds read through the attribute maps the
    /// route transports.
    ///
    /// [`GRAMMAR`] answers with the active grammar's source document, and a
    /// regime's Morphism binding with the active route's morphism document, so
    /// an editor panel mounts a real document in every regime.
    pub fn resolve(&self, binding: &str) -> Option<&Section> {
        let regime = self.regime_state();
        if binding == regime.id.graph_binding() {
            return Some(&regime.graph);
        }
        match binding {
            MORPHISM => Some(&self.overlays.morphism),
            MORPHISM_AGENTIC | MORPHISM_MEMBRANE | MORPHISM_OKF => {
                Some(&self.overlays.morphism_spec)
            }
            PHYSICS => Some(&self.overlays.physics),
            SELECTED | SELECTED_OKF => Some(&self.overlays.selected),
            GRAMMAR => Some(&self.overlays.grammar),
            other => self.projection().sections.get(other),
        }
    }

    /// The bindings of the active regime whose sheaf cannot read the site, in
    /// binding order. Empty whenever every panel resolves.
    pub fn unreadable(&self) -> Vec<&str> {
        self.projection().unreadable.iter().map(String::as_str).collect()
    }

    /// The selected node id.
    pub fn selected(&self) -> Option<&str> {
        self.selected.as_deref()
    }

    /// Select `node`, or clear the selection when it names no object of the
    /// site.
    pub fn select(&mut self, node: Option<&str>) {
        let next = node.filter(|id| self.object(id).is_some()).map(str::to_owned);
        if next != self.selected {
            self.selected = next;
            self.subobject = self.selected_sorts();
            self.recompute();
        }
    }

    /// Move the selection by `step` objects in site order, wrapping around.
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
        let id = self.site.objects[next].id.clone();
        self.select(Some(&id));
    }

    /// Select the object a badge names.
    ///
    /// A badge's value is its label with the group's count appended, so the
    /// trailing count is stripped first. `kind` (one badge per regime sort)
    /// and `value` (one badge per distinct `tag:0`) walk to the next matching
    /// object after the current selection and wrap around; `object` (one badge
    /// per object) names its object outright. A badge that matches nothing
    /// leaves the selection alone.
    pub fn select_matching(&mut self, field: &str, value: &str) {
        let wanted = strip_count(value);
        let regime = self.regime_state();
        let matches = |object: &Object| -> bool {
            match field {
                "kind" => regime.sort_label(regime.sort_of(&object.kind)) == wanted,
                "value" => object.tags.first() == Some(&wanted),
                "object" => object.title == value || object.title == wanted,
                _ => false,
            }
        };
        let start = self
            .selected
            .as_deref()
            .and_then(|id| self.site.objects.iter().position(|object| object.id == id))
            .map_or(0, |index| index + 1);
        let count = self.site.objects.len();
        let Some(found) = (0..count)
            .map(|step| (start + step) % count)
            .find(|index| matches(&self.site.objects[*index]))
        else {
            return;
        };
        let id = self.site.objects[found].id.clone();
        self.select(Some(&id));
    }

    /// The active subobject: the active regime's sorts covering the selected
    /// object, transported by [`set_regime`](Self::set_regime). Empty when
    /// nothing is selected.
    pub fn active_sorts(&self) -> &BTreeSet<String> {
        &self.subobject
    }

    /// The renderer-neutral graph over the site: objects in site order, sorted
    /// and coloured by the active regime, sized by degree and tuned by the
    /// active regime's `p_R*` of the one base. The tuning is the same in every
    /// regime; only the sort and the colour move.
    pub fn graph(&self) -> GraphModel {
        let site = &self.site;
        let regime = self.regime_state();
        let colors = ThemeTokens::dark().colors;
        let mut degree: BTreeMap<&str, usize> = BTreeMap::new();
        for morphism in &site.morphisms {
            *degree.entry(morphism.domain.as_str()).or_default() += 1;
            *degree.entry(morphism.codomain.as_str()).or_default() += 1;
        }
        let index: BTreeMap<&str, u32> =
            site.objects.iter().enumerate().map(|(at, object)| (object.id.as_str(), at as u32)).collect();

        let nodes = site
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
                    species: tuning.map_or_else(String::new, |tuning| tuning.engine_type.clone()),
                }
            })
            .collect();

        let edges = site
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
        graph_layout_of(&self.regime_state().topos)
    }

    /// One component per connected piece of the site under the active regime's
    /// assembly bonds, with the stage its β₁, χ, and boundary classify. The
    /// bonds are the regime's own kinds, not every kind that sorts into them,
    /// so the agentic and membrane regimes still assemble the sessions into
    /// chains, rings, sheets, tubes, and closed membranes.
    pub fn assembly(&self) -> Vec<ComponentInvariants> {
        let regime = self.regime_state();
        let bonded = self.site.restrict_morphism_kinds(&regime.assembly_kinds);
        assembly(&bonded, &regime.topos.morphism_sorting, &regime.assembly_sorts)
    }

    /// The regime's own words for the stage one component assembled into.
    pub fn stage_label(&self, stage: Stage) -> &str {
        stage_label(&self.regime_state().topos.stalks, stage)
    }

    /// One line of grammar, regime, the routes the site's kinds take, its
    /// size, the selection, and the count of panels that cannot read the site.
    pub fn status_line(&self) -> String {
        let site = &self.site;
        let mut line = format!(
            "grammar {} · {} · {} · {} objects {} morphisms",
            self.grammar_id(),
            self.regime.label(),
            self.route_summary(),
            site.objects.len(),
            site.morphisms.len()
        );
        if let Some(id) = self.selected() {
            line.push_str(" · selected ");
            line.push_str(id);
        }
        let unreadable = self.projection().unreadable.len();
        if unreadable > 0 {
            line.push_str(&format!(" · {unreadable} panels cannot read the site"));
        }
        line
    }

    /// The per-kind attribute maps the active regime reads the site through,
    /// composed from the morphism attribute maps along each route.
    pub fn attributes(&self) -> &AttributeMaps {
        &self.attributes[self.regime.index()]
    }

    fn regime_of(&self, regime: RegimeId) -> &Regime {
        &self.regimes[regime.index()]
    }

    fn regime_state(&self) -> &Regime {
        self.regime_of(self.regime)
    }

    fn projection(&self) -> &Projection {
        &self.projections[self.regime.index()]
    }

    fn morphism(&self, id: MorphismId) -> &GeometricMorphism {
        match id {
            MorphismId::F => &self.f,
            MorphismId::Psi => &self.psi,
            MorphismId::Chi => &self.chi,
            MorphismId::Omega => &self.omega,
        }
    }

    /// The shortest morphism path from `from` to `to`, breadth-first over the
    /// four documents, so a kind reaches every regime by one or two morphisms.
    fn path(&self, from: RegimeId, to: RegimeId) -> Vec<MorphismId> {
        if from == to {
            return Vec::new();
        }
        let mut queue: VecDeque<(RegimeId, Vec<MorphismId>)> = VecDeque::from([(from, Vec::new())]);
        let mut seen = BTreeSet::from([from]);
        while let Some((here, steps)) = queue.pop_front() {
            for id in MorphismId::ALL {
                let (domain, codomain) = (self.morphism(id).domain.as_str(), self.morphism(id).codomain.as_str());
                if domain != here.topos_id() {
                    continue;
                }
                let next = RegimeId::ALL
                    .into_iter()
                    .find(|regime| regime.topos_id() == codomain)
                    .unwrap_or(here);
                if !seen.insert(next) {
                    continue;
                }
                let mut path = steps.clone();
                path.push(id);
                if next == to {
                    return path;
                }
                queue.push_back((next, path));
            }
        }
        Vec::new()
    }

    fn object(&self, id: &str) -> Option<&Object> {
        self.site.objects.iter().find(|object| object.id == id)
    }

    /// The active regime's reading of the selected object's sorts.
    fn selected_sorts(&self) -> BTreeSet<String> {
        self.selected
            .as_deref()
            .and_then(|id| self.object(id))
            .map(|object| BTreeSet::from([self.regime_state().sort_of(&object.kind).to_owned()]))
            .unwrap_or_default()
    }

    /// Re-read the site under another of its grammars.
    fn use_grammar(&mut self, grammar: usize) -> bool {
        if grammar == self.grammar {
            return true;
        }
        let Ok(parsed) = parse_site(&self.grammars[grammar]) else {
            return false;
        };
        self.site = parsed.site;
        self.grammar = grammar;
        self.unresolved_links = parsed.unresolved_links;
        if self.selected.as_ref().is_some_and(|id| self.object(id).is_none()) {
            self.selected = None;
        }
        self.subobject = self.selected_sorts();
        self.recompute();
        true
    }

    /// Carry the active subobject to `next`: `f*` back to the selected kind's
    /// native regime, then `∃` out along the route to `next`. Both legs are the
    /// route's own morphisms, so the diagram's commutation is what makes them
    /// agree.
    fn transport_to(&mut self, next: RegimeId) {
        let kind = self.selected.as_deref().and_then(|id| self.object(id)).map(|object| object.kind.clone());
        let Some(kind) = kind else {
            return;
        };
        let route = self.route_to(&kind, self.regime);
        let mut sorts = self.subobject.clone();
        if self.regime != route.native {
            for step in route.path.iter().rev() {
                sorts = self.morphism(*step).pullback_subobject(&sorts);
            }
        }
        let outgoing = self.route_to(&kind, next);
        if next != outgoing.native {
            for step in &outgoing.path {
                sorts = self.morphism(*step).image(&sorts);
            }
        }
        self.subobject = sorts;
    }

    fn recompute(&mut self) {
        self.overlays.selected = Section::Text(TextModel { text: self.selected_text() });
        self.overlays.morphism = Section::Text(TextModel { text: self.morphism_text() });
        self.overlays.morphism_spec = Section::Text(TextModel { text: self.morphism_spec_text() });
        self.overlays.grammar =
            Section::Text(TextModel { text: self.grammars[self.grammar].source.clone() });
        self.overlays.physics = self.physics_section();
    }

    /// The selected object's attributes as the active regime reads them — the
    /// transported field values, not the site's own — then its neighbours
    /// under the active regime's morphism sorts.
    fn selected_text(&self) -> String {
        let site = &self.site;
        let regime = self.regime_state();
        let mut lines = vec!["Selected object".to_owned()];
        let Some(id) = self.selected.as_deref() else {
            lines.push("nothing selected: press n or p, or click a node.".to_owned());
            return lines.join("\n");
        };
        let Some(object) = self.object(id) else {
            lines.push(format!("{id} is not an object of the site"));
            return lines.join("\n");
        };
        let attributes = self.attributes();
        lines.push(format!("{}  [{} · {}]", object.id, object.kind, regime.sort_of(&object.kind)));
        lines.push(format!("title: {}", object.title));
        for (key, value) in &object.fields {
            lines.push(format!("{key}: {value}"));
        }
        let transported: Vec<String> = attributes
            .get(&object.kind)
            .map(|map| map.iter().map(|(read, from)| format!("{read} <- {from}")).collect())
            .unwrap_or_default();
        if !transported.is_empty() {
            lines.push(format!(
                "read in {} through {}: {}",
                regime.id.label(),
                self.route(&object.kind).text(),
                transported.join(", ")
            ));
        }
        if !object.tags.is_empty() {
            lines.push(format!("tags: {}", object.tags.join(", ")));
        }
        let neighbours: Vec<&Morphism> = site
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
        lines.push(format!("active sorts: {}", sort_names(&self.subobject)));
        lines.join("\n")
    }

    /// The routes the site's kinds take into the active regime, φ per kind with
    /// the object counts read off the site, the attribute maps those routes
    /// transport, and the two diagram laws.
    fn morphism_text(&self) -> String {
        let regime = self.regime_state();
        let mut lines = vec![
            format!(
                "one site · {} objects {} morphisms · read as {}",
                self.site.objects.len(),
                self.site.morphisms.len(),
                regime.id.label()
            ),
            format!("routes: {}", self.route_summary()),
            String::new(),
            format!("site kind -> {} sort (objects):", regime.id.label()),
        ];
        for (kind, native, sort, objects) in self.fibres() {
            let route = self.route_to(&kind, self.regime);
            lines.push(format!(
                "  {kind} [native {}] -> {} : {objects} objects{}",
                self.regime_of(native).sort_label(&kind),
                regime.sort_label(&sort),
                if route.is_native() { String::new() } else { format!(" ({})", path_text(&route.path)) }
            ));
        }
        lines.push(String::new());
        lines.push(format!(
            "triangle: f ∘ χ = ψ — {} is {}",
            MorphismId::Psi.name(),
            path_text(&[MorphismId::Chi, MorphismId::F])
        ));
        lines.push(format!(
            "square: ψ ∘ ω = f — {} is {}",
            MorphismId::F.name(),
            path_text(&[MorphismId::Omega, MorphismId::Psi])
        ));
        lines.push(String::new());
        lines.push("attributes transported along the routes:".to_owned());
        for id in MorphismId::ALL {
            let morphism = self.morphism(id);
            if morphism.attributes.is_empty() {
                lines.push(format!("  {}: none (every field crosses unchanged)", id.name()));
                continue;
            }
            let pairs: Vec<String> = morphism
                .attributes
                .iter()
                .map(|(read, from)| format!("{read} <- {from}"))
                .collect();
            lines.push(format!("  {}: {}", id.name(), pairs.join(", ")));
        }
        lines.join("\n")
    }

    /// The active route's morphism document, indented JSON: the morphism the
    /// selected kind takes into the active regime, or the first morphism whose
    /// codomain is the active regime when nothing is selected.
    fn morphism_spec_text(&self) -> String {
        let id = self
            .selected
            .as_deref()
            .and_then(|id| self.object(id))
            .and_then(|object| self.route(&object.kind).path.last().copied())
            .unwrap_or_else(|| {
                MorphismId::ALL
                    .into_iter()
                    .find(|id| self.morphism(*id).codomain == self.regime.topos_id())
                    .unwrap_or(MorphismId::F)
            });
        self.morphism(id).to_json_pretty()
    }

    /// The tuning the active regime pulls back from the base, one row per sort,
    /// and the law that makes it the same in every regime.
    fn physics_section(&self) -> Section {
        let regime = self.regime_state();
        let columns = vec![
            column("part", "Part", TextAlign::Left, ColumnWidth::Flex { weight: 1 }),
            column("sort", "Sort", TextAlign::Left, ColumnWidth::Flex { weight: 2 }),
            column("engine", "Engine", TextAlign::Left, ColumnWidth::Flex { weight: 2 }),
            column("tuning", "Tuning", TextAlign::Left, ColumnWidth::Flex { weight: 3 }),
        ];
        let mut rows = regime
            .topos
            .stalks
            .objects
            .keys()
            .map(|sort| {
                let tuning = regime.physics.object(sort);
                row(vec![
                    "object".to_owned(),
                    regime.sort_label(sort),
                    tuning.map_or("—".to_owned(), |tuning| tuning.engine_type.clone()),
                    tuning.map_or_else(
                        || "—".to_owned(),
                        |tuning| {
                            format!("repulsion {}, mass {}", num(tuning.repulsion), num(tuning.mass))
                        },
                    ),
                ])
            })
            .collect::<Vec<_>>();
        rows.extend(regime.topos.stalks.morphisms.keys().map(|sort| {
            let tuning = regime.physics.morphism(sort);
            row(vec![
                "morphism".to_owned(),
                regime.topos.stalks.morphisms.get(sort).map_or_else(
                    || sort.to_owned(),
                    |stalk| stalk.label.clone(),
                ),
                tuning.map_or("—".to_owned(), |tuning| tuning.engine_type.clone()),
                tuning.map_or_else(
                    || "—".to_owned(),
                    |tuning| {
                        format!("weight {}, rest {}", num(tuning.weight), num(tuning.rest_length))
                    },
                ),
            ])
        }));
        rows.push(row(vec![
            "law".to_owned(),
            "one physics section".to_owned(),
            "topos-physics".to_owned(),
            "every path from a kind's native regime to the base agrees on its numbers"
                .to_owned(),
        ]));
        Section::Table(TableModel { columns, rows })
    }

    /// Every site kind with the regime that sorts it natively, the sort the
    /// active regime reads it as, and how many objects it covers.
    fn fibres(&self) -> Vec<(String, RegimeId, String, usize)> {
        let regime = self.regime_state();
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        let mut rows = Vec::new();
        for object in &self.site.objects {
            if !seen.insert(object.kind.as_str()) {
                continue;
            }
            let objects = self
                .site
                .objects
                .iter()
                .filter(|other| other.kind == object.kind)
                .count();
            rows.push((
                object.kind.clone(),
                self.native_regime(&object.kind),
                regime.sort_of(&object.kind).to_owned(),
                objects,
            ));
        }
        rows
    }
}

/// One site as a grammar reads it.
struct ParsedSite {
    site: Site,
    unresolved_links: usize,
}

/// Parse the site under one grammar.
fn parse_site(entry: &GrammarEntry) -> Result<ParsedSite, String> {
    let report = entry
        .grammar
        .parse_reported(&entry.data)
        .map_err(|error| format!("{} data: {error}", entry.id))?;
    Ok(ParsedSite { site: report.site, unresolved_links: report.unresolved_links })
}

/// Every site object kind.
fn site_kind_set(site: &Site) -> BTreeSet<String> {
    site.objects.iter().map(|object| object.kind.clone()).collect()
}

/// Every site morphism kind.
fn site_morphism_kind_set(site: &Site) -> BTreeSet<String> {
    site.morphisms
        .iter()
        .filter_map(|morphism| morphism.kind.clone())
        .collect()
}

/// The regime that sorts `kind` natively, given the three regimes in
/// `RegimeId::ALL` order.
fn native_regime_of(regimes: &[&Regime; 3], kind: &str) -> RegimeId {
    RegimeId::ALL
        .into_iter()
        .find(|regime| regimes[regime.index()].topos.native_object_kinds().contains(kind))
        .unwrap_or(RegimeId::Agentic)
}

/// The regime that sorts the site morphism kind `kind` natively.
fn native_morphism_regime_of(regimes: &[&Regime; 3], kind: &str) -> RegimeId {
    RegimeId::ALL
        .into_iter()
        .find(|regime| regimes[regime.index()].topos.native_morphism_kinds().contains(kind))
        .unwrap_or(RegimeId::Agentic)
}

/// The path from `from` to `to` over the four morphism documents.
fn path_between(
    morphisms: [&GeometricMorphism; 4],
    from: RegimeId,
    to: RegimeId,
) -> Vec<MorphismId> {
    if from == to {
        return Vec::new();
    }
    let mut queue: VecDeque<(RegimeId, Vec<MorphismId>)> = VecDeque::from([(from, Vec::new())]);
    let mut seen = BTreeSet::from([from]);
    while let Some((here, steps)) = queue.pop_front() {
        for (index, morphism) in morphisms.iter().enumerate() {
            if morphism.domain != here.topos_id() {
                continue;
            }
            let Some(next) =
                RegimeId::ALL.into_iter().find(|regime| regime.topos_id() == morphism.codomain)
            else {
                continue;
            };
            if !seen.insert(next) {
                continue;
            }
            let mut path = steps.clone();
            path.push(MorphismId::ALL[index]);
            if next == to {
                return path;
            }
            queue.push_back((next, path));
        }
    }
    Vec::new()
}

/// The regime field each site kind's field is read from in `regime`: the
/// attribute maps of the route from the kind's native regime, composed, and
/// the trivial identity everywhere else.
fn attribute_maps(
    reader: &Regime,
    regimes: [&Regime; 3],
    morphisms: [&GeometricMorphism; 4],
    kinds: BTreeSet<String>,
) -> AttributeMaps {
    let morphisms_ref = [morphisms[0], morphisms[1], morphisms[2], morphisms[3]];
    let mut maps = AttributeMaps::new();
    for kind in kinds {
        let native = native_regime_of(&regimes, &kind);
        if native == reader.id {
            continue;
        }
        let path = path_between(morphisms_ref, native, reader.id);
        let mut map: BTreeMap<String, String> = BTreeMap::new();
        // Every field either leg names is transported; the rest cross unchanged.
        let mut fields: BTreeSet<String> = BTreeSet::new();
        for step in &path {
            fields.extend(morphisms_ref[index_of(*step)].attributes.keys().cloned());
        }
        for field in fields {
            let mut name = field.clone();
            for step in &path {
                name = morphisms_ref[index_of(*step)].attribute(&name).to_owned();
            }
            if name != field {
                map.insert(field.to_owned(), name);
            }
        }
        if !map.is_empty() {
            maps.insert(kind, map);
        }
    }
    maps
}

/// `MorphismId::ALL` index of `id`.
fn index_of(id: MorphismId) -> usize {
    MorphismId::ALL.iter().position(|candidate| *candidate == id).unwrap_or(0)
}

/// One law of the diagram: `composed` (the two legs named by `name`) must be
/// `target` on every object and every morphism sort, over the same sorts.
fn check_diagram(
    composed: &GeometricMorphism,
    target: &GeometricMorphism,
    target_id: &MorphismId,
    name: &str,
    legs: &str,
) -> Result<(), String> {
    if let Some((sort, image)) = composed
        .object_sorts
        .iter()
        .find(|(sort, image)| target.object_sorts.get(*sort) != Some(*image))
    {
        return Err(format!(
            "{name} fails on the object sort {sort}: {legs} sends it to {image}, {} sends it to {}",
            target_id.name(),
            image_of(&target.object_sorts, sort)
        ));
    }
    if let Some((sort, image)) = composed
        .morphism_sorts
        .iter()
        .find(|(sort, image)| target.morphism_sorts.get(*sort) != Some(*image))
    {
        return Err(format!(
            "{name} fails on the morphism sort {sort}: {legs} sends it to {image}, {} sends it to {}",
            target_id.name(),
            image_of(&target.morphism_sorts, sort)
        ));
    }
    if composed.object_sorts != target.object_sorts || composed.morphism_sorts != target.morphism_sorts {
        return Err(format!(
            "{name} fails: {legs} sorts {} objects and {} morphisms, {} sorts {} and {}",
            composed.object_sorts.len(),
            composed.morphism_sorts.len(),
            target_id.name(),
            target.object_sorts.len(),
            target.morphism_sorts.len()
        ));
    }
    Ok(())
}

/// The physics pulled back from the base along every path from a kind's native
/// regime must agree per kind: whichever way the kind is read, the base engine
/// and its numbers are the same.
fn check_physics_per_kind(
    base: &BaseTopos,
    regimes: &[Regime; 3],
    site: &Site,
    morphisms: [&GeometricMorphism; 4],
) -> Result<(), String> {
    let seen: [&Regime; 3] = [&regimes[0], &regimes[1], &regimes[2]];
    for kind in site_kind_set(site) {
        let native = native_regime_of(&seen, &kind);
        let consensus = engine_object(base, regimes, morphisms, native, native, &kind);
        for regime in regimes.iter() {
            let engine = engine_object(base, regimes, morphisms, native, regime.id, &kind);
            if engine != consensus {
                return Err(format!(
                    "the object physics of the kind {kind} pulled back from {} to the base along \
                     {} disagrees with the path through {}: {engine:?} against {consensus:?}",
                    native.label(),
                    regime.id.label(),
                    native.label(),
                ));
            }
        }
    }
    for kind in site_morphism_kind_set(site) {
        let native = native_morphism_regime_of(&seen, &kind);
        let consensus = engine_morphism(base, regimes, morphisms, native, native, &kind);
        for regime in regimes.iter() {
            let engine = engine_morphism(base, regimes, morphisms, native, regime.id, &kind);
            if engine != consensus {
                return Err(format!(
                    "the morphism physics of the kind {kind} pulled back from {} to the base along \
                     {} disagrees with the path through {}: {engine:?} against {consensus:?}",
                    native.label(),
                    regime.id.label(),
                    native.label(),
                ));
            }
        }
    }
    Ok(())
}

/// The base engine sort a site object kind reaches when it is sorted by the
/// route from `native` to `reader` and pulled back through that reader's `p_R*`.
fn engine_object(
    base: &BaseTopos,
    regimes: &[Regime; 3],
    morphisms: [&GeometricMorphism; 4],
    native: RegimeId,
    reader: RegimeId,
    kind: &str,
) -> Option<String> {
    let path = path_between(morphisms, native, reader);
    let mut sort = kind.to_owned();
    for step in &path {
        sort = morphisms[index_of(*step)].object_sorts.get(&sort)?.clone();
    }
    let engine = regimes[reader.index()].engine_object(&sort)?.to_owned();
    let stalk = base.stalks.objects.get(&engine)?;
    Some(format!("{engine} repulsion {} mass {}", num(stalk.repulsion), num(stalk.mass)))
}

/// The base engine sort a site morphism kind reaches, read as [`engine_object`].
fn engine_morphism(
    base: &BaseTopos,
    regimes: &[Regime; 3],
    morphisms: [&GeometricMorphism; 4],
    native: RegimeId,
    reader: RegimeId,
    kind: &str,
) -> Option<String> {
    let path = path_between(morphisms, native, reader);
    let mut sort = kind.to_owned();
    for step in &path {
        sort = morphisms[index_of(*step)].morphism_sorts.get(&sort)?.clone();
    }
    let engine = regimes[reader.index()].engine_morphism(&sort)?.to_owned();
    let stalk = base.stalks.morphisms.get(&engine)?;
    Some(format!("{engine} weight {} rest {}", num(stalk.weight), num(stalk.rest_length)))
}

/// The regime's resolved panels over the site, taken as one set where it can be
/// and sheaf by sheaf where it cannot: `Topos::global_sections_over` is
/// all-or-nothing, and a sheaf can still ask for a field the site does not
/// carry.
fn project(regime: &Regime, site: &Site, attributes: &AttributeMaps) -> Projection {
    if let Ok(all) = regime.topos.global_sections_over(site, &regime.physics, attributes) {
        return Projection {
            sections: all.iter().map(|(binding, section)| (binding.clone(), section.clone())).collect(),
            unreadable: BTreeSet::new(),
        };
    }
    let mut projection = Projection::default();
    for (binding, sheaf) in &regime.topos.sheaves {
        let mut alone = regime.topos.clone();
        alone.sheaves = BTreeMap::from([(binding.clone(), sheaf.clone())]);
        let outcome = alone
            .global_sections_over(site, &regime.physics, attributes)
            .map_err(|error| error.to_string())
            .and_then(|sections| {
                sections
                    .get(binding)
                    .cloned()
                    .ok_or_else(|| format!("{binding} resolved to no content"))
            });
        match outcome {
            Ok(section) => {
                projection.sections.insert(binding.clone(), section);
            }
            Err(error) => {
                projection.unreadable.insert(binding.clone());
                projection.sections.insert(
                    binding.clone(),
                    Section::Text(TextModel { text: unreadable_text(regime, binding, &error) }),
                );
            }
        }
    }
    projection
}

/// What a panel says when the regime's sheaf cannot read the site.
fn unreadable_text(regime: &Regime, binding: &str, error: &str) -> String {
    format!(
        "{binding} does not read the site.\n{} asks for a field the site does not carry under \
         any morphism's attribute map.\nsheaf: {error}\nNo other panel is affected.",
        regime.topos.id,
    )
}

/// The bonds one regime assembles over, and the site morphism kinds they are
/// drawn from: the structural ones, not the ambient bond that ties every object
/// to every other (the agentic `consolidates`, the membrane `solvation`, the
/// catalog's `contains`), so the components counted here are the ones that
/// actually have a stage.
fn assembly_sorts(id: RegimeId) -> BTreeSet<String> {
    let sorts: &[&str] = match id {
        RegimeId::Agentic => &["continues", "shares"],
        RegimeId::Membrane => &["tail_bond", "lateral"],
        RegimeId::OkfCatalog => {
            &["derived_from", "computes", "executed_by", "attested_by", "references"]
        }
    };
    sorts.iter().map(|sort| (*sort).to_owned()).collect()
}

/// The site morphism kinds one regime's assembly bonds are drawn from.
fn assembly_kinds(id: RegimeId) -> BTreeSet<String> {
    let kinds: &[&str] = match id {
        RegimeId::Agentic | RegimeId::Membrane => &["continues", "shares"],
        RegimeId::OkfCatalog => &[
            "derived_from",
            "computes",
            "executed_by",
            "attested_by",
            "references",
        ],
    };
    kinds.iter().map(|kind| (*kind).to_owned()).collect()
}

/// The external graph panel's layout request, or the default when the workspace
/// declares no external panel.
fn graph_layout_of(topos: &Topos) -> GraphLayout {
    topos
        .workspace
        .panels
        .iter()
        .find_map(|panel| match &panel.content {
            ContentSpec::External { layout, .. } => Some(layout.clone()),
            _ => None,
        })
        .unwrap_or_default()
}

/// A regime's own words for one assembly stage.
fn stage_label(stalks: &Stalks, stage: Stage) -> &str {
    match stage {
        Stage::Chain => &stalks.stages.chain,
        Stage::Branched => &stalks.stages.branched,
        Stage::Ring => &stalks.stages.ring,
        Stage::Sheet => &stalks.stages.sheet,
        Stage::Tube => &stalks.stages.tube,
        Stage::Membrane => &stalks.stages.membrane,
    }
}

/// The palette slot a regime sort wears. Looked up by sort name, so each regime
/// paints its own sorts and its colouring is stable across switches.
fn color_of(colors: &ColorTokens, sort: &str) -> Color {
    match sort {
        // topos-agentic: sessions and memories.
        "planner" => colors.primary,
        "coder" => colors.success,
        "reviewer" => colors.warning,
        "memory" => colors.text_muted,
        // topos-membrane: the lipids those sessions read as.
        "head" => colors.highlight,
        "tail" => colors.failure,
        "solvent" => colors.info,
        // topos-okf-catalog: the normalised OKF types.
        "bundle" => colors.text_muted,
        "table" => colors.primary,
        "dataset" => colors.success,
        "policy" => colors.warning,
        "metric" => colors.tertiary,
        "computation" => colors.failure,
        "skill" => colors.highlight,
        "reference" => colors.info,
        "concept" => colors.border,
        "log" => colors.divider,
        _ => colors.text,
    }
}

fn rgb(color: Color) -> Rgb {
    (color.r, color.g, color.b)
}

/// Drawn radius in renderer units: incident morphisms, compressed.
fn radius(degree: usize) -> f32 {
    (2.0 + (degree as f32).sqrt() * 1.6).min(14.0)
}

/// A sort set as `{a, b}`, or `∅` when empty.
fn sort_names(sorts: &BTreeSet<String>) -> String {
    if sorts.is_empty() {
        "∅".to_owned()
    } else {
        format!("{{{}}}", sorts.iter().cloned().collect::<Vec<_>>().join(", "))
    }
}

/// A badge value without the group's trailing count: `Planner 12` is `Planner`,
/// and a label with no count is itself.
fn strip_count(value: &str) -> String {
    match value.rsplit_once(' ') {
        Some((label, count)) if count.parse::<usize>().is_ok() => label.to_owned(),
        _ => value.to_owned(),
    }
}

/// The solver's spec name, as `LayoutAlgorithm` serialises it.
fn solver_name(algorithm: &LayoutAlgorithm) -> String {
    format!("{algorithm:?}").to_lowercase()
}

/// An f64 as the shortest decimal that reads back exactly.
fn num(value: f64) -> String {
    let text = format!("{value}");
    text.strip_suffix(".0").unwrap_or(&text).to_owned()
}

/// Where `sort` lands, or `nothing` when the map does not cover it.
fn image_of(sorts: &BTreeMap<String, String>, sort: &str) -> String {
    sorts.get(sort).cloned().unwrap_or_else(|| String::from("nothing"))
}

fn column(key: &str, title: &str, align: TextAlign, width: ColumnWidth) -> TableColumn {
    TableColumn { key: key.to_owned(), title: title.to_owned(), width, align, kind: ColumnKind::Text }
}

fn row(cells: Vec<String>) -> TableRow {
    TableRow {
        id: format!("row:{}", cells.join("|")),
        tags: Vec::new(),
        cells: cells.into_iter().map(TableCell::Text).collect(),
    }
}