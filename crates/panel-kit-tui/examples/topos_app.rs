//! Shared topos/grammar demo host for the ratatui native and browser backends.
//!
//! The host owns both regime `WorkspaceSpec`s (`topos-agentic`, `topos-membrane`),
//! the shared demo model, one persisted layout per topos, and the header strip
//! that drives the two axes. Entry hosts own only input plumbing and terminal
//! setup. Keys: `t` topos, `g` grammar, `n`/`x` append, `r` reset, `s` moves the
//! per-sort badge focus, space toggles the focused sort in the active subobject
//! (transported by `∃f`/`f*` on a topos switch; the status line names it).
// Each entry host uses a different subset of the shared model and host API.
#[allow(dead_code)]
#[path = "../../../examples/support/topos_demo.rs"]
mod topos_demo;

use std::fmt;

use panel_kit_core::frame::{
    project_into, ChromeProjectionInput, FrameStatus, ProjectionBuffer, ProjectionInput,
    TileLayoutMetrics,
};
use panel_kit_core::persist::{
    apply_save_decision, layout_key_for_spec, restore_snapshot, switch_layout, LayoutError,
    LayoutStore, RestoreContext,
};
use panel_kit_core::reducer::{
    reduce, ReduceContext, Snapshot, Viewport, WheelDisposition, WorkspaceEvent,
};
use panel_kit_core::spec::{BackendKind, BindingManifest, ResolvedWorkspace, WorkspaceSpec};
use panel_kit_core::widgets::charts::{five_num, BoxItemView, SeriesView};
use panel_kit_core::widgets::table::TableView;
use panel_kit_core::widgets::ContentSpec;
use panel_kit_core::{
    ChromeSpec, Charset as SpecCharset, FocusContext, Key, KeyChord, PanelCommand,
    SnapPolicy, SpecPanelId, SurfaceCapabilities, SurfaceProfile,
};
use panel_kit_tui::badge;
use panel_kit_tui::charts::{boxplot, flame, gauges, time_series};
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

use panel_kit_grammar::Section;
use topos_demo::ToposDemo;

/// Rows the header strip occupies above the projected workspace.
pub const HEADER_ROWS: u16 = 1;

