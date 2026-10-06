//! Shared trio demo host for the ratatui native and browser backends.
//!
//! One site (the agent sessions and the OKF knowledge objects, joined by
//! `produces`/`verifies`) read in three regimes (agentic, membrane,
//! okf-catalog) related by the morphisms `f`, `ψ`, `χ`, and `ω`; the host
//! owns each regime's resolved `WorkspaceSpec`, its persisted panel layout, the
//! live graph positions, and the header strip that drives the regime and the
//! grammar. Entry hosts own only input plumbing and terminal setup.
//!
//! Keys: `t` next regime, `g` grammar, `n`/`p` next and previous node
//! selection, `l` re-solve the layout, `r` reset selection and re-solve. A
//! regime switch recolours the graph and keeps the positions — the physics is
//! one pullback, so nothing moves — while a grammar switch re-reads the site and
//! re-solves seeded from the positions on screen.
#![allow(dead_code)]

mod graph_layout;
mod trio_host;

use std::fmt;

use graph_layout::{project_2d, solve, Tween};
use panel_kit_core::frame::{
    project_into, ChromeProjectionInput, FrameStatus, ProjectionBuffer, ProjectionInput,
    TileLayoutMetrics,
};
use panel_kit_core::persist::{
    apply_save_decision, layout_key_for_spec, restore_snapshot, switch_layout, LayoutError,
    LayoutStore, RestoreContext,
};
use panel_kit_core::reducer::{reduce, ReduceContext, Snapshot, Viewport, WorkspaceEvent};
#[cfg(target_arch = "wasm32")]
use panel_kit_core::reducer::WheelDisposition;
use panel_kit_core::spec::{BackendKind, BindingManifest, ResolvedWorkspace, WorkspaceSpec};
use panel_kit_core::widgets::graph::{GraphModel, GraphView};
use panel_kit_core::widgets::table::TableView;
use panel_kit_core::widgets::ContentSpec;
use panel_kit_core::{
    ChromeSpec, Charset as SpecCharset, FocusContext, Key, KeyChord, SnapPolicy, SpecPanelId,
    SurfaceCapabilities, SurfaceProfile,
};
use panel_kit_grammar::Section;
use panel_kit_tui::badge;
use panel_kit_tui::charts::gauges;
use panel_kit_tui::input::{route_wheel_to, workspace_event_from_key, WheelDelta};
use panel_kit_tui::scroll::PanelScroll;
#[cfg(not(target_arch = "wasm32"))]
use panel_kit_tui::store::JsonFileLayoutStore;
use panel_kit_tui::widgets::{self, TuiHitBuffer};
use panel_kit_tui::{rect_from_region, Charset, ResolvedTuiTheme};
use ratatui::layout::{Position, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use trio_host::TrioSpecs;

/// The Nix-authored JSON the trio's one site and three regimes are built from.
const AGENTIC: &str = include_str!(env!("PANEL_KIT_TRIO_AGENTIC"));
const MEMBRANE: &str = include_str!(env!("PANEL_KIT_TRIO_MEMBRANE"));
const OKF_CATALOG: &str = include_str!(env!("PANEL_KIT_TRIO_OKF_CATALOG"));
const BASE: &str = include_str!(env!("PANEL_KIT_TRIO_BASE"));
const MORPHISM_F: &str = include_str!(env!("PANEL_KIT_TRIO_F"));
const MORPHISM_PSI: &str = include_str!(env!("PANEL_KIT_TRIO_PSI"));
const MORPHISM_CHI: &str = include_str!(env!("PANEL_KIT_TRIO_CHI"));
const AGENTIC_PHYSICS: &str = include_str!(env!("PANEL_KIT_TRIO_AGENTIC_PHYSICS"));
const MEMBRANE_PHYSICS: &str = include_str!(env!("PANEL_KIT_TRIO_MEMBRANE_PHYSICS"));
const CATALOG_PHYSICS: &str = include_str!(env!("PANEL_KIT_TRIO_OKF_CATALOG_PHYSICS"));

const GRAMMAR_LINES: &str = include_str!(env!("PANEL_KIT_TRIO_GRAMMAR_LINES"));
const GRAMMAR_JSON: &str = include_str!(env!("PANEL_KIT_TRIO_GRAMMAR_JSON"));
const DATA_LINES: &str = include_str!(env!("PANEL_KIT_TRIO_DATA_LINES"));
const DATA_JSON: &str = include_str!(env!("PANEL_KIT_TRIO_DATA_JSON"));
const MORPHISM_OMEGA: &str = include_str!(env!("PANEL_KIT_TRIO_OMEGA"));


/// Re-exported so an entry host can name the three regimes without reaching
/// into the host module itself.
pub use trio_host::{RegimeId, TrioHost};

/// Rows the header strip occupies above the projected workspace: the status
/// line over the control strip, so the routes have a full width to read.
pub const HEADER_ROWS: u16 = 2;
/// Milliseconds the graph takes to walk to a re-solved layout.
pub const TRANSITION_MS: u32 = 800;

/// Failure decoding a trio spec or restoring its layout.
#[derive(Debug)]
pub enum HostError {
    /// The trio's regimes, morphisms, or grammars did not load.
    Trio(String),
    /// Strict spec decoding or provider resolution failed.
    Spec(String),
}

impl fmt::Display for HostError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Trio(message) | Self::Spec(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for HostError {}

/// A demo key intent, translated from a platform key event by the entry host.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DemoKey {
    /// `t`: enter the next regime, in header order.
    NextRegime,
    /// `g`: re-read the site with its next grammar.
    Grammar,
    /// `n` / `p`: advance the node selection.
    Selection(isize),
    /// `l`: re-solve the layout from scratch.
    Solve,
    /// `r`: clear the selection and re-solve.
    Reset,
    /// Page scroll the focused panel body, in rows.
    Page(f64),
    /// Any other key, reduced through the spec's input bindings.
    Chord(KeyChord),
    /// `q` on the native entry host.
    #[cfg(not(target_arch = "wasm32"))]
    Quit,
}

impl DemoKey {
    /// Classify a renderer-neutral key chord; page keys are entry-host policy.
    pub fn from_chord(chord: KeyChord) -> Self {
        if let Key::Char(ch) = chord.key {
            match ch {
                't' => return Self::NextRegime,
                'g' => return Self::Grammar,
                'n' => return Self::Selection(1),
                'p' => return Self::Selection(-1),
                'l' => return Self::Solve,
                'r' => return Self::Reset,
                _ => {}
            }
        }
        Self::Chord(chord)
    }
}

/// One panel body painted by the most recent frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BodyPaint {
    /// Projected panel key.
    pub key: SpecPanelId,
    /// Body rectangle the painter wrote into.
    pub rect: Rect,
    /// Whether the panel's authored binding resolved to demo content.
    pub bound: bool,
}

