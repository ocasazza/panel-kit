//! Host façade over `panel_kit_grammar` for the trio demo: OKF is the third
//! regime beside the two agent-trace regimes.
//!
//! Two sites — the agent-trace site, read by `trace-lines` or by the
//! equivalent `trace-json`, and the OKF knowledge-catalog site, read by `okf` —
//! and three regimes related by three geometric morphisms: `f: agentic →
//! membrane`, `ψ: okf-catalog → membrane`, and `χ: okf-catalog → agentic`.
//! [`TrioHost::load`] checks every topos, checks each morphism against its
//! domain and codomain, checks that each commutes over the one physics base,
//! and checks the triangle `f ∘ χ = ψ`, so a host that loads is a host whose
//! physics is one pullback and whose presentation may move along the triangle
//! without touching the assembly.
//!
//! A regime reads a site when the site's native regime has a morphism into it:
//! the trace site is native in agentic and reaches membrane along `f`; the OKF
//! site is native in okf-catalog and reaches agentic along `χ` and membrane
//! along `ψ`, which the triangle makes the same map as `χ` then `f`. The trace
//! site in okf-catalog is refused by name — no morphism `agentic → okf-catalog`
//! exists, and the host says so instead of inventing one.
//!
//! Physics is the invariant. For one site, [`TrioHost::graph`] reads the same
//! mass, repulsion, weight, and rest length in every regime that can read the
//! site, because each regime's tuning is `p_R*` of the one base and the three
//! morphisms commute over it. Only the sort and the colour move. The panels are
//! the active regime's sheaves over the active site plus three host overlays:
//! the route with φ's fibres ([`MORPHISM`]), the pulled-back tuning
//! ([`PHYSICS`]), and the selected object ([`SELECTED`], which the OKF catalog
//! workspace's own `okf.selected` panel also mounts).
//!
//! Each regime's sheaves are authored over one site: the agentic and membrane
//! panels read the trace fields `latency_ms` and `energy`, which the OKF corpus
//! does not carry. So a foreign (site, regime) pair is projected sheaf by
//! sheaf, and a panel that cannot read the site names its own error while every
//! panel that can still reads it.
//!
//! Only `std`, `panel_kit_core`, and `panel_kit_grammar` are used: this file is
//! `#[path]`-included by the root-crate and `panel-kit-tui` examples and by the
//! host-contract test.

use std::collections::{BTreeMap, BTreeSet};

use panel_kit_core::badge::Rgb;
use panel_kit_core::spec::{ExternalComponentSpec, WorkspaceSpec};
use panel_kit_core::theme::{Color, ColorTokens, ThemeTokens};
use panel_kit_core::widgets::graph::{GraphEdge, GraphLayout, GraphModel, GraphNode, LayoutAlgorithm};
use panel_kit_core::widgets::table::{ColumnWidth, TableCell, TableColumn, TableModel, TableRow, TextAlign};
use panel_kit_core::widgets::{ContentSpec, TextModel};
use panel_kit_grammar::{
    assembly, BaseTopos, CompiledGrammar, ComponentInvariants, GeometricMorphism, GrammarPackage,
    Morphism, Object, Physics, Section, Site, Stage, Stalks, Topos,
};

/// Agentic-trace regime topos id.
pub const TOPOS_AGENTIC: &str = "topos-agentic";
/// Lipid self-assembly regime topos id.
pub const TOPOS_MEMBRANE: &str = "topos-membrane";
/// OKF catalog regime topos id.
pub const TOPOS_OKF_CATALOG: &str = "topos-okf-catalog";
/// Line-format agent-trace grammar id.
pub const GRAMMAR_LINES: &str = "trace-lines";
/// JSON-format agent-trace grammar id.
pub const GRAMMAR_JSON: &str = "trace-json";
/// OKF grammar id.
pub const GRAMMAR_OKF: &str = "okf";

/// The host overlay stating the route taken and φ against the active site.
pub const MORPHISM: &str = "trio.morphism";
/// The host overlay stating the tuning the active regime pulls back.
pub const PHYSICS: &str = "trio.physics";
/// The host overlay listing the selected object.
pub const SELECTED: &str = "trio.selected";
/// The OKF catalog workspace's own Selected panel; it mounts [`SELECTED`].
pub const SELECTED_OKF: &str = "okf.selected";

/// Why the trace site is not offered in the OKF catalog regime.
pub const NO_MORPHISM_TO_CATALOG: &str = "no morphism agentic → okf-catalog";