/// Failure decoding a topos spec or locating its authored JSON.
#[derive(Debug)]
pub enum HostError {
    /// A topos the demo model declares has no spec JSON in the host's pair.
    MissingSpec(&'static str),
    /// Strict spec decoding or provider resolution failed.
    Spec(String),
}

impl fmt::Display for HostError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingSpec(spec_id) => write!(f, "no WorkspaceSpec JSON supplied for {spec_id}"),
            Self::Spec(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for HostError {}

/// A demo control the header strip can trigger.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeaderAction {
    /// Swap spec, persisted layout, and stalks to the named topos.
    SetTopos(&'static str),
    /// Re-parse a different grammar's sample source.
    SetGrammar(&'static str),
    /// Append one well-formed raw line.
    AppendValid,
    /// Append one line the active grammar rejects.
    AppendMalformed,
    /// Restore the active grammar's sample source.
    ResetSource,
}

/// A demo key intent, translated from a platform key event by the entry host.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DemoKey {
    /// `t` next topos, `g` next grammar, `n` valid line, `x` malformed line, `r` reset.
    Switch(char),
    /// `s` advances the per-sort badge focus cursor.
    CycleSort,
    /// Space toggles the focused sort in the active subobject.
    ToggleSort,
    /// `p` theme palette.
    Palette,
    /// `1`-`9` restore the panel at that 1-based catalog position.
    Restore(usize),
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
                't' | 'g' | 'n' | 'x' | 'r' => return Self::Switch(ch),
                's' => return Self::CycleSort,
                ' ' => return Self::ToggleSort,
                'p' => return Self::Palette,
                '1'..='9' => return Self::Restore(usize::from(ch as u8 - b'0')),
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

/// One topos: its resolved spec, its persisted layout, and its live snapshot.
struct Scene {
    spec_id: &'static str,
    resolved: ResolvedWorkspace,
    snapshot: Snapshot<SpecPanelId>,
    store: Option<Box<dyn LayoutStore>>,
}

/// One header control: pre-measured text plus the action it triggers.
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

/// Demo host owning every topos scene, the shared model, and the header strip.
pub struct ToposHost {
    demo: ToposDemo,
    scenes: Vec<Scene>,
    active: usize,
    theme: ResolvedTuiTheme,
    charset: Charset,
    paper: bool,
    scroll: PanelScroll<SpecPanelId>,
    hits: TuiHitBuffer<SpecPanelId>,
    projection: ProjectionBuffer<SpecPanelId>,
    jobs: Vec<BodyJob>,
    paints: Vec<BodyPaint>,
    live_keys: Vec<SpecPanelId>,
    header_items: Vec<HeaderItem>,
    header_zones: Vec<(Rect, HeaderAction)>,
    grid: (f64, f64),
}

impl ToposHost {
    /// Decode both topos specs, restore their layouts, and select the first.
    pub fn new(backend: BackendKind) -> Result<Self, HostError> {
        let demo = ToposDemo::new();
        let mut scenes = Vec::new();
        for spec_id in demo.topos_ids() {
            let Some(json) = demo.workspace_json(spec_id) else {
                return Err(HostError::MissingSpec(spec_id));
            };
            let spec = WorkspaceSpec::from_json_str(json)
                .map_err(|error| HostError::Spec(format!("{spec_id}: {error}")))?;
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
                .map_err(|error| HostError::Spec(format!("{spec_id}: {error}")))?;
            let store = make_store(&resolved);
            let snapshot = restored(&resolved, store.as_deref());
            scenes.push(Scene {
                spec_id,
                resolved,
                snapshot,
                store,
            });
        }

        let capacity = scenes
            .iter()
            .map(|scene| scene.resolved.catalog.len())
            .max()
            .unwrap_or(0);
        Ok(Self {
            theme: ResolvedTuiTheme::from(&scenes[0].resolved.theme),
            charset: spec_charset(scenes[0].resolved.glyphs),
            header_items: build_header_items(demo.topos_ids(), demo.grammar_ids()),
            demo,
            scenes,
            active: 0,
            paper: false,
            scroll: PanelScroll::with_capacity(capacity),
            hits: TuiHitBuffer::with_capacity(capacity, capacity),
            projection: ProjectionBuffer::with_panel_capacity(capacity),
            jobs: Vec::with_capacity(capacity),
            paints: Vec::with_capacity(capacity),
            live_keys: Vec::with_capacity(capacity),
            header_zones: Vec::with_capacity(capacity),
            grid: (1.0, 1.0),
        })
    }

    /// Shared demo model, for hosts that surface its counters.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn demo(&self) -> &ToposDemo {
        &self.demo
    }

    /// Every topos spec id, in demo cycle order.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn topos_ids(&self) -> Vec<&'static str> {
        self.scenes.iter().map(|scene| scene.spec_id).collect()
    }

    /// Authored panel titles of the active topos, in catalog order.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn panel_titles(&self) -> Vec<String> {
        self.scenes[self.active]
            .resolved
            .panels
            .iter()
            .map(|panel| panel.title.clone())
            .collect()
    }