/// One header control: pre-measured text plus the action it triggers. The
/// grammar is not one: the status line names it and `g` cycles it, and the
/// Grammar panel shows the grammar that read the site.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeaderAction {
    /// Read the site in the named regime.
    SetRegime(RegimeId),
    /// Re-solve the layout.
    Solve,
    /// Clear the selection and re-solve.
    Reset,
}

#[derive(Clone)]
struct HeaderItem {
    text: String,
    width: u16,
    action: HeaderAction,
    active: bool,
}

impl HeaderItem {
    fn new(label: &str, action: HeaderAction) -> Self {
        Self {
            text: format!("[{label}]"),
            width: label.chars().count() as u16 + 2,
            action,
            active: false,
        }
    }
}

#[derive(Clone, Copy)]
struct BodyJob {
    body: Rect,
    key: SpecPanelId,
}

/// One regime: its resolved spec and its persisted layout.
struct Scene {
    resolved: ResolvedWorkspace,
    snapshot: Snapshot<SpecPanelId>,
    store: Option<Box<dyn LayoutStore>>,
}

/// Demo host owning the three regime scenes, the live graph, and the header.
pub struct TrioApp {
    host: TrioHost,
    scenes: Vec<Scene>,
    active: usize,
    model: GraphModel,
    positions: Vec<[f32; 3]>,
    tween: Option<(Tween, f64)>,
    theme: ResolvedTuiTheme,
    charset: Charset,
    scroll: PanelScroll<SpecPanelId>,
    hits: TuiHitBuffer<SpecPanelId>,
    clip: widgets::ClipScratch,
    projection: ProjectionBuffer<SpecPanelId>,
    jobs: Vec<BodyJob>,
    paints: Vec<BodyPaint>,
    live_keys: Vec<SpecPanelId>,
    header_items: Vec<HeaderItem>,
    header_zones: Vec<(Rect, HeaderAction)>,
    graph_rect: Rect,
    reduced_motion: bool,
    grid: (f64, f64),
}

