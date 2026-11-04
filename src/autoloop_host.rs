//! Host façade for the omp auto-loop regimes.
//!
//! Parses the loop's two inputs — the projection (`GET /api/trace`, pest
//! grammar `autoloop-trace`) and the control site (`GET /api/site`, json
//! grammar `autoloop-control`) — glues them along `repo`, and takes global
//! sections under both regimes over the glued site. An edit offered on a table
//! row becomes the bytes to `POST /api/site` via `revise` and `write_intent`;
//! the host performs the IO and reloads.
//!
//! Specs are passed in as JSON text, so this file has no build-time inputs: the
//! web example embeds them, the host-contract test reads them at run time.

use std::collections::BTreeMap;

use panel_kit_grammar::{
    BaseTopos, CompiledGrammar, EditInput, GeometricMorphism, GlobalSections, GrammarPackage, Object,
    Physics, RowEdits, Section, SectionEdit, Site, Topos,
};

/// Trace regime topos id.
pub const TRACE: &str = "topos-autoloop-trace";
/// Control regime topos id.
pub const CONTROL: &str = "topos-autoloop-control";

/// The Nix-authored JSON a host is built from.
pub struct Specs<'a> {
    /// `topos-physics`.
    pub base: &'a str,
    /// `topos-autoloop-trace`.
    pub trace_topos: &'a str,
    /// `topos-autoloop-control`.
    pub control_topos: &'a str,
    /// p_trace into the base.
    pub trace_physics: &'a str,
    /// p_control into the base.
    pub control_physics: &'a str,
    /// `autoloop-trace` grammar package.
    pub trace_grammar: &'a str,
    /// `autoloop-control` grammar package.
    pub control_grammar: &'a str,
}

struct Regime {
    topos: Topos,
    physics: Physics,
}

struct Loaded {
    site: Site,
    sections: [GlobalSections; 2],
}

/// Both regimes, both grammars, and the last site that loaded.
pub struct AutoloopHost {
    trace_grammar: CompiledGrammar,
    control_grammar: CompiledGrammar,
    regimes: [Regime; 2],
    active: usize,
    loaded: Option<Loaded>,
}

fn regime(base: &BaseTopos, topos: &str, physics: &str) -> Result<Regime, String> {
    let topos = Topos::from_json_str(topos).map_err(|e| e.to_string())?;
    topos.validate().map_err(|e| format!("{}: {e}", topos.id))?;
    let p = GeometricMorphism::from_json_str(physics).map_err(|e| e.to_string())?;
    let physics = Physics::new(base, &p, &topos).map_err(|e| format!("{}: {e}", topos.id))?;
    Ok(Regime { topos, physics })
}

fn grammar(package: &str) -> Result<CompiledGrammar, String> {
    GrammarPackage::from_json_str(package)
        .and_then(|package| package.compile())
        .map_err(|e| e.to_string())
}

impl AutoloopHost {
    /// Compile both grammars and validate both regimes and their physics.
    pub fn new(specs: &Specs<'_>) -> Result<Self, String> {
        let base = BaseTopos::from_json_str(specs.base).map_err(|e| e.to_string())?;
        Ok(Self {
            trace_grammar: grammar(specs.trace_grammar)?,
            control_grammar: grammar(specs.control_grammar)?,
            regimes: [
                regime(&base, specs.trace_topos, specs.trace_physics)?,
                regime(&base, specs.control_topos, specs.control_physics)?,
            ],
            active: 0,
            loaded: None,
        })
    }

    /// Regime ids in switcher order.
    pub fn topos_ids(&self) -> [&str; 2] {
        [TRACE, CONTROL]
    }

    /// The regime whose sections `resolve` returns.
    pub fn active_id(&self) -> &str {
        &self.regimes[self.active].topos.id
    }

    /// Make `id` the active regime; false when no regime has that id.
    pub fn set_active(&mut self, id: &str) -> bool {
        match self.regimes.iter().position(|r| r.topos.id == id) {
            Some(index) => {
                self.active = index;
                true
            }
            None => false,
        }
    }

    /// A regime's embedded workspace spec.
    pub fn workspace_json(&self, id: &str) -> Option<String> {
        self.regimes.iter().find(|r| r.topos.id == id).map(|r| r.topos.workspace_json())
    }

    /// Parse both inputs, glue, and take sections under both regimes. On error
    /// the previously loaded site stays in place.
    pub fn load(&mut self, trace: &str, control: &str) -> Result<(), String> {
        let trace = self.trace_grammar.parse(trace).map_err(|e| format!("trace: {e}"))?;
        let control = self.control_grammar.parse(control).map_err(|e| format!("control site: {e}"))?;
        let site = trace.glue(&control, "repo").map_err(|e| format!("glue: {e}"))?;
        let section = |r: &Regime| r.topos.global_sections(&site, &r.physics).map_err(|e| format!("{}: {e}", r.topos.id));
        let sections = [section(&self.regimes[0])?, section(&self.regimes[1])?];
        self.loaded = Some(Loaded { site, sections });
        Ok(())
    }

    /// Whether a site has loaded.
    pub fn is_loaded(&self) -> bool {
        self.loaded.is_some()
    }

    /// The active regime's section for a binding.
    pub fn resolve(&self, binding: &str) -> Option<&Section> {
        self.loaded.as_ref()?.sections[self.active].get(binding)
    }

    /// The object and offered edits behind each row of an editable table.
    pub fn row_edits(&self, binding: &str) -> Option<&[RowEdits]> {
        self.loaded.as_ref()?.sections[self.active].row_edits(binding)
    }

    /// An edit declared by the active regime.
    pub fn edit(&self, id: &str) -> Option<&SectionEdit> {
        self.regimes[self.active].topos.edits.get(id)
    }

    /// An object of the loaded site.
    pub fn object(&self, id: &str) -> Option<&Object> {
        self.loaded.as_ref()?.site.objects.iter().find(|o| o.id == id)
    }

    /// The bytes to write for setting `values` on `object` with the active
    /// regime's edit `edit`: the revised object as its grammar emits it.
    pub fn write(&self, edit: &str, object: &str, values: BTreeMap<String, String>) -> Result<Vec<u8>, String> {
        let loaded = self.loaded.as_ref().ok_or("nothing has loaded")?;
        let topos = &self.regimes[self.active].topos;
        let grammar = match topos.edits.get(edit).map(|e| e.grammar.as_str()) {
            Some(id) if id == self.control_grammar.id() => &self.control_grammar,
            Some(id) => return Err(format!("edit {edit} writes through {id}, which this host does not serve")),
            None => return Err(format!("no edit {edit}")),
        };
        let input = EditInput { object: object.to_owned(), values };
        let (revised, revision) = topos.revise(&loaded.site, edit, &input).map_err(|e| e.to_string())?;
        if revision.support.is_empty() {
            return Err("nothing changed".to_owned());
        }
        let intent = topos.write_intent(grammar, &revised, &revision).map_err(|e| e.to_string())?;
        Ok(intent.bytes)
    }
}