/// The Nix-authored JSON a trio host is built from: three regime topoi, the one
/// physics base, the three morphisms between the regimes, the three morphisms
/// from a regime into the base, and both sites' grammars and data.
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
    /// p_agentic into the base.
    pub agentic_physics: &'a str,
    /// p_membrane into the base.
    pub membrane_physics: &'a str,
    /// p_catalog into the base.
    pub catalog_physics: &'a str,
    /// The `trace-lines` grammar package.
    pub trace_lines_grammar: &'a str,
    /// The line-format agent-trace data.
    pub trace_lines_data: &'a str,
    /// The `trace-json` grammar package.
    pub trace_json_grammar: &'a str,
    /// The JSON-format agent-trace data.
    pub trace_json_data: &'a str,
    /// The `okf` grammar package.
    pub okf_grammar: &'a str,
    /// The knowledge-catalog envelope.
    pub okf_data: &'a str,
}

/// One of the two sites a regime may read.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SiteId {
    /// The agent-trace site, read by `trace-lines` or by `trace-json`.
    Traces,
    /// The OKF knowledge-catalog site, read by `okf`.
    Okf,
}

impl SiteId {
    /// Every site, in header order.
    pub const ALL: [SiteId; 2] = [SiteId::Traces, SiteId::Okf];

    /// The regime whose sortings read this site natively.
    pub const fn native_regime(self) -> RegimeId {
        match self {
            SiteId::Traces => RegimeId::Agentic,
            SiteId::Okf => RegimeId::OkfCatalog,
        }
    }

    /// The grammar ids that read this site, in cycle order.
    pub const fn grammar_ids(self) -> &'static [&'static str] {
        match self {
            SiteId::Traces => &[GRAMMAR_LINES, GRAMMAR_JSON],
            SiteId::Okf => &[GRAMMAR_OKF],
        }
    }

    /// Display label.
    pub const fn label(self) -> &'static str {
        match self {
            SiteId::Traces => "traces",
            SiteId::Okf => "OKF",
        }
    }

    const fn index(self) -> usize {
        self as usize
    }
}

/// One of the three regimes a site may be read in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RegimeId {
    /// Planner, coder, reviewer sessions and memory stores.
    Agentic,
    /// Lipids: heads, tails, solvent.
    Membrane,
    /// The normalised OKF types.
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

impl std::fmt::Display for SiteId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.label())
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


/// One of the trio's three morphisms between regimes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MorphismId {
    /// f: `topos-agentic` → `topos-membrane`.
    F,
    /// ψ: `topos-okf-catalog` → `topos-membrane`.
    Psi,
    /// χ: `topos-okf-catalog` → `topos-agentic`.
    Chi,
}

impl MorphismId {
    /// Every morphism, in the order the triangle names them.
    pub const ALL: [MorphismId; 3] = [MorphismId::F, MorphismId::Psi, MorphismId::Chi];

    /// The symbol the specs and panels use.
    pub const fn symbol(self) -> &'static str {
        match self {
            MorphismId::F => "f",
            MorphismId::Psi => "ψ",
            MorphismId::Chi => "χ",
        }
    }

    /// The morphism's own name: symbol, domain, codomain.
    pub const fn name(self) -> &'static str {
        match self {
            MorphismId::F => "f: topos-agentic -> topos-membrane",
            MorphismId::Psi => "ψ: topos-okf-catalog -> topos-membrane",
            MorphismId::Chi => "χ: topos-okf-catalog -> topos-agentic",
        }
    }
}

/// How a site's native regime reaches the regime reading it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Route {
    /// The site's native regime; every route starts there.
    pub native: RegimeId,
    /// The regime the site is read in.
    pub regime: RegimeId,
    /// The morphisms taken from the native regime, in application order.
    pub path: Vec<MorphismId>,
    /// The composite the commuting triangle makes the same map, when the route
    /// admits one: `χ` then `f` lands exactly where `ψ` does.
    pub composite: Option<Vec<MorphismId>>,
}

impl Route {
    /// Whether the site is read in its own native regime.
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

/// One site: the parsed objects, the grammar that read them, and each regime's
/// resolved panels over them.
struct SiteData {
    id: SiteId,
    grammar: usize,
    site: Site,
    unresolved_links: usize,
    projections: [Projection; 3],
}

impl SiteData {
    /// Parse the grammar's data and project every regime's sheaves over it.
    fn parse(
        id: SiteId,
        grammars: &[GrammarEntry],
        grammar: usize,
        regimes: &[Regime; 3],
    ) -> Result<Self, String> {
        let entry = grammars.get(grammar).ok_or_else(|| format!("{} has no grammar {grammar}", id.label()))?;
        let report = entry
            .grammar
            .parse_reported(&entry.data)
            .map_err(|error| format!("{} data: {error}", entry.id))?;
        let projections = [
            project(id, &regimes[0], &report.site),
            project(id, &regimes[1], &report.site),
            project(id, &regimes[2], &report.site),
        ];
        Ok(SiteData {
            id,
            grammar,
            site: report.site,
            unresolved_links: report.unresolved_links,
            projections,
        })
    }