impl TrioApp {
    /// Load the site and all three regimes, and read it in agentic.
    pub fn new(backend: BackendKind, reduced_motion: bool) -> Result<Self, HostError> {
        let specs = TrioSpecs {
            agentic: AGENTIC,
            membrane: MEMBRANE,
            okf_catalog: OKF_CATALOG,
            base: BASE,
            f: MORPHISM_F,
            psi: MORPHISM_PSI,
            chi: MORPHISM_CHI,
            omega: MORPHISM_OMEGA,
            agentic_physics: AGENTIC_PHYSICS,
            membrane_physics: MEMBRANE_PHYSICS,
            catalog_physics: CATALOG_PHYSICS,
            trace_lines_grammar: GRAMMAR_LINES,
            trace_lines_data: DATA_LINES,
            trace_json_grammar: GRAMMAR_JSON,
            trace_json_data: DATA_JSON,
        };
        let mut host = TrioHost::load(&specs).map_err(HostError::Trio)?;

        let mut scenes = Vec::with_capacity(RegimeId::ALL.len());
        for regime in RegimeId::ALL {
            host.set_regime(regime);
            let spec = WorkspaceSpec::from_json_str(&host.workspace_json())
                .map_err(|error| HostError::Spec(format!("{regime}: {error}")))?;
            let manifest = BindingManifest {
                backend,
                panels: spec
                    .panels
                    .iter()
                    .map(|panel| panel.provider_declaration())
                    .collect(),
            };
            let resolved = spec
                .resolve(&manifest)
                .map_err(|error| HostError::Spec(format!("{regime}: {error}")))?;
            let store = make_store(&resolved);
            let snapshot = restored(&resolved, store.as_deref());
            scenes.push(Scene { resolved, snapshot, store });
        }

        // Scenes are pushed in `RegimeId::ALL` order, so agentic is the first.
        host.set_regime(RegimeId::Agentic);
        let active = 0;
        let model = host.graph();
        let positions = solve(&model, &host.graph_layout(), None);
        let capacity = scenes
            .iter()
            .map(|scene| scene.resolved.catalog.len())
            .max()
            .unwrap_or(0);
        let app = Self {
            theme: ResolvedTuiTheme::from(&scenes[0].resolved.theme),
            charset: spec_charset(scenes[0].resolved.glyphs),
            scroll: PanelScroll::with_capacity(capacity),
            hits: TuiHitBuffer::with_capacity(capacity, capacity),
            clip: widgets::ClipScratch::default(),
            projection: ProjectionBuffer::with_panel_capacity(capacity),
            jobs: Vec::with_capacity(capacity),
            paints: Vec::with_capacity(capacity),
            live_keys: Vec::with_capacity(capacity),
            header_zones: Vec::with_capacity(capacity),
            graph_rect: Rect::ZERO,
            grid: (1.0, 1.0),
            reduced_motion,
            header_items: header_items(),
            positions,
            tween: None,
            host,
            scenes,
            active,
            model,
        };
        Ok(app)
    }

    /// The active regime.
    pub fn regime(&self) -> RegimeId {
        self.host.regime()
    }

    /// Per native regime, the objects it sorts and the route into the active
    /// regime.
    pub fn route_summary(&self) -> String {
        self.host.route_summary()
    }

    /// Bindings of the active regime that cannot read the site.
    pub fn unreadable(&self) -> Vec<String> {
        self.host.unreadable().into_iter().map(str::to_owned).collect()
    }

