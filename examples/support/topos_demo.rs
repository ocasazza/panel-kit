//! Host façade over `panel_kit_grammar` for the topos/grammar demo hosts.
//!
//! Two grammars parse equivalent data into equal sites; two regime topoi take
//! global sections of each site into panel content. The physics base topos and
//! three geometric morphisms (f: agentic→membrane, p_agentic, p_membrane) are
//! loaded and validated at startup, so `p_membrane ∘ f = p_agentic` and each
//! regime's physics tuning is the inverse image `p_R*` of the base. Build-time
//! `include_str!(env!(..))` embeds the Nix-authored JSON. Hosts paint each panel
//! from `resolve(binding_id)` and feed each topos's `workspace_json` to their
//! spec resolver.
//!
//! The demo also holds an **active subobject** (a set of regime sorts) shown on
//! the per-sort badges sheaf. Switching agentic→membrane transports it by the
//! image `∃f`; membrane→agentic by the inverse image `f*`.
//!
//! Only `panel_kit_grammar` is used: this file is `#[path]`-included by the
//! root-crate and `panel-kit-tui` examples and by the host-contract test.

use std::collections::BTreeSet;

use panel_kit_grammar::{
    BadgeGroup, BaseTopos, CompiledGrammar, GeometricMorphism, GlobalSections, GrammarPackage,
    status_table, MorphismSide, Physics, PhysicsSection, Section, Sheaf, Site, Topos,
};

/// Agentic-trace regime topos id (slot A).
pub const TOPOS_AGENTIC: &str = "topos-agentic";
/// Lipid self-assembly regime topos id (slot B).
pub const TOPOS_MEMBRANE: &str = "topos-membrane";
/// Line-format grammar id.
pub const GRAMMAR_LINES: &str = "trace-lines";
/// JSON-format grammar id.
pub const GRAMMAR_JSON: &str = "trace-json";

const TOPOS_A_JSON: &str = include_str!(env!("PANEL_KIT_TOPOS_A"));
const TOPOS_B_JSON: &str = include_str!(env!("PANEL_KIT_TOPOS_B"));
const TOPOS_PHYSICS_JSON: &str = include_str!(env!("PANEL_KIT_TOPOS_PHYSICS"));
const MORPHISM_JSON: &str = include_str!(env!("PANEL_KIT_MORPHISM"));
const MORPHISM_A_PHYSICS_JSON: &str = include_str!(env!("PANEL_KIT_MORPHISM_A_PHYSICS"));
const MORPHISM_B_PHYSICS_JSON: &str = include_str!(env!("PANEL_KIT_MORPHISM_B_PHYSICS"));
const GRAMMAR_LINES_PACKAGE: &str = include_str!(env!("PANEL_KIT_GRAMMAR_LINES"));
const GRAMMAR_JSON_PACKAGE: &str = include_str!(env!("PANEL_KIT_GRAMMAR_JSON"));
const DATA_LINES: &str = include_str!(env!("PANEL_KIT_DATA_LINES"));
const DATA_JSON: &str = include_str!(env!("PANEL_KIT_DATA_JSON"));

/// Workspace spec JSON embedded in a topos, parsed from its build-time
/// constant for host spec resolvers needed before a [`ToposDemo`] exists.
pub fn workspace_spec_json(id: &str) -> Option<String> {
    let json = match id {
        TOPOS_AGENTIC => TOPOS_A_JSON,
        TOPOS_MEMBRANE => TOPOS_B_JSON,
        _ => return None,
    };
    Some(
        Topos::from_json_str(json)
            .expect("topos spec decodes")
            .workspace_json(),
    )
}

/// Wire format a grammar's editable source is authored in.
#[derive(Clone, Copy)]
enum Syntax {
    Lines,
    Json,
}

/// One grammar: its compiled parser, canonical data source, and append syntax.
struct GrammarEntry {
    id: &'static str,
    grammar: CompiledGrammar,
    data: &'static str,
    syntax: Syntax,
}

impl GrammarEntry {
    fn compile(id: &'static str, package: &str, data: &'static str, syntax: Syntax) -> Self {
        let grammar = GrammarPackage::from_json_str(package)
            .expect("grammar package decodes")
            .compile()
            .expect("grammar package compiles");
        debug_assert_eq!(grammar.id(), id);
        Self {
            id,
            grammar,
            data,
            syntax,
        }
    }