    /// Re-parse under another grammar, re-projecting every regime over it.
    fn reparse(
        &mut self,
        grammars: &[GrammarEntry],
        grammar: usize,
        regimes: &[Regime; 3],
    ) -> Result<(), String> {
        *self = Self::parse(self.id, grammars, grammar, regimes)?;
        Ok(())
    }
}

/// One regime's resolved panels over one site.
#[derive(Default)]
struct Projection {
    sections: BTreeMap<String, Section>,
    unreadable: BTreeSet<String>,
}

/// One grammar: its compiled parser and the data it is read against.
struct GrammarEntry {
    id: &'static str,
    grammar: CompiledGrammar,
    data: String,
}

impl GrammarEntry {
    fn compile(id: &'static str, package: &str, data: &str) -> Result<Self, String> {
        let grammar = GrammarPackage::from_json_str(package)
            .map_err(|error| format!("{id}: {error}"))?
            .compile()
            .map_err(|error| format!("{id}: {error}"))?;
        if grammar.id() != id {
            return Err(format!("{id}: the package compiles the grammar {}", grammar.id()));
        }
        Ok(GrammarEntry { id, grammar, data: data.to_owned() })
    }
}

/// One regime: its validated spec, its `p_R*` tuning, the placeholder its
/// external graph panel mounts, and the bonds its assembly counts.
struct Regime {
    topos: Topos,
    physics: Physics,
    graph: Section,
    assembly_sorts: BTreeSet<String>,
}

impl Regime {
    /// Decode, validate, and pull the tuning back from the one base.
    fn load(
        base: &BaseTopos,
        topos: &str,
        p: &GeometricMorphism,
        id: RegimeId,
    ) -> Result<Self, String> {
        let topos = Topos::from_json_str(topos).map_err(|error| error.to_string())?;
        topos.validate().map_err(|error| format!("{}: {error}", topos.id))?;
        let physics =
            Physics::new(base, p, &topos).map_err(|error| format!("{}: {error}", topos.id))?;
        let graph = Section::Text(TextModel {
            text: format!(
                "{} mounts the host's {} model.\nThe body is TrioHost::graph(): the site's objects \
                 in site order, sorted and coloured by {}, tuned by {}.\nSolved with the {} layout.",
                topos.id,
                id.graph_binding(),
                id.label(),
                topos.id,
                solver_name(&graph_layout_of(&topos).algorithm),
            ),
        });
        Ok(Regime { topos, physics, graph, assembly_sorts: assembly_sorts(id) })
    }