    /// The grammar reading the site.
    pub fn grammar_id(&self) -> &'static str {
        self.host.grammar_id()
    }

    /// The selected node id, if any.
    pub fn selected(&self) -> Option<&str> {
        self.host.selected()
    }

    /// Authored panel titles of the active regime, in catalog order.
    pub fn panel_titles(&self) -> Vec<String> {
        self.scenes[self.active]
            .resolved
            .panels
            .iter()
            .map(|panel| panel.title.clone())
            .collect()
    }

    /// Panel bodies painted by the most recent frame.
    pub fn paints(&self) -> &[BodyPaint] {
        &self.paints
    }

    /// Body rectangle the panel with authored id `id` was painted into by the
    /// last frame, so a host can read one panel's text back out of the buffer.
    pub fn body_rect(&self, id: &str) -> Option<Rect> {
        let scene = &self.scenes[self.active];
        let index = scene.resolved.panels.iter().position(|panel| panel.id == id)?;
        let key = scene.snapshot.panels.get(index)?.kind;
        self.paints.iter().find(|paint| paint.key == key).map(|paint| paint.rect)
    }

    /// Body rectangle the graph panel was painted into by the last frame.
    pub fn graph_rect(&self) -> Rect {
        self.graph_rect
    }

    /// The live positions, projected onto two axes.
    pub fn positions_2d(&self) -> Vec<[f32; 2]> {
        project_2d(&self.positions)
    }

    /// The node ids of the active graph, in declaration order.
    pub fn node_ids(&self) -> Vec<String> {
        self.model.nodes.iter().map(|node| node.id.clone()).collect()
    }

    /// The node colours of the active graph, as sRGB channels.
    pub fn node_colors(&self) -> Vec<[u8; 3]> {
        self.model
            .nodes
            .iter()
            .map(|node| {
                let (r, g, b) = node.color;
                [r, g, b]
            })
            .collect()
    }

    /// The node id at `index` of the active graph.
    pub fn node_id(&self, index: usize) -> Option<&str> {
        self.model.nodes.get(index).map(|node| node.id.as_str())
    }

    /// One line per assembly component of the active regime: its stage, its
    /// representative object, and its object and morphism counts.
    pub fn assembly_labels(&self) -> Vec<String> {
        self.host
            .assembly()
            .into_iter()
            .map(|component| {
                format!(
                    "{} · {} · {} objects {} morphisms",
                    self.host.stage_label(component.stage),
                    component.representative,
                    component.v,
                    component.e
                )
            })
            .collect()
    }

    /// Live canvas size in terminal cells, for browser wheel translation.
    #[cfg(target_arch = "wasm32")]
    pub fn grid(&self) -> (f64, f64) {
        self.grid
    }

    /// Apply one demo key intent at wall-clock `now_ms`; true asks the entry
    /// host to exit.
    pub fn handle_key(&mut self, key: DemoKey, now_ms: f64) -> Result<bool, LayoutError> {
        match key {
            DemoKey::NextRegime => self.next_regime(now_ms),
            DemoKey::Grammar => self.cycle_grammar(now_ms),
            DemoKey::Selection(step) => self.host.cycle_selection(step),
            DemoKey::Solve => self.relayout(now_ms, false),
            DemoKey::Reset => {
                self.host.select(None);
                self.relayout(now_ms, false);
            }
            DemoKey::Page(rows) => self.page_scroll(rows)?,
            DemoKey::Chord(chord) => {
                self.reduce(workspace_event_from_key(chord, FocusContext::Workspace))?;
            }
            #[cfg(not(target_arch = "wasm32"))]
            DemoKey::Quit => return Ok(true),
        }
        Ok(false)
    }

    /// Enter the next regime, in header order.
    pub fn next_regime(&mut self, now_ms: f64) {
        let current = self.host.regime();
        let index = RegimeId::ALL.iter().position(|regime| *regime == current).unwrap_or(0);
        self.enter_regime(RegimeId::ALL[(index + 1) % RegimeId::ALL.len()], now_ms);
    }

    /// Re-read the site with its next grammar and re-solve from the positions
    /// on screen.
    pub fn cycle_grammar(&mut self, now_ms: f64) {
        self.host.cycle_grammar();
        self.relayout(now_ms, true);
    }

    /// Advance the running layout tween to wall-clock `now_ms`.
    pub fn advance(&mut self, now_ms: f64) {
        let Some((tween, started)) = self.tween.as_ref() else {
            return;
        };
        let elapsed_ms = (now_ms - started).max(0.0) as u32;
        if tween.is_complete(elapsed_ms) {
            self.positions = tween.at(u32::MAX);
            self.tween = None;
        } else {
            self.positions = tween.at(elapsed_ms);
        }
    }

    /// Paint one frame: header strip, workspace chrome, and bound bodies.
    pub fn draw(&mut self, frame: &mut ratatui::Frame) -> Result<(), LayoutError> {
        let area = frame.area();
        self.hits.clear();
        self.paints.clear();
        self.header_zones.clear();
        self.jobs.clear();
        self.graph_rect = Rect::ZERO;
        if area.height <= HEADER_ROWS || area.width < 8 {
            return Ok(());
        }
        let workspace = Rect::new(
            area.x,
            area.y + HEADER_ROWS,
            area.width,
            area.height - HEADER_ROWS,
        );
        self.sync_viewport(workspace)?;

        let surface = self.surface_profile();
        let tile = self.tile_metrics(surface);
        let chrome = chrome_input(&self.scenes[self.active].resolved.chrome);
        let projected = project_into(
            ProjectionInput {
                snapshot: &self.scenes[self.active].snapshot,
                surface,
                chrome: &chrome,
                clamp: &self.scenes[self.active].resolved.layout.clamp,
                tile: &tile,
            },
            &mut self.projection,
        );
        widgets::root::draw_root(frame, area, &self.theme, self.charset);
        if projected.status == FrameStatus::TooSmall {
            frame.render_widget(
                Paragraph::new("resize").style(Style::default().fg(self.theme.dim)),
                area,
            );
            return Ok(());
        }

        let clip = rect_from_region(projected.chrome.workspace);
        let scene = &self.scenes[self.active];
        let (theme, charset) = (&self.theme, self.charset);
        let (hits, jobs) = (&mut self.hits, &mut self.jobs);
        widgets::paint_clipped(frame, clip, &mut self.clip, |frame| {
            for panel in projected.panels.iter().copied() {
                let Some(meta) = scene.resolved.catalog.get(panel.key) else {
                    continue;
                };
                widgets::panel::draw_panel_surface(frame, panel, theme, charset, hits);
                let body = widgets::panel::draw_panel_chrome(frame, panel, meta, theme, charset, hits);
                widgets::panel::draw_traffic_lights(frame, panel, projected.mode, None, theme, charset, hits);
                widgets::panel::draw_resize_grip(frame, panel, None, theme, hits);
                let body = body.intersection(clip);
                if body.width > 0 && body.height > 0 {
                    jobs.push(BodyJob { body, key: panel.key });
                }
            }
        });
        self.hits.clip_panels(clip);
        widgets::dock::draw_dock(
            frame,
            rect_from_region(projected.chrome.dock),
            projected.dock,
            widgets::dock::DockRenderContext {
                catalog: &self.scenes[self.active].resolved.catalog,
                label: &self.scenes[self.active].resolved.chrome.dock_label,
                theme: &self.theme,
                charset: self.charset,
            },
            &mut self.hits,
        );
        widgets::root::draw_workspace_scrollbar(frame, &projected, &self.theme);
        self.paint_bodies(frame);
        self.draw_header(frame, Rect::new(area.x, area.y, area.width, HEADER_ROWS));
        Ok(())
    }

    /// Apply the header or graph control under `at`; true when consumed.
    pub fn press(&mut self, at: Position, now_ms: f64) -> bool {
        if let Some((_, action)) = self
            .header_zones
            .iter()
            .rev()
            .find(|(rect, _)| rect.contains(at))
            .map(|(rect, action)| (*rect, *action))
        {
            self.run_header(action, now_ms);
            return true;
        }
        if !self.graph_rect.contains(at) {
            return false;
        }
        let column = at.x.saturating_sub(self.graph_rect.x);
        let row = at.y.saturating_sub(self.graph_rect.y);
        let Some(index) =
            panel_kit_tui::graph::node_at(self.graph_rect, &self.positions_2d(), (column, row))
        else {
            return false;
        };
        if let Some(id) = self.node_id(index).map(str::to_owned) {
            self.host.select(Some(&id));
        }
        true
    }

    /// Reduce a non-wheel pointer event against the last frame's hit regions.
    #[cfg(target_arch = "wasm32")]
    pub fn pointer_event(
        &mut self,
        event: panel_kit_core::PointerEvent,
    ) -> Result<(), LayoutError> {
        let Some(event) = panel_kit_tui::input::workspace_event_from_pointer(&self.hits, event)
        else {
            return Ok(());
        };
        self.reduce(event)
    }

    /// Route one wheel gesture; true when a panel body or the workspace moved.
    #[cfg(target_arch = "wasm32")]
    pub fn wheel(&mut self, at: (f64, f64), delta: WheelDelta) -> bool {
        let before = self.scenes[self.active].snapshot.workspace_scroll;
        let event = panel_kit_tui::input::route_wheel(&self.hits, &mut self.scroll, at, delta);
        let consumed = matches!(
            event,
            WorkspaceEvent::Wheel {
                disposition: WheelDisposition::ContentConsumed,
                ..
            }
        );
        let _ = self.reduce(event);
        consumed || self.scenes[self.active].snapshot.workspace_scroll != before
    }

    /// Run one header control: read a regime, re-solve, or reset.
    fn run_header(&mut self, action: HeaderAction, now_ms: f64) {
        match action {
            HeaderAction::SetRegime(regime) => self.enter_regime(regime, now_ms),
            HeaderAction::Solve => self.relayout(now_ms, false),
            HeaderAction::Reset => {
                self.host.select(None);
                self.relayout(now_ms, false);
            }
        }
    }

    /// Enter `regime`. The physics is one pullback, so the arrangement is
    /// kept: the regime only re-sorts and re-colours the same nodes.
    fn enter_regime(&mut self, regime: RegimeId, _now_ms: f64) {
        self.host.set_regime(regime);
        if let Some(index) = self.scene_index(regime) {
            if index != self.active {
                self.swap_scene(index);
            }
        }
        self.model = self.host.graph();
    }

    fn scene_index(&self, regime: RegimeId) -> Option<usize> {
        RegimeId::ALL
            .iter()
            .position(|candidate| *candidate == regime)
            .filter(|index| *index < self.scenes.len())
    }

    /// Carry the old regime's panel layout over to `index`'s panels where they
    /// match, adopting its spec, glyph family, and theme.
    fn swap_scene(&mut self, index: usize) {
        let viewport = (
            self.scenes[self.active].snapshot.viewport.width,
            self.scenes[self.active].snapshot.viewport.height,
        );
        let context = RestoreContext {
            units: self.scenes[index].resolved.layout.units,
            viewport,
        };
        let defaults = self.scenes[index].resolved.initial.clone();
        let switched = match (
            self.scenes[self.active].store.as_deref(),
            self.scenes[index].store.as_deref(),
        ) {
            (Some(old), Some(new)) => switch_layout(
                old,
                &self.scenes[self.active].snapshot,
                &self.scenes[self.active].resolved.catalog,
                new,
                defaults,
                &self.scenes[index].resolved.catalog,
                context,
            )
            .unwrap_or_else(|_| self.scenes[index].resolved.initial.clone()),
            _ => self.scenes[index].resolved.initial.clone(),
        };
        self.scenes[index].snapshot = switched;
        self.active = index;
        self.charset = spec_charset(self.scenes[index].resolved.glyphs);
        self.theme = ResolvedTuiTheme::from(&self.scenes[index].resolved.theme);
        self.scroll.clear();
    }

    /// Re-read the graph and solve it, from the positions on screen when
    /// `seeded`, and tween to the result.
    fn relayout(&mut self, now_ms: f64, seeded: bool) {
        self.model = self.host.graph();
        let seed = seeded.then_some(self.positions.as_slice());
        let target = solve(&self.model, &self.host.graph_layout(), seed);
        let from = std::mem::take(&mut self.positions);
        let tween = Tween::new(from, target, TRANSITION_MS, self.reduced_motion);
        if tween.is_complete(0) {
            self.positions = tween.at(0);
            self.tween = None;
            return;
        }
        self.tween = Some((tween, now_ms));
    }

    /// Paint every projected body: the graph panel from the live positions,
    /// every other panel from the section its binding resolved to.
    fn paint_bodies(&mut self, frame: &mut ratatui::Frame) {
        let positions = self.positions_2d();
        let graph_binding = self.host.regime().graph_binding();
        for job in self.jobs.iter().copied() {
            let Some(spec) = self.scenes[self.active]
                .resolved
                .panels
                .get(job.key.index() as usize)
            else {
                continue;
            };
            let is_graph = matches!(spec.content, ContentSpec::External { .. })
                && spec.content.binding_id() == Some(graph_binding);
            if is_graph {
                self.graph_rect = job.body;
                panel_kit_tui::graph::graph(
                    frame,
                    job.body,
                    &self.theme,
                    GraphView {
                        nodes: &self.model.nodes,
                        edges: &self.model.edges,
                    },
                    &positions,
                    self.host.selected(),
                );
                self.paints.push(BodyPaint { key: job.key, rect: job.body, bound: true });
                continue;
            }
            let content = self.host.resolve(spec.content.binding_id().unwrap_or_default());
            paint_body(
                frame,
                job.body,
                &job.key,
                &spec.content,
                content,
                &self.theme,
                &mut self.scroll,
            );
            self.paints.push(BodyPaint {
                key: job.key,
                rect: job.body,
                bound: content.is_some(),
            });
        }
        self.live_keys.clear();
        self.live_keys.extend(
            self.scenes[self.active]
                .snapshot
                .panels
                .iter()
                .map(|window| window.kind),
        );
        self.scroll.retain_panels(&self.live_keys);
    }

    /// Paint the header: the status line over the demo controls, so the routes
    /// have a full width to read.
    fn draw_header(&mut self, frame: &mut ratatui::Frame, area: Rect) {
        let theme = self.theme;
        let regime = self.host.regime();
        for item in &mut self.header_items {
            item.active = match item.action {
                HeaderAction::SetRegime(id) => id == regime,
                _ => false,
            };
        }

        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                self.status_line(),
                Style::default().fg(theme.fg),
            ))),
            Rect::new(area.x, area.y, area.width, 1),
        );

        let row = Rect::new(area.x, area.y + 1, area.width, 1);
        let mut shown: u16 = self.header_items.iter().map(|item| item.width + 1).sum();
        let mut first = 0;
        while shown > row.width && first < self.header_items.len() {
            shown -= self.header_items[first].width + 1;
            first += 1;
        }

        let mut x = row.right();
        for index in (first..self.header_items.len()).rev() {
            let item = &self.header_items[index];
            let rect = Rect::new(x.saturating_sub(item.width), row.y, item.width, 1);
            let style = if item.active {
                Style::default()
                    .fg(theme.inverse_fg)
                    .bg(theme.accent)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.dim)
            };
            frame.render_widget(
                Paragraph::new(Line::from(Span::styled(item.text.as_str(), style))),
                rect,
            );
            self.header_zones.push((rect, item.action));
            x = rect.x.saturating_sub(1);
        }

        // The key hints take whatever the controls leave, so a narrow terminal
        // loses them rather than the controls.
        let keys = Rect::new(row.x, row.y, x.saturating_sub(row.x), 1);
        if keys.width > 0 {
            frame.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    "t regime · g grammar · n/p node · l solve · r reset",
                    Style::default().fg(theme.dim),
                ))),
                keys,
            );
        }
    }

    /// The active regime, per native regime the route its kinds take and how
    /// many objects that covers, the grammar, the graph size, and the panels
    /// that cannot read the site, if any. The keys ride the control strip.
    pub fn status_line(&self) -> String {
        let selection = self.host.selected().map_or_else(String::new, |id| format!(" · sel {id}"));
        let unreadable = match self.host.unreadable().len() {
            0 => String::new(),
            count => format!(" · {count} panels cannot read the site"),
        };
        format!(
            "{} · {} · {} · {} nodes · {} edges{selection}{unreadable}",
            self.host.regime(),
            self.host.route_summary(),
            self.host.grammar_id(),
            self.model.nodes.len(),
            self.model.edges.len(),
        )
    }

    /// Reduce one workspace event and apply the spec save policy.
    fn reduce(&mut self, event: WorkspaceEvent<SpecPanelId>) -> Result<(), LayoutError> {
        let surface = self.surface_profile();
        let scene = &mut self.scenes[self.active];
        let context = ReduceContext {
            surface,
            clamp: &scene.resolved.layout.clamp,
            command_step: scene.resolved.input.steps,
            tile: &scene.resolved.layout.tile.resize,
            snap: SnapPolicy::CELLS,
        };
        let reduction = reduce(&mut scene.snapshot, event, context);
        let Some(store) = scene.store.as_deref() else {
            return Ok(());
        };
        let decision = scene.resolved.persistence.save_policy.decide(&reduction);
        apply_save_decision(decision, store, &scene.snapshot, &scene.resolved.catalog)
    }

    /// Scroll the focused panel body by a page, else the workspace.
    fn page_scroll(&mut self, rows: f64) -> Result<(), LayoutError> {
        let event = match self.scenes[self.active].snapshot.focused {
            Some(key) => route_wheel_to(&mut self.scroll, &key, WheelDelta::new(0.0, rows)),
            None => WorkspaceEvent::Wheel {
                delta_y: rows,
                disposition: panel_kit_core::reducer::WheelDisposition::BubbleToWorkspace,
            },
        };
        self.reduce(event)
    }

    /// Publish a viewport change when the surface size moved.
    fn sync_viewport(&mut self, area: Rect) -> Result<(), LayoutError> {
        self.grid = (f64::from(area.width), f64::from(area.height));
        let size = Viewport {
            width: area.width.into(),
            height: area.height.into(),
            units: self.scenes[self.active].resolved.layout.units,
        };
        if self.scenes[self.active].snapshot.viewport == size {
            return Ok(());
        }
        let policy = self.scenes[self.active].resolved.surface.resize_policy;
        self.reduce(WorkspaceEvent::ViewportChanged { size, policy })
    }

    fn surface_profile(&self) -> SurfaceProfile {
        let scene = &self.scenes[self.active];
        SurfaceProfile::from_logical_width(
            scene.snapshot.viewport.width,
            scene.resolved.surface.compact_max,
            scene.resolved.surface.tablet_max,
            SurfaceCapabilities {
                coarse_pointer: false,
                hover: true,
                keyboard: true,
            },
        )
    }

    fn tile_metrics(&self, surface: SurfaceProfile) -> TileLayoutMetrics {
        let tile = &self.scenes[self.active].resolved.layout.tile;
        TileLayoutMetrics {
            resize: tile.resize,
            columns: surface.tile_columns(),
            row_min: tile.row_min,
            gap: tile.gap,
            padding: tile.padding,
            fill_viewport: tile.fill_viewport,
            fill_order: panel_kit_core::frame::TileFillOrder::RowMajor,
        }
    }
}