    /// A suffix the grammar accepts: a new object line for `Lines`; nothing for
    /// the JSON document, since a record cannot be appended as a trailing line.
    fn valid_suffix(&self, seq: usize) -> String {
        match self.syntax {
            Syntax::Lines => format!(
                "N|appended_{seq}|Appended {seq}|memory|demo|latency_ms={lat};energy={energy};t={t};status=ok",
                lat = 3 + seq,
                energy = 5 + seq,
                t = 64 + seq,
            ),
            Syntax::Json => String::new(),
        }
    }

    /// A suffix the grammar rejects: an incomplete object line, or JSON trailing
    /// garbage.
    fn malformed_suffix(&self, seq: usize) -> String {
        match self.syntax {
            Syntax::Lines => format!("N|broken_{seq}"),
            Syntax::Json => format!("!malformed_{seq}"),
        }
    }
}

/// One topos: its validated spec, serialized workspace, physics tuning
/// (`p_R*` of the base), and the per-sort badges binding id, if any.
struct ToposEntry {
    id: &'static str,
    spec: Topos,
    workspace_json: String,
    physics: Physics,
    badge_binding: Option<String>,
    morphism_binding: Option<String>,
    invariants_binding: Option<String>,
}

impl ToposEntry {
    fn build(id: &'static str, spec: Topos, physics: Physics) -> Self {
        let workspace_json = spec.workspace_json();
        let badge_binding = kinds_badges_binding(&spec);
        let morphism_binding = marker_binding(&spec, |sheaf| matches!(sheaf, Sheaf::Morphism));
        let invariants_binding = marker_binding(&spec, |sheaf| matches!(sheaf, Sheaf::Invariants));
        Self {
            id,
            spec,
            workspace_json,
            physics,
            badge_binding,
            morphism_binding,
            invariants_binding,
        }
    }
}

/// Active grammar, active topos, the current site, its global sections, and the
/// active subobject shown on the per-sort badges.
pub struct ToposDemo {
    grammars: Vec<GrammarEntry>,
    topoi: Vec<ToposEntry>,
    morphism: GeometricMorphism,
    grammar: usize,
    topos: usize,
    source: String,
    site: Site,
    contents: GlobalSections,
    active: BTreeSet<String>,
    active_badges: Option<Section>,
    badge_base: Option<Section>,
    morphism_section: Option<Section>,
    invariants_section: Option<Section>,
    focus: usize,
    morphism_note: Option<String>,
    diagnostics: Vec<String>,
    appended: usize,
    grammars_agree: bool,
}

impl ToposDemo {
    /// Compile both grammars, load and validate the base topos, both regimes,
    /// and the three morphisms, build one [`Physics`] per regime, and take
    /// global sections of the lines site under the agentic topos.
    pub fn new() -> Self {
        let grammars = vec![
            GrammarEntry::compile(GRAMMAR_LINES, GRAMMAR_LINES_PACKAGE, DATA_LINES, Syntax::Lines),
            GrammarEntry::compile(GRAMMAR_JSON, GRAMMAR_JSON_PACKAGE, DATA_JSON, Syntax::Json),
        ];

        let base = BaseTopos::from_json_str(TOPOS_PHYSICS_JSON).expect("physics base decodes");
        let morphism =
            GeometricMorphism::from_json_str(MORPHISM_JSON).expect("agentic→membrane morphism decodes");
        let p_agentic =
            GeometricMorphism::from_json_str(MORPHISM_A_PHYSICS_JSON).expect("p_agentic decodes");
        let p_membrane =
            GeometricMorphism::from_json_str(MORPHISM_B_PHYSICS_JSON).expect("p_membrane decodes");

        let agentic_spec = decode_topos(TOPOS_A_JSON);
        let membrane_spec = decode_topos(TOPOS_B_JSON);

        morphism
            .validate(&agentic_spec, &membrane_spec)
            .expect("f: agentic→membrane commutes with the sortings");
        morphism
            .validate_over(&p_agentic, &p_membrane)
            .expect("p_membrane ∘ f = p_agentic over the base");

        let agentic_physics = Physics::new(&base, &p_agentic, &agentic_spec)
            .expect("agentic pulls back the base tuning");
        let membrane_physics = Physics::new(&base, &p_membrane, &membrane_spec)
            .expect("membrane pulls back the base tuning");

        let topoi = vec![
            ToposEntry::build(TOPOS_AGENTIC, agentic_spec, agentic_physics),
            ToposEntry::build(TOPOS_MEMBRANE, membrane_spec, membrane_physics),
        ];

        let source = grammars[0].data.to_owned();
        let site = grammars[0]
            .grammar
            .parse(&source)
            .expect("canonical lines data parses");

        let grammars_agree = sites_agree(&grammars);
        let mut demo = Self {
            grammars,
            topoi,
            morphism,
            grammar: 0,
            topos: 0,
            source,
            site,
            contents: GlobalSections::default(),
            active: BTreeSet::new(),
            active_badges: None,
            badge_base: None,
            morphism_section: None,
            invariants_section: None,
            focus: 0,
            morphism_note: None,
            diagnostics: Vec::new(),
            appended: 0,
            grammars_agree,
        };
        demo.active = demo.current_sort_set();
        demo.reproject();
        demo
    }