    /// The regime object sort of `kind`, falling back to the site kind.
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
}

/// The three host overlays, rebuilt on every selection or regime change.
struct Overlays {
    selected: Section,
    morphism: Section,
    physics: Section,
}

impl Overlays {
    fn empty() -> Self {
        let blank = || Section::Text(TextModel { text: String::new() });
        Self { selected: blank(), morphism: blank(), physics: blank() }
    }
}

/// Two sites and three regimes under the three morphisms that relate them,
/// with every panel of every available (site, regime) pair already resolved.
pub struct TrioHost {
    regimes: [Regime; 3],
    f: GeometricMorphism,
    psi: GeometricMorphism,
    chi: GeometricMorphism,
    trace_grammars: [GrammarEntry; 2],
    okf_grammar: GrammarEntry,
    sites: [SiteData; 2],
    site: SiteId,
    regime: RegimeId,
    selected: Option<String>,
    subobject: BTreeSet<String>,
    overlays: Overlays,
}

impl TrioHost {
    /// Compile both sites' grammars, decode and validate the base, the three
    /// regimes and the six morphisms into the base, check `f`, `ψ`, and `χ`
    /// against their domain and codomain, check the triangle `f ∘ χ = ψ`, check
    /// that each of the three commutes over the base, and take every available
    /// (site, regime) pair's panels.
    ///
    /// The laws come in the order they constrain each other: the triangle, then
    /// the sortings, then commutation with the base. A morphism is pinned by the
    /// sortings it has to commute with, so the triangle is checked first and a
    /// document that disagrees with the other two is named against them.
    pub fn load(specs: &TrioSpecs<'_>) -> Result<Self, String> {
        let base = BaseTopos::from_json_str(specs.base).map_err(|error| error.to_string())?;
        let morphism = |label: &str, json: &str| {
            GeometricMorphism::from_json_str(json).map_err(|error| format!("{label} does not decode: {error}"))
        };
        let f = morphism(MorphismId::F.name(), specs.f)?;
        let psi = morphism(MorphismId::Psi.name(), specs.psi)?;
        let chi = morphism(MorphismId::Chi.name(), specs.chi)?;
        let p_agentic = morphism("p_agentic", specs.agentic_physics)?;
        let p_membrane = morphism("p_membrane", specs.membrane_physics)?;
        let p_catalog = morphism("p_catalog", specs.catalog_physics)?;
        let regimes = [
            Regime::load(&base, specs.agentic, &p_agentic, RegimeId::Agentic)?,
            Regime::load(&base, specs.membrane, &p_membrane, RegimeId::Membrane)?,
            Regime::load(&base, specs.okf_catalog, &p_catalog, RegimeId::OkfCatalog)?,
        ];

        // The triangle comes first. Each morphism is pinned by the two sortings
        // it has to commute with, so a document that is not the one the other two
        // compose to is reported against those two — which says which document is
        // wrong and where — rather than against a sorting law it breaks downstream
        // of that disagreement.
        let triangle = chi
            .compose(&f)
            .map_err(|error| format!("the triangle f ∘ χ = ψ does not compose: {error}"))?;
        check_triangle(&triangle, &psi)?;

        f.validate(&regimes[0].topos, &regimes[1].topos)
            .map_err(|error| format!("f is not a morphism of its topos specs: {error}"))?;
        psi.validate(&regimes[2].topos, &regimes[1].topos)
            .map_err(|error| format!("ψ is not a morphism of its topos specs: {error}"))?;
        chi.validate(&regimes[2].topos, &regimes[0].topos)
            .map_err(|error| format!("χ is not a morphism of its topos specs: {error}"))?;

        f.validate_over(&p_agentic, &p_membrane)
            .map_err(|error| format!("p_membrane ∘ f = p_agentic over the base fails: {error}"))?;
        psi.validate_over(&p_catalog, &p_membrane)
            .map_err(|error| format!("p_membrane ∘ ψ = p_catalog over the base fails: {error}"))?;
        chi.validate_over(&p_catalog, &p_agentic)
            .map_err(|error| format!("p_agentic ∘ χ = p_catalog over the base fails: {error}"))?;

        let trace_grammars = [
            GrammarEntry::compile(GRAMMAR_LINES, specs.trace_lines_grammar, specs.trace_lines_data)?,
            GrammarEntry::compile(GRAMMAR_JSON, specs.trace_json_grammar, specs.trace_json_data)?,
        ];
        let okf_grammar = GrammarEntry::compile(GRAMMAR_OKF, specs.okf_grammar, specs.okf_data)?;
        let sites = [
            SiteData::parse(SiteId::Traces, &trace_grammars, 0, &regimes)?,
            SiteData::parse(SiteId::Okf, std::slice::from_ref(&okf_grammar), 0, &regimes)?,
        ];

        let mut host = Self {
            regimes,
            f,
            psi,
            chi,
            trace_grammars,
            okf_grammar,
            sites,
            site: SiteId::Traces,
            regime: RegimeId::Agentic,
            selected: None,
            subobject: BTreeSet::new(),
            overlays: Overlays::empty(),
        };
        host.subobject = host.selected_sorts();
        host.recompute();
        Ok(host)
    }

    /// The morphism path from the site's native regime to `regime`, or the
    /// reason the pair is not offered.
    pub fn available(site: SiteId, regime: RegimeId) -> Result<Route, String> {
        let native = Route {
            native: site.native_regime(),
            regime,
            path: Vec::new(),
            composite: None,
        };
        match (site, regime) {
            (SiteId::Traces, RegimeId::Agentic) | (SiteId::Okf, RegimeId::OkfCatalog) => Ok(native),
            (SiteId::Traces, RegimeId::Membrane) => Ok(Route {
                native: RegimeId::Agentic,
                regime,
                path: vec![MorphismId::F],
                composite: None,
            }),
            (SiteId::Okf, RegimeId::Agentic) => Ok(Route {
                native: RegimeId::OkfCatalog,
                regime,
                path: vec![MorphismId::Chi],
                composite: None,
            }),
            (SiteId::Okf, RegimeId::Membrane) => Ok(Route {
                native: RegimeId::OkfCatalog,
                regime,
                path: vec![MorphismId::Psi],
                composite: Some(vec![MorphismId::Chi, MorphismId::F]),
            }),
            (SiteId::Traces, RegimeId::OkfCatalog) => Err(NO_MORPHISM_TO_CATALOG.to_owned()),
        }
    }