/// The header controls: the three regimes and the two layout controls.
fn header_items() -> Vec<HeaderItem> {
    let mut items: Vec<HeaderItem> = RegimeId::ALL
        .into_iter()
        .map(|regime| HeaderItem::new(regime.label(), HeaderAction::SetRegime(regime)))
        .collect();
    items.push(HeaderItem::new("solve", HeaderAction::Solve));
    items.push(HeaderItem::new("reset", HeaderAction::Reset));
    items
}

/// Paint one panel body from the section its authored binding resolved to.
#[allow(clippy::too_many_arguments)]
fn paint_body(
    frame: &mut ratatui::Frame,
    body: Rect,
    key: &SpecPanelId,
    spec: &ContentSpec,
    content: Option<&Section>,
    theme: &ResolvedTuiTheme,
    scroll: &mut PanelScroll<SpecPanelId>,
) {
    let Some(content) = content else {
        let binding = spec.binding_id().unwrap_or("<no binding>");
        frame.render_widget(
            Paragraph::new(format!("unbound: {binding}")).style(Style::default().fg(theme.red)),
            body,
        );
        return;
    };
    match content {
        Section::Text(text) => {
            panel_kit_tui::scroll::lines(
                frame,
                body,
                theme,
                key,
                scroll,
                wrapped_lines(&text.text, body.width, theme),
            );
        }
        Section::Table(model) => {
            panel_kit_tui::table::table(
                frame,
                body,
                theme,
                key,
                scroll,
                TableView {
                    columns: &model.columns,
                    rows: &model.rows,
                },
            );
        }
        Section::Badges(specs) => {
            let rows: Vec<Line<'_>> = specs
                .iter()
                .map(|badge_spec| Line::from(badge::spans(badge_spec, theme)))
                .collect();
            panel_kit_tui::scroll::lines(frame, body, theme, key, scroll, rows);
        }
        Section::Gauges(models) => gauges(frame, body, theme, models),
        Section::Meter(model) => frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(model.label.as_str(), Style::default().fg(theme.fg)),
                Span::raw(" "),
                panel_kit_tui::meter::span_model(model, meter_width(body.width)),
                Span::raw(" "),
                Span::styled(model.text.as_str(), Style::default().fg(theme.dim)),
            ])),
            body,
        ),
        _ => {}
    }
}