    /// Panel bodies painted by the most recent frame.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn paints(&self) -> &[BodyPaint] {
        &self.paints
    }

    /// Live canvas size in terminal cells, for browser wheel translation.
    #[cfg(target_arch = "wasm32")]
    pub fn grid(&self) -> (f64, f64) {
        self.grid
    }

    /// Split borrows for entry hosts that route platform events themselves.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn routing(&mut self) -> (&TuiHitBuffer<SpecPanelId>, &mut PanelScroll<SpecPanelId>) {
        (&self.hits, &mut self.scroll)
    }

    /// Swap topos: persisted layout, authored spec, and visual stalks.
    pub fn set_topos(&mut self, spec_id: &str) -> bool {
        let Some(index) = self
            .scenes
            .iter()
            .position(|scene| scene.spec_id == spec_id)
        else {
            return false;
        };
        if index == self.active {
            return self.demo.set_topos(spec_id);
        }

        let viewport = (
            self.scenes[self.active].snapshot.viewport.width,
            self.scenes[self.active].snapshot.viewport.height,
        );
        let context = RestoreContext {
            units: self.scenes[index].resolved.layout.units,
            viewport,
        };
        let defaults = self.scenes[index].resolved.initial.clone();
        let (Some(old_store), Some(new_store)) = (
            self.scenes[self.active].store.as_deref(),
            self.scenes[index].store.as_deref(),
        ) else {
            self.scenes[index].snapshot = defaults;
            self.enter_scene(index);
            return self.demo.set_topos(spec_id);
        };
        let switched = switch_layout(
            old_store,
            &self.scenes[self.active].snapshot,
            &self.scenes[self.active].resolved.catalog,
            new_store,
            defaults.clone(),
            &self.scenes[index].resolved.catalog,
            context,
        )
        .unwrap_or(defaults);
        self.scenes[index].snapshot = switched;
        self.enter_scene(index);
        self.demo.set_topos(spec_id)
    }

    /// Switch to the next topos in demo cycle order.
    pub fn cycle_topos(&mut self) {
        self.set_topos(self.demo.next_topos_id());
    }

    /// Switch to the next grammar, re-parsing the sample source.
    pub fn cycle_grammar(&mut self) {
        self.demo.cycle_grammar();
    }

    /// Toggle the demo theme between the spec tokens and the paper preset.
    pub fn toggle_palette(&mut self) {
        self.paper = !self.paper;
        self.theme = if self.paper {
            ResolvedTuiTheme::from(&panel_kit_core::theme::ThemeTokens::paper())
        } else {
            ResolvedTuiTheme::from(&self.scenes[self.active].resolved.theme)
        };
    }

    /// Apply one demo key intent; true asks the entry host to exit.
    pub fn handle_key(&mut self, key: DemoKey) -> Result<bool, LayoutError> {
        match key {
            DemoKey::Switch('t') => self.cycle_topos(),
            DemoKey::Switch('g') => self.cycle_grammar(),
            DemoKey::Switch('n') => self.demo.append_valid_line(),
            DemoKey::Switch('x') => self.demo.append_malformed_line(),
            DemoKey::Switch('r') => self.demo.reset_source(),
            DemoKey::Switch(_) => {}
            DemoKey::CycleSort => self.demo.cycle_sort_focus(),
            DemoKey::ToggleSort => self.demo.toggle_focused_sort(),
            DemoKey::Palette => self.toggle_palette(),
            DemoKey::Restore(slot) => self.restore_panel(slot)?,
            DemoKey::Page(rows) => self.page_scroll(rows)?,
            DemoKey::Chord(chord) => {
                self.reduce(workspace_event_from_key(chord, FocusContext::Workspace))?;
            }
            #[cfg(not(target_arch = "wasm32"))]
            DemoKey::Quit => return Ok(true),
        }
        Ok(false)
    }

    /// Apply a header control under `at`; true when the host consumed it.
    pub fn press(&mut self, at: Position) -> bool {
        let Some((_, action)) = self
            .header_zones
            .iter()
            .rev()
            .find(|(rect, _)| rect.contains(at))
        else {
            return false;
        };
        match *action {
            HeaderAction::SetTopos(spec_id) => {
                self.set_topos(spec_id);
            }
            HeaderAction::SetGrammar(grammar_id) => {
                self.demo.set_grammar(grammar_id);
            }
            HeaderAction::AppendValid => self.demo.append_valid_line(),
            HeaderAction::AppendMalformed => self.demo.append_malformed_line(),
            HeaderAction::ResetSource => self.demo.reset_source(),
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

    /// Reduce one workspace event and apply the spec save policy.
    pub fn reduce(&mut self, event: WorkspaceEvent<SpecPanelId>) -> Result<(), LayoutError> {
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

    /// Paint one frame: header strip, workspace chrome, and bound bodies.
    pub fn draw(&mut self, frame: &mut ratatui::Frame) -> Result<(), LayoutError> {
        let area = frame.area();
        self.hits.clear();
        self.paints.clear();
        self.header_zones.clear();
        self.jobs.clear();
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

        for panel in projected.panels.iter().copied() {
            let Some(meta) = self.scenes[self.active].resolved.catalog.get(panel.key) else {
                continue;
            };
            widgets::panel::draw_panel_surface(
                frame,
                panel,
                &self.theme,
                self.charset,
                &mut self.hits,
            );
            let body = widgets::panel::draw_panel_chrome(
                frame,
                panel,
                meta,
                &self.theme,
                self.charset,
                &mut self.hits,
            );
            widgets::panel::draw_traffic_lights(
                frame,
                panel,
                projected.mode,
                None,
                &self.theme,
                self.charset,
                &mut self.hits,
            );
            widgets::panel::draw_resize_grip(frame, panel, None, &self.theme, &mut self.hits);
            let body = body.intersection(workspace);
            if body.width > 0 && body.height > 0 {
                self.jobs.push(BodyJob { body, key: panel.key });
            }
        }
        widgets::dock::draw_dock(
            frame,
            rect_from_region(projected.chrome.dock),
            projected.dock,
            widgets::dock::DockRenderContext {
                catalog: &self.scenes[self.active].resolved.catalog,
                label: self.scenes[self.active].resolved.chrome.dock_label.as_str(),
                theme: &self.theme,
                charset: self.charset,
            },
            &mut self.hits,
        );


        widgets::root::draw_workspace_scrollbar(frame, &projected, &self.theme);
        self.paint_bodies(frame);
        self.draw_header(
            frame,
            Rect::new(area.x, area.y, area.width, HEADER_ROWS),
        );
        Ok(())
    }

    /// Restore the panel at a 1-based catalog position.
    fn restore_panel(&mut self, slot: usize) -> Result<(), LayoutError> {
        let index = slot.saturating_sub(1);
        let Some(panel) = self.scenes[self.active].resolved.panels.get(index) else {
            return Ok(());
        };
        let Some(key) = self.scenes[self.active]
            .resolved
            .catalog
            .get_by_stable_id(&panel.id)
            .map(|meta| meta.key)
        else {
            return Ok(());
        };
        self.reduce(WorkspaceEvent::Command {
            target: Some(key),
            command: PanelCommand::Restore,
        })
    }

    /// Paint every projected body from the binding its panel authored.
    fn paint_bodies(&mut self, frame: &mut ratatui::Frame) {
        let scene = &self.scenes[self.active];
        let demo = &self.demo;
        let theme = self.theme;
        let scroll = &mut self.scroll;
        for job in self.jobs.iter().copied() {
            let Some(spec) = scene.resolved.panels.get(job.key.index() as usize) else {
                continue;
            };
            let content = demo.resolve(spec.content.binding_id().unwrap_or_default());
            paint_body(
                frame,
                job.body,
                &job.key,
                &spec.content,
                content,
                &theme,
                scroll,
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

    /// Paint the one-row header: status line plus the demo controls.
    fn draw_header(&mut self, frame: &mut ratatui::Frame, area: Rect) {
        let theme = self.theme;
        let (active_topos, active_grammar) = (self.demo.topos_id(), self.demo.grammar_id());
        for item in &mut self.header_items {
            item.active = match item.action {
                HeaderAction::SetTopos(spec_id) => spec_id == active_topos,
                HeaderAction::SetGrammar(grammar_id) => grammar_id == active_grammar,
                _ => false,
            };
        }

        let mut shown: u16 = self
            .header_items
            .iter()
            .map(|item| item.width + 1)
            .sum();
        let mut first = 0;
        while shown > area.width && first < self.header_items.len() {
            shown -= self.header_items[first].width + 1;
            first += 1;
        }

        let mut x = area.right();
        for index in (first..self.header_items.len()).rev() {
            let item = &self.header_items[index];
            let rect = Rect::new(x.saturating_sub(item.width), area.y, item.width, 1);
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

        let status = Rect::new(area.x, area.y, x.saturating_sub(area.x), 1);
        if status.width > 0 {
            frame.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    self.demo.status_line(),
                    Style::default().fg(theme.fg),
                ))),
                status,
            );
        }
    }

    /// Adopt a scene's glyph family and spec theme, dropping stale offsets.
    fn enter_scene(&mut self, index: usize) {
        self.active = index;
        self.charset = spec_charset(self.scenes[index].resolved.glyphs);
        if !self.paper {
            self.theme = ResolvedTuiTheme::from(&self.scenes[index].resolved.theme);
        }
        self.scroll.clear();
    }

    /// Scroll the focused panel body by a page, else the workspace.
    fn page_scroll(&mut self, rows: f64) -> Result<(), LayoutError> {
        let event = match self.scenes[self.active].snapshot.focused {
            Some(key) => route_wheel_to(&mut self.scroll, &key, WheelDelta::new(0.0, rows)),
            None => WorkspaceEvent::Wheel {
                delta_y: rows,
                disposition: WheelDisposition::BubbleToWorkspace,
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

fn build_header_items(
    toposes: Vec<&'static str>,
    grammars: Vec<&'static str>,
) -> Vec<HeaderItem> {
    let mut items = Vec::with_capacity(toposes.len() + grammars.len() + 3);
    for spec_id in toposes {
        items.push(HeaderItem::new(
            spec_id.trim_start_matches("topos-"),
            HeaderAction::SetTopos(spec_id),
        ));
    }
    for grammar_id in grammars {
        items.push(HeaderItem::new(
            grammar_id,
            HeaderAction::SetGrammar(grammar_id),
        ));
    }
    items.push(HeaderItem::new("+ line", HeaderAction::AppendValid));
    items.push(HeaderItem::new("+ bad", HeaderAction::AppendMalformed));
    items.push(HeaderItem::new("reset", HeaderAction::ResetSource));
    items
}

/// Paint one panel body from the content its authored binding resolved to.
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
        Section::TimeSeries(models) => {
            let views: Vec<SeriesView<'_>> = models
                .iter()
                .map(|model| SeriesView {
                    name: model.name.as_str(),
                    points: &model.points,
                })
                .collect();
            time_series(frame, body, theme, time_unit(spec), &views);
        }
        Section::Gauges(models) => gauges(frame, body, theme, models),
        Section::Flamegraph(spans) => flame(frame, body, theme, spans),
        Section::Boxplot(items) => {
            let views: Vec<BoxItemView<'_>> = items
                .iter()
                .filter_map(|item| {
                    five_num(&item.samples).map(|summary| BoxItemView {
                        label: item.label.as_str(),
                        summary,
                        color: item.color,
                    })
                })
                .collect();
            boxplot(frame, body, theme, &views);
        }
        Section::Badges(specs) => {
            let rows: Vec<Line<'_>> = specs
                .iter()
                .map(|badge_spec| Line::from(badge::spans(badge_spec, theme)))
                .collect();
            panel_kit_tui::scroll::lines(frame, body, theme, key, scroll, rows);
        }
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
        Section::Status(model) => {
            frame.render_widget(Paragraph::new(panel_kit_tui::status::line(model)), body);
        }
    }
}

fn time_unit(spec: &ContentSpec) -> &str {
    match spec {
        ContentSpec::TimeSeries { unit, .. } => unit.as_str(),
        _ => "",
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

fn restored(
    resolved: &ResolvedWorkspace,
    store: Option<&dyn LayoutStore>,
) -> Snapshot<SpecPanelId> {
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
    restore_snapshot(store, defaults, &resolved.catalog, context).unwrap_or_else(|_| {
        resolved.initial.clone()
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn make_store(resolved: &ResolvedWorkspace) -> Option<Box<dyn LayoutStore>> {
    if !resolved.persistence.enabled {
        return None;
    }
    let key = layout_key_for_spec(&resolved.persistence.key, &resolved.id);
    let name = key.replace(['/', '\\'], "_");
    Some(Box::new(JsonFileLayoutStore::new(
        std::env::temp_dir().join(format!("panel-kit-topos-{name}.json")),
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