    /// The active site.
    pub fn site_id(&self) -> SiteId {
        self.site
    }

    /// The parsed objects of the active site.
    pub fn site(&self) -> &Site {
        &self.sites[self.site.index()].site
    }

    /// Links the OKF engine could not resolve to an object of the active site;
    /// always zero on the agent-trace site.
    pub fn unresolved_links(&self) -> usize {
        self.sites[self.site.index()].unresolved_links
    }

    /// The regime the active site is read in.
    pub fn regime(&self) -> RegimeId {
        self.regime
    }

    /// The morphism path the active pair takes, or the reason it has none.
    pub fn route(&self) -> Result<Route, String> {
        Self::available(self.site, self.regime)
    }

    /// The grammar ids that read the active site, in cycle order.
    pub fn grammar_ids(&self) -> &'static [&'static str] {
        self.site.grammar_ids()
    }

    /// The grammar that read the active site.
    pub fn grammar_id(&self) -> &'static str {
        self.grammars()[self.sites[self.site.index()].grammar].id
    }

    /// Read the active site with another of its grammars; false when the site
    /// has no such grammar, or when that grammar cannot read the data.
    pub fn set_grammar(&mut self, id: &str) -> bool {
        match self.grammars().iter().position(|entry| entry.id == id) {
            Some(index) => self.use_grammar(index),
            None => false,
        }
    }

    /// Advance to the next grammar of the active site.
    pub fn cycle_grammar(&mut self) {
        let count = self.grammars().len();
        if count > 1 {
            self.use_grammar((self.sites[self.site.index()].grammar + 1) % count);
        }
    }

    /// Read the other site, keeping the regime when that site can carry it and
    /// otherwise taking the other site's native regime. The selection names an
    /// object of one site, so it does not survive the switch.
    pub fn set_site(&mut self, site: SiteId) {
        if site == self.site {
            return;
        }
        self.site = site;
        if Self::available(site, self.regime).is_err() {
            self.regime = site.native_regime();
        }
        self.selected = None;
        self.subobject.clear();
        self.recompute();
    }

    /// Read the active site in `regime`, transporting the active subobject along
    /// the route: `∃` forward away from the native regime, `f*` back towards it.
    /// `Err` names why the pair is not offered.
    pub fn set_regime(&mut self, regime: RegimeId) -> Result<(), String> {
        Self::available(self.site, regime)?;
        if regime != self.regime {
            self.transport_to(regime);
            self.regime = regime;
            self.recompute();
        }
        Ok(())
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
    /// placeholder, one of the three host overlays, else the active regime's
    /// sheaf over the active site.
    pub fn resolve(&self, binding: &str) -> Option<&Section> {
        let regime = self.regime_state();
        if binding == self.regime.graph_binding() {
            return Some(&regime.graph);
        }
        match binding {
            MORPHISM => Some(&self.overlays.morphism),
            PHYSICS => Some(&self.overlays.physics),
            SELECTED | SELECTED_OKF => Some(&self.overlays.selected),
            other => self.projection().sections.get(other),
        }
    }

    /// The bindings of the active (site, regime) pair whose sheaf cannot read
    /// the active site, in binding order.
    pub fn unreadable(&self) -> Vec<&str> {
        self.projection().unreadable.iter().map(String::as_str).collect()
    }

    /// The selected node id.
    pub fn selected(&self) -> Option<&str> {
        self.selected.as_deref()
    }

    /// Select `node`, or clear the selection when it names no object of the
    /// active site.
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
        let count = self.site().objects.len();
        if count == 0 {
            return;
        }
        let current = self
            .selected
            .as_deref()
            .and_then(|id| self.site().objects.iter().position(|object| object.id == id))
            .map_or(-step, |index| index as isize);
        let next = (current + step).rem_euclid(count as isize) as usize;
        let id = self.site().objects[next].id.clone();
        self.select(Some(&id));
    }

    /// The active subobject: the active regime's sorts covering the selected
    /// object, transported by [`set_regime`](Self::set_regime). Empty when
    /// nothing is selected.
    pub fn active_sorts(&self) -> &BTreeSet<String> {
        &self.subobject
    }

    /// The renderer-neutral graph over the active site: objects in site order,
    /// sorted and coloured by the active regime, sized by degree and tuned by
    /// the active regime's `p_R*` of the one base. The tuning is the same in
    /// every regime that can read the site; only the sort and the colour move.
    pub fn graph(&self) -> GraphModel {
        let site = self.site();
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

    /// One component per connected piece of the active site under the active
    /// regime's assembly bonds, with the stage its β₁, χ, and boundary classify.
    pub fn assembly(&self) -> Vec<ComponentInvariants> {
        assembly(
            self.site(),
            &self.regime_state().topos.morphism_sorting,
            &self.regime_state().assembly_sorts,
        )
    }

    /// The regime's own words for the stage one component assembled into.
    pub fn stage_label(&self, stage: Stage) -> &str {
        stage_label(&self.regime_state().topos.stalks, stage)
    }

    /// One line of site, grammar, regime, route, size, selection, and the count
    /// of panels that cannot read the active site.
    pub fn status_line(&self) -> String {
        let site = self.site();
        let route = match self.route() {
            Ok(route) => route.text(),
            Err(reason) => format!("{} unavailable: {reason}", self.regime.label()),
        };
        let mut line = format!(
            "site {} · grammar {} · {} · {route} · {} objects {} morphisms",
            self.site.label(),
            self.grammar_id(),
            self.regime.label(),
            site.objects.len(),
            site.morphisms.len()
        );
        if let Some(id) = self.selected() {
            line.push_str(" · selected ");
            line.push_str(id);
        }
        let unreadable = self.projection().unreadable.len();
        if unreadable > 0 {
            line.push_str(&format!(" · {unreadable} panels cannot read this site"));
        }
        line
    }

    fn regime_state(&self) -> &Regime {
        &self.regimes[self.regime.index()]
    }

    fn projection(&self) -> &Projection {
        &self.sites[self.site.index()].projections[self.regime.index()]
    }

    fn grammars(&self) -> &[GrammarEntry] {
        match self.site {
            SiteId::Traces => &self.trace_grammars,
            SiteId::Okf => std::slice::from_ref(&self.okf_grammar),
        }
    }

    fn morphism(&self, id: MorphismId) -> &GeometricMorphism {
        match id {
            MorphismId::F => &self.f,
            MorphismId::Psi => &self.psi,
            MorphismId::Chi => &self.chi,
        }
    }

    fn object(&self, id: &str) -> Option<&Object> {
        self.site().objects.iter().find(|object| object.id == id)
    }

    /// The active regime's reading of the selected object's sorts.
    fn selected_sorts(&self) -> BTreeSet<String> {
        self.selected
            .as_deref()
            .and_then(|id| self.object(id))
            .map(|object| BTreeSet::from([self.regime_state().sort_of(&object.kind).to_owned()]))
            .unwrap_or_default()
    }

    /// Re-read the agent-trace site under another of its grammars. The OKF site
    /// has one grammar, so it is untouched.
    fn use_grammar(&mut self, grammar: usize) -> bool {
        if grammar == self.sites[self.site.index()].grammar {
            return true;
        }
        if self.sites[SiteId::Traces.index()]
            .reparse(&self.trace_grammars, grammar, &self.regimes)
            .is_err()
        {
            return false;
        }
        if self.selected.as_ref().is_some_and(|id| self.object(id).is_none()) {
            self.selected = None;
        }
        self.subobject = self.selected_sorts();
        self.recompute();
        true
    }

    /// Carry the active subobject to `next`: `f*` back to the site's native
    /// regime, then `∃` out along the route to `next`. Both legs are the route's
    /// own morphisms, so the triangle's commutation is what makes them agree.
    fn transport_to(&mut self, next: RegimeId) {
        let native = self.site.native_regime();
        let mut sorts = self.subobject.clone();
        if self.regime != native {
            if let Ok(route) = Self::available(self.site, self.regime) {
                for step in route.path.iter().rev() {
                    sorts = self.morphism(*step).pullback_subobject(&sorts);
                }
            }
        }
        if next != native {
            if let Ok(route) = Self::available(self.site, next) {
                for step in &route.path {
                    sorts = self.morphism(*step).image(&sorts);
                }
            }
        }
        self.subobject = sorts;
    }

    fn recompute(&mut self) {
        self.overlays.selected = Section::Text(TextModel { text: self.selected_text() });
        self.overlays.morphism = Section::Text(TextModel { text: self.morphism_text() });
        self.overlays.physics = self.physics_section();
    }

    /// The selected object's attributes, then its neighbours under the active
    /// regime's morphism sorts.
    fn selected_text(&self) -> String {
        let site = self.site();
        let regime = self.regime_state();
        let mut lines = vec!["Selected object".to_owned()];
        let Some(id) = self.selected.as_deref() else {
            lines.push("nothing selected: press n or p, or click a node.".to_owned());
            return lines.join("\n");
        };
        let Some(object) = self.object(id) else {
            lines.push(format!("{id} is not an object of the {} site", self.site.label()));
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

    /// The route the active pair takes, φ as a table over the native regime's
    /// sorts with the fibre node counts read off the active site, and the
    /// commuting triangle the route rides.
    fn morphism_text(&self) -> String {
        let site = self.site();
        let Ok(route) = self.route() else {
            let reason = Self::available(self.site, self.regime)
                .err()
                .unwrap_or_else(|| String::from("no route"));
            return format!("{} does not read the {} site: {reason}", self.regime.label(), self.site.label());
        };
        let mut lines = vec![
            format!(
                "{} site · native {} · read as {}",
                self.site.label(),
                route.native.label(),
                self.regime.label()
            ),
            format!("route: {}", route.text()),
        ];
        for step in &route.path {
            lines.push(format!(
                "{} · {} objects, {} morphisms",
                step.name(),
                site.objects.len(),
                site.morphisms.len()
            ));
        }
        if let Some(composite) = &route.composite {
            lines.push(format!(
                "the triangle makes this the same map as {}",
                path_text(composite)
            ));
        }
        lines.push(String::new());
        lines.push(format!(
            "φ fibres on the {} site ({} -> {} : objects):",
            self.site.label(),
            route.native.label(),
            self.regime.label()
        ));
        for (sort, sources, nodes) in self.fibres() {
            lines.push(format!(
                "  {} -> {} : {nodes} objects",
                if sources.is_empty() { "∅".to_owned() } else { sources.join(", ") },
                self.regime_state().sort_label(&sort)
            ));
        }
        lines.push(String::new());
        lines.push(format!(
            "triangle: f ∘ χ = ψ — {} is {}",
            MorphismId::Psi.name(),
            path_text(&[MorphismId::Chi, MorphismId::F])
        ));
        lines.join("\n")
    }

    /// The tuning the active regime pulls back from the base, one row per sort,
    /// and the law that makes it the same in every regime at this site.
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
            "the same engine and the same numbers in every regime that reads this site".to_owned(),
        ]));
        Section::Table(TableModel { columns, rows })
    }

    /// φ's fibres over the active regime's object sorts: the native regime's
    /// sorts reaching each one, with how many site objects each fibre covers.
    fn fibres(&self) -> Vec<(String, Vec<String>, usize)> {
        let native = &self.regimes[self.site.native_regime().index()];
        let target = self.regime_state();
        let route = self.route().ok();
        let mut image: BTreeMap<&str, &str> =
            native.topos.stalks.objects.keys().map(|sort| (sort.as_str(), sort.as_str())).collect();
        for step in route.as_ref().map(|route| route.path.clone()).unwrap_or_default() {
            let morphism = self.morphism(step);
            image = image
                .iter()
                .map(|(sort, image)| {
                    (
                        *sort,
                        morphism.object_sorts.get(*image).map_or(*image, String::as_str),
                    )
                })
                .collect();
        }
        let site = self.site();
        target
            .topos
            .stalks
            .objects
            .keys()
            .map(|sort| {
                let sources: Vec<String> = image
                    .iter()
                    .filter(|(_, image)| **image == sort.as_str())
                    .map(|(sort, _)| native.sort_label(sort))
                    .collect();
                let nodes = site
                    .objects
                    .iter()
                    .filter(|object| {
                        image.get(native.sort_of(&object.kind)).is_some_and(|image| *image == sort.as_str())
                    })
                    .count();
                (sort.clone(), sources, nodes)
            })
            .collect()
    }
}