fn meter_width(area_width: u16) -> usize {
    usize::from(area_width).saturating_sub(22).clamp(6, 24)
}

/// Wrap body text to the panel width, keeping the authored indentation.
fn wrapped_lines(text: &str, width: u16, theme: &ResolvedTuiTheme) -> Vec<Line<'static>> {
    let width = usize::from(width.max(1));
    let style = Style::default().fg(theme.fg);
    let mut lines = Vec::new();
    for raw in text.lines() {
        let body = raw.trim_start_matches(' ');
        let indent = " ".repeat(raw.len() - body.len());
        let limit = width.saturating_sub(indent.len()).max(1);
        let mut current = indent.clone();
        if !body.is_empty() {
            let mut filled = 0_usize;
            for word in body.split_whitespace() {
                for (index, piece) in chunks(word, limit).iter().enumerate() {
                    let space = usize::from(filled > 0 && index == 0);
                    let cells = piece.chars().count();
                    if filled > 0 && filled + space + cells > limit {
                        lines.push(Line::from(Span::styled(std::mem::take(&mut current), style)));
                        current.clone_from(&indent);
                        filled = 0;
                    }
                    if space > 0 {
                        current.push(' ');
                        filled += 1;
                    }
                    current.push_str(piece);
                    filled += cells;
                }
            }
        }
        lines.push(Line::from(Span::styled(current, style)));
    }
    lines
}