    /// Every grammar id, in cycle order.
    pub fn grammar_ids(&self) -> Vec<&'static str> {
        self.grammars.iter().map(|entry| entry.id).collect()
    }

    /// Active grammar id.
    pub fn grammar_id(&self) -> &'static str {
        self.grammars[self.grammar].id
    }

    /// Switch grammar, re-parsing that grammar's data file; false when unknown.
    pub fn set_grammar(&mut self, id: &str) -> bool {
        let Some(index) = self.grammars.iter().position(|entry| entry.id == id) else {
            return false;
        };
        self.grammar = index;
        self.reset_source();
        true
    }

    /// Advance to the next grammar, re-parsing its data file.
    pub fn cycle_grammar(&mut self) {
        self.grammar = (self.grammar + 1) % self.grammars.len();
        self.reset_source();
    }

    /// Every topos id, in cycle order.
    pub fn topos_ids(&self) -> Vec<&'static str> {
        self.topoi.iter().map(|entry| entry.id).collect()
    }

    /// Active topos id.
    pub fn topos_id(&self) -> &'static str {
        self.topoi[self.topos].id
    }

    /// Switch topos and re-take global sections of the current site, transporting
    /// the active subobject through the morphism; false when unknown.
    pub fn set_topos(&mut self, id: &str) -> bool {
        let Some(index) = self.topoi.iter().position(|entry| entry.id == id) else {
            return false;
        };
        self.transport_active(index);
        self.topos = index;
        self.focus = 0;
        self.reproject();
        true
    }

    /// The topos id the next switch selects.
    pub fn next_topos_id(&self) -> &'static str {
        self.topoi[(self.topos + 1) % self.topoi.len()].id
    }

    /// The embedded workspace spec JSON of a topos, for host spec resolvers.
    pub fn workspace_json(&self, id: &str) -> Option<&str> {
        self.topoi
            .iter()
            .find(|entry| entry.id == id)
            .map(|entry| entry.workspace_json.as_str())
    }

    /// Append one line the active grammar accepts, growing the site.
    pub fn append_valid_line(&mut self) {
        let suffix = self.grammars[self.grammar].valid_suffix(self.appended);
        self.apply_suffix(&suffix);
    }

    /// Append one line the active grammar rejects, surfacing the error.
    pub fn append_malformed_line(&mut self) {
        let suffix = self.grammars[self.grammar].malformed_suffix(self.appended);
        self.apply_suffix(&suffix);
    }

    /// Restore the active grammar's data file and clear diagnostics.
    pub fn reset_source(&mut self) {
        let index = self.grammar;
        self.source = self.grammars[index].data.to_owned();
        self.appended = 0;
        self.diagnostics.clear();
        self.site = self.grammars[index]
            .grammar
            .parse(&self.source)
            .expect("canonical data parses");
        self.reproject();
    }

    /// Object count of the current site.
    pub fn record_count(&self) -> usize {
        self.site.objects.len()
    }

    /// Rejected append attempts since the last reset or grammar switch.
    pub fn diagnostic_count(&self) -> usize {
        self.diagnostics.len()
    }

    /// The current parsed site.
    pub fn site(&self) -> &Site {
        &self.site
    }

    /// The current topos spec (sortings, stalks, sheaves).
    pub fn current_topos(&self) -> &Topos {
        &self.topoi[self.topos].spec
    }

    /// The physics section over the current site under the active regime. Equal
    /// under both regimes: the base tuning pulled back along `p_R`.
    pub fn physics_section(&self) -> PhysicsSection {
        let entry = &self.topoi[self.topos];
        entry.spec.physics_section(&self.site, &entry.physics)
    }

    /// Binding id of the active topos's per-sort badges sheaf, if it has one.
    pub fn active_binding(&self) -> Option<&str> {
        self.topoi[self.topos].badge_binding.as_deref()
    }

    /// The active subobject: the set of currently-active regime sorts.
    pub fn active_set(&self) -> &BTreeSet<String> {
        &self.active
    }

    /// Replace the active subobject, keeping only sorts of the current topos.
    pub fn set_active<I: IntoIterator<Item = String>>(&mut self, sorts: I) {
        let allowed = self.current_sort_set();
        self.active = sorts.into_iter().filter(|sort| allowed.contains(sort)).collect();
        self.morphism_note = None;
        self.reproject_subobject();
    }

    /// Toggle one regime sort in the active subobject; ignores unknown sorts.
    pub fn toggle_active_sort(&mut self, sort: &str) {
        if !self.current_sort_set().contains(sort) {
            return;
        }
        if !self.active.remove(sort) {
            self.active.insert(sort.to_owned());
        }
        self.morphism_note = None;
        self.reproject_subobject();
    }

    /// Advance the terminal sort-focus cursor over the per-sort badges.
    pub fn cycle_sort_focus(&mut self) {
        let count = self.active_sorts().len();
        self.focus = if count == 0 { 0 } else { (self.focus + 1) % count };
    }

    /// Toggle the sort under the terminal focus cursor.
    pub fn toggle_focused_sort(&mut self) {
        if let Some(sort) = self.active_sorts().get(self.focus).cloned() {
            self.toggle_active_sort(&sort);
        }
    }

    /// One-line status of topos, grammar, site size, last transport, focus, and
    /// diagnostics.
    pub fn status_line(&self) -> String {
        let topos = self.topoi[self.topos].id.trim_start_matches("topos-");
        let grammar = self.grammars[self.grammar].id;
        let objects = self.site.objects.len();
        let morphisms = self.site.morphisms.len();
        let mut line = format!("topos {topos} · grammar {grammar} · {objects}n {morphisms}e");
        if let Some(note) = &self.morphism_note {
            line.push_str(" · ");
            line.push_str(note);
        }
        if let Some(label) = self.focus_label() {
            line.push_str(" · focus ");
            line.push_str(&label);
        }
        if !self.diagnostics.is_empty() {
            line.push_str(&format!(" · {} diag", self.diagnostics.len()));
        }
        line
    }

    /// Resolved content for one binding id; the active topos's per-sort badges
    /// carry the active subobject. `None` when the active topos does not bind it.
    pub fn resolve(&self, binding: &str) -> Option<&Section> {
        let entry = &self.topoi[self.topos];
        if entry.badge_binding.as_deref() == Some(binding) {
            if let Some(section) = self.active_badges.as_ref() {
                return Some(section);
            }
        }
        if entry.morphism_binding.as_deref() == Some(binding) {
            if let Some(section) = self.morphism_section.as_ref() {
                return Some(section);
            }
        }
        if entry.invariants_binding.as_deref() == Some(binding) {
            if let Some(section) = self.invariants_section.as_ref() {
                return Some(section);
            }
        }
        self.contents.get(binding)
    }

    /// Parse `source` plus `suffix`; commit and re-project on success, else keep
    /// the last good site and record the grammar error.
    fn apply_suffix(&mut self, suffix: &str) {
        self.appended += 1;
        let trial = joined(&self.source, suffix);
        match self.grammars[self.grammar].grammar.parse(&trial) {
            Ok(site) => {
                self.source = trial;
                self.site = site;
                self.reproject();
            }
            Err(error) => self.diagnostics.push(error.to_string()),
        }
    }

    /// Full re-projection after a site or topos change: the panels over the
    /// active subobject, the per-sort badge control over the whole site, and the
    /// overlays. Prior content is kept on failure.
    fn reproject(&mut self) {
        self.recompute_badge_base();
        self.recompute_panels();
        self.recompute_overlays();
    }

    /// Re-projection after only the active subobject changed: the panels and the
    /// overlays, but not the whole-site badge control.
    fn reproject_subobject(&mut self) {
        self.recompute_panels();
        self.recompute_overlays();
    }

    /// The subsite over the active subobject `U`: objects whose sort is in `U`
    /// and the morphisms between them.
    fn restricted_site(&self) -> Site {
        let entry = &self.topoi[self.topos];
        self.site.restrict_objects(&entry.spec.object_sorting, &self.active)
    }

    /// Γ(U, F): re-take global sections over the restricted subsite, keeping
    /// prior content on failure.
    fn recompute_panels(&mut self) {
        let subsite = self.restricted_site();
        let sections = {
            let entry = &self.topoi[self.topos];
            entry.spec.global_sections(&subsite, &entry.physics)
        };
        match sections {
            Ok(contents) => self.contents = contents,
            Err(error) => self.diagnostics.push(error.to_string()),
        }
    }

    /// The per-sort badge control is Γ over the whole site (every sort), so it
    /// lists every sort regardless of `U`. Recomputed only on a site/topos change.
    fn recompute_badge_base(&mut self) {
        let Some(binding) = self.active_binding().map(str::to_owned) else {
            self.badge_base = None;
            return;
        };
        let sections = {
            let entry = &self.topoi[self.topos];
            entry.spec.global_sections(&self.site, &entry.physics)
        };
        match sections {
            Ok(full) => self.badge_base = full.get(&binding).cloned(),
            Err(error) => self.diagnostics.push(error.to_string()),
        }
    }

    /// Rebuild the three host overlays: the active-badge control, the live
    /// morphism panel, and the demo-invariants panel.
    fn recompute_overlays(&mut self) {
        self.active_badges = self.build_active_badges();
        self.morphism_section = self.build_morphism_section();
        self.invariants_section = self.build_invariants_section();
    }

    /// Transport the active subobject across a topos switch: `∃f` on
    /// agentic→membrane, `f*` on membrane→agentic. No change within a regime.
    fn transport_active(&mut self, next: usize) {
        let from = self.topoi[self.topos].id;
        let to = self.topoi[next].id;
        if from == to {
            return;
        }
        if from == TOPOS_AGENTIC && to == TOPOS_MEMBRANE {
            self.active = self.morphism.image(&self.active);
            self.morphism_note = Some("via ∃f agentic→membrane".to_owned());
        } else if from == TOPOS_MEMBRANE && to == TOPOS_AGENTIC {
            self.active = self.morphism.pullback_subobject(&self.active);
            self.morphism_note = Some("via f* membrane→agentic".to_owned());
        }
    }

    /// Overlay the active subobject on the whole-site per-sort badge control:
    /// stamp each badge's `field` with its regime sort and mark it active.
    fn build_active_badges(&self) -> Option<Section> {
        let Some(Section::Badges(base)) = self.badge_base.as_ref() else {
            return None;
        };
        let sorts = self.active_sorts();
        let mut badges = base.clone();
        for (badge, sort) in badges.iter_mut().zip(sorts.iter()) {
            badge.field = sort.clone();
            badge.active = self.active.contains(sort);
        }
        Some(Section::Badges(badges))
    }

    /// The live morphism panel: the section of the agentic→membrane morphism
    /// against the active subobject, taken from the current regime's side.
    fn build_morphism_section(&self) -> Option<Section> {
        self.topoi[self.topos].morphism_binding.as_ref()?;
        let e = &self.topoi.iter().find(|entry| entry.id == TOPOS_AGENTIC)?.spec;
        let f = &self.topoi.iter().find(|entry| entry.id == TOPOS_MEMBRANE)?.spec;
        let side = if self.topos_id() == TOPOS_AGENTIC {
            MorphismSide::Domain
        } else {
            MorphismSide::Codomain
        };
        Some(self.morphism.section(e, f, &self.active, side))
    }

    /// The live invariants panel: three recomputed topos facts as status rows.
    fn build_invariants_section(&self) -> Option<Section> {
        self.topoi[self.topos].invariants_binding.as_ref()?;
        Some(status_table("Invariant", "Holds", &self.invariants()))
    }

    /// The three topos invariants over the demo data, recomputed live: both
    /// grammars present the same site; the physics section is regime-invariant;
    /// and the physics section commutes with restriction to the subobject `U`.
    fn invariants(&self) -> Vec<(String, bool)> {
        let same_site = self.grammars_agree;

        let agentic = self.topoi.iter().find(|entry| entry.id == TOPOS_AGENTIC);
        let membrane = self.topoi.iter().find(|entry| entry.id == TOPOS_MEMBRANE);
        let physics_regime_invariant = match (agentic, membrane) {
            (Some(a), Some(m)) => {
                a.spec.physics_section(&self.site, &a.physics)
                    == m.spec.physics_section(&self.site, &m.physics)
            }
            _ => false,
        };

        let entry = &self.topoi[self.topos];
        let subsite = self.restricted_site();
        let restricted = entry.spec.physics_section(&subsite, &entry.physics);
        let full = entry.spec.physics_section(&self.site, &entry.physics);
        let kept: BTreeSet<&str> = subsite.objects.iter().map(|object| object.id.as_str()).collect();
        let base_objects: Vec<_> = full
            .objects
            .iter()
            .filter(|(id, ..)| kept.contains(id.as_str()))
            .cloned()
            .collect();
        let base_morphisms: Vec<_> = full
            .morphisms
            .iter()
            .filter(|(domain, codomain, ..)| {
                kept.contains(domain.as_str()) && kept.contains(codomain.as_str())
            })
            .cloned()
            .collect();
        let physics_restricts =
            restricted.objects == base_objects && restricted.morphisms == base_morphisms;

        vec![
            ("trace-lines = trace-json (same site)".to_owned(), same_site),
            ("physics: agentic = membrane".to_owned(), physics_regime_invariant),
            ("physics over U = base over U".to_owned(), physics_restricts),
        ]
    }

    /// Regime object sorts present in the site, in first-appearance order — the
    /// order the badges sheaf emits them.
    fn active_sorts(&self) -> Vec<String> {
        let sorting = &self.topoi[self.topos].spec.object_sorting;
        let mut seen = BTreeSet::new();
        let mut sorts = Vec::new();
        for object in &self.site.objects {
            if let Some(sort) = sorting.get(&object.kind) {
                if seen.insert(sort.clone()) {
                    sorts.push(sort.clone());
                }
            }
        }
        sorts
    }

    /// Every object sort the current regime declares a stalk for.
    fn current_sort_set(&self) -> BTreeSet<String> {
        self.topoi[self.topos]
            .spec
            .stalks
            .objects
            .keys()
            .cloned()
            .collect()
    }

    /// Display label of the sort under the terminal focus cursor.
    fn focus_label(&self) -> Option<String> {
        let sorts = self.active_sorts();
        let sort = sorts.get(self.focus)?;
        Some(
            self.topoi[self.topos]
                .spec
                .stalks
                .objects
                .get(sort)
                .map(|stalk| stalk.label.clone())
                .unwrap_or_else(|| sort.clone()),
        )
    }
}