/// The law that makes the three morphisms a triangle: `f ∘ χ` must be `ψ` on
/// every object and every morphism sort, over the same sorts.
fn check_triangle(composed: &GeometricMorphism, psi: &GeometricMorphism) -> Result<(), String> {
    if let Some((sort, image)) = composed
        .object_sorts
        .iter()
        .find(|(sort, image)| psi.object_sorts.get(*sort) != Some(*image))
    {
        return Err(format!(
            "the triangle f ∘ χ = ψ fails on the object sort {sort}: χ then f sends it to {image}, ψ sends it to {}",
            image_of(&psi.object_sorts, sort)
        ));
    }
    if let Some((sort, image)) = composed
        .morphism_sorts
        .iter()
        .find(|(sort, image)| psi.morphism_sorts.get(*sort) != Some(*image))
    {
        return Err(format!(
            "the triangle f ∘ χ = ψ fails on the morphism sort {sort}: χ then f sends it to {image}, ψ sends it to {}",
            image_of(&psi.morphism_sorts, sort)
        ));
    }
    if composed.object_sorts != psi.object_sorts || composed.morphism_sorts != psi.morphism_sorts {
        return Err(format!(
            "the triangle f ∘ χ = ψ fails: χ then f sorts {} objects and {} morphisms, ψ sorts {} and {}",
            composed.object_sorts.len(),
            composed.morphism_sorts.len(),
            psi.object_sorts.len(),
            psi.morphism_sorts.len()
        ));
    }
    Ok(())
}