/// Split `text` into pieces of at most `width` characters.
fn chunks(text: &str, width: usize) -> Vec<&str> {
    let mut pieces = Vec::new();
    let mut rest = text;
    while rest.chars().count() > width {
        let split = rest
            .char_indices()
            .nth(width)
            .map_or(text.len(), |(index, _)| index);
        pieces.push(&rest[..split]);
        rest = &rest[split..];
    }
    pieces.push(rest);
    pieces
}

fn restored(resolved: &ResolvedWorkspace, store: Option<&dyn LayoutStore>) -> Snapshot<SpecPanelId> {
    let defaults = resolved.initial.clone();
    if !resolved.persistence.restore {
        return defaults;
    }
    let Some(store) = store else {
        return defaults;
    };
    let context = RestoreContext {
        units: resolved.layout.units,
        viewport: (
            resolved.initial.viewport.width,
            resolved.initial.viewport.height,
        ),
    };
    restore_snapshot(store, defaults, &resolved.catalog, context)
        .unwrap_or_else(|_| resolved.initial.clone())
}

#[cfg(not(target_arch = "wasm32"))]
fn make_store(resolved: &ResolvedWorkspace) -> Option<Box<dyn LayoutStore>> {
    if !resolved.persistence.enabled {
        return None;
    }
    let key = layout_key_for_spec(&resolved.persistence.key, &resolved.id);
    let name = key.replace(['/', '\\'], "_");
    Some(Box::new(JsonFileLayoutStore::new(
        std::env::temp_dir().join(format!("panel-kit-trio-{name}.json")),
    )))
}