impl Default for ToposDemo {
    fn default() -> Self {
        Self::new()
    }
}

/// Decode a topos spec and validate its version, workspace, and sheaf bindings.
fn decode_topos(json: &str) -> Topos {
    let spec = Topos::from_json_str(json).expect("topos spec decodes");
    spec.validate().expect("topos spec validates");
    spec
}

/// The binding id of the topos's per-sort ("group by kind") badges sheaf.
fn kinds_badges_binding(spec: &Topos) -> Option<String> {
    spec.sheaves.iter().find_map(|(id, sheaf)| match sheaf {
        Sheaf::Badges(badges) if badges.group == BadgeGroup::Kinds => Some(id.clone()),
        _ => None,
    })
}

/// The binding id of the first sheaf matching `pred`, if any.
fn marker_binding(spec: &Topos, pred: impl Fn(&Sheaf) -> bool) -> Option<String> {
    spec.sheaves
        .iter()
        .find_map(|(id, sheaf)| pred(sheaf).then(|| id.clone()))
}

/// Whether the line and JSON grammars parse their canonical data to equal sites.
fn sites_agree(grammars: &[GrammarEntry]) -> bool {
    let site_of = |id| {
        grammars
            .iter()
            .find(|entry| entry.id == id)
            .and_then(|entry| entry.grammar.parse(entry.data).ok())
    };
    match (site_of(GRAMMAR_LINES), site_of(GRAMMAR_JSON)) {
        (Some(lines), Some(json)) => lines == json,
        _ => false,
    }
}

/// Join a parsed source with an append suffix on its own line; an empty suffix
/// leaves the source unchanged.
fn joined(source: &str, suffix: &str) -> String {
    if suffix.is_empty() {
        return source.to_owned();
    }
    let mut text = source.trim_end().to_owned();
    text.push('\n');
    text.push_str(suffix);
    text
}