/// Where `sort` lands, or `nothing` when the map does not cover it.
fn image_of(sorts: &BTreeMap<String, String>, sort: &str) -> String {
    sorts.get(sort).cloned().unwrap_or_else(|| String::from("nothing"))
}

/// The regime's resolved panels over a site, taken as one set where it can be
/// and sheaf by sheaf where it cannot: `Topos::global_sections` is
/// all-or-nothing, and a regime authored over one site can be asked to read
/// another whose objects do not carry the fields its tables ask for.
fn project(id: SiteId, regime: &Regime, site: &Site) -> Projection {
    if let Ok(all) = regime.topos.global_sections(site, &regime.physics) {
        return Projection {
            sections: all
                .iter()
                .map(|(binding, section)| (binding.clone(), section.clone()))
                .collect(),
            unreadable: BTreeSet::new(),
        };
    }
    let mut projection = Projection::default();
    for (binding, sheaf) in &regime.topos.sheaves {
        let mut alone = regime.topos.clone();
        alone.sheaves = BTreeMap::from([(binding.clone(), sheaf.clone())]);
        let outcome = alone
            .global_sections(site, &regime.physics)
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
                    Section::Text(TextModel { text: unreadable_text(id, regime, binding, &error) }),
                );
            }
        }
    }
    projection
}