#[cfg(target_arch = "wasm32")]
fn make_store(resolved: &ResolvedWorkspace) -> Option<Box<dyn LayoutStore>> {
    if !resolved.persistence.enabled {
        return None;
    }
    let key = layout_key_for_spec(&resolved.persistence.key, &resolved.id);
    Some(Box::new(storage::BrowserLocalStorage::new(key)))
}

#[cfg(target_arch = "wasm32")]
mod storage {
    use panel_kit_core::persist::LayoutStore;
    use ratzilla::web_sys::js_sys::{Function, Reflect};
    use ratzilla::web_sys::wasm_bindgen::{JsCast, JsValue};

    /// Browser `localStorage` adapter for the core layout store contract.
    pub struct BrowserLocalStorage {
        key: String,
    }

    impl BrowserLocalStorage {
        /// Create a store bound to one namespaced layout key.
        pub fn new(key: impl Into<String>) -> Self {
            Self { key: key.into() }
        }

        fn storage() -> Result<JsValue, String> {
            let window = ratzilla::web_sys::window().ok_or("window unavailable")?;
            Reflect::get(window.as_ref(), &JsValue::from_str("localStorage"))
                .map_err(|error| format!("{error:?}"))
        }

        fn method(storage: &JsValue, name: &str) -> Result<Function, String> {
            Reflect::get(storage, &JsValue::from_str(name))
                .map_err(|error| format!("{error:?}"))?
                .dyn_into::<Function>()
                .map_err(|_| format!("localStorage.{name} is not callable"))
        }
    }

    impl LayoutStore for BrowserLocalStorage {
        fn load(&self) -> Result<Option<String>, String> {
            let storage = Self::storage()?;
            let value = Self::method(&storage, "getItem")?
                .call1(&storage, &JsValue::from_str(&self.key))
                .map_err(|error| format!("{error:?}"))?;
            Ok(value.as_string())
        }

        fn save(&self, json: &str) -> Result<(), String> {
            let storage = Self::storage()?;
            Self::method(&storage, "setItem")?
                .call2(
                    &storage,
                    &JsValue::from_str(&self.key),
                    &JsValue::from_str(json),
                )
                .map_err(|error| format!("{error:?}"))?;
            Ok(())
        }

        fn clear(&self) -> Result<(), String> {
            let storage = Self::storage()?;
            Self::method(&storage, "removeItem")?
                .call1(&storage, &JsValue::from_str(&self.key))
                .map_err(|error| format!("{error:?}"))?;
            Ok(())
        }
    }
}

fn chrome_input(chrome: &ChromeSpec) -> ChromeProjectionInput {
    ChromeProjectionInput {
        metrics: chrome.metrics,
        hit_target_min: chrome.hit_target_min,
        panel_header_h: chrome.panel_header_h,
        panel_frame: chrome.panel_frame,
        title_in_border: chrome.title_in_border,
        mode_control: chrome.mode_control,
        minimize_control: chrome.minimize_control,
        maximize_control: chrome.maximize_control,
        resize_grip: chrome.resize_grip,
        dock: chrome.dock,
    }
}

fn spec_charset(charset: SpecCharset) -> Charset {
    match charset {
        SpecCharset::Unicode => Charset::Unicode,
        SpecCharset::Ascii => Charset::Ascii,
    }
}