/// What a panel says when the regime's sheaf cannot read the site.
fn unreadable_text(id: SiteId, regime: &Regime, binding: &str, error: &str) -> String {
    format!(
        "{binding} does not read the {} site.\n{} is authored over the agent-trace site, and this \
         panel asks for a field the {} site does not carry.\nsheaf: {error}\nThe site's own \
         regime {} reads it.",
        id.label(),
        regime.topos.id,
        id.label(),
        id.native_regime().label(),
    )
}

/// The bonds one regime assembles over: the structural ones, not the ambient
/// bond that ties every object to every other (the agentic `consolidates`, the
/// membrane `solvation`, the catalog's `contains`), so the components counted
/// here are the ones that actually have a stage.
fn assembly_sorts(id: RegimeId) -> BTreeSet<String> {
    let sorts: &[&str] = match id {
        RegimeId::Agentic => &["continues", "shares"],
        RegimeId::Membrane => &["tail_bond", "lateral"],
        RegimeId::OkfCatalog => &["derived_from", "computes", "executed_by", "attested_by", "references"],
    };
    sorts.iter().map(|sort| (*sort).to_owned()).collect()
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
        "planner" => colors.blue,
        "coder" => colors.green,
        "reviewer" => colors.yellow,
        "memory" => colors.dim,
        // topos-membrane: the lipids those sessions read as.
        "head" => colors.accent,
        "tail" => colors.red,
        "solvent" => colors.badge_info,
        // topos-okf-catalog: the normalised OKF types.
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

/// A sort set as `{a, b}`, or `∅` when empty.
fn sort_names(sorts: &BTreeSet<String>) -> String {
    if sorts.is_empty() {
        "∅".to_owned()
    } else {
        format!("{{{}}}", sorts.iter().cloned().collect::<Vec<_>>().join(", "))
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

fn column(key: &str, title: &str, align: TextAlign, width: ColumnWidth) -> TableColumn {
    TableColumn { key: key.to_owned(), title: title.to_owned(), width, align }
}

fn row(cells: Vec<String>) -> TableRow {
    TableRow { cells: cells.into_iter().map(TableCell::Text).collect() }
}
