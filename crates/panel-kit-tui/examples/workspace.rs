//! Native terminal backend for the shared panel-kit workspace-spec canary.
//!
//! The example is executable documentation for host-owned composition: it
//! decodes the Nix-authored `WorkspaceSpec`, owns a `Snapshot`, reduces native
//! input through `panel_kit_tui::input` + `panel_kit_core::reduce`, projects into
//! reusable scratch, then draws the independent ratatui parts in host order.
//!
//! Run through Nix: `nix build .#checks.$(nix eval --raw --impure --expr builtins.currentSystem).workspace-spec-tui-native`.
//! Run manually with `PANEL_KIT_WORKSPACE_SPEC=/path/to/workspace.json cargo run -p panel-kit-tui --features spec-plan --example workspace`.

mod workspace_canary;

use std::time::Duration;

use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, MouseButton, MouseEventKind,
};
use crossterm::execute;
use evidence::{check_offscreen, RenderEvidence};
use panel_kit_core::frame::{
    project_into, ChromeProjectionInput, ProjectionBuffer, ProjectionInput, TileLayoutMetrics,
};
use panel_kit_core::persist::{apply_save_decision, restore_snapshot, LayoutError, RestoreContext};
use panel_kit_core::reducer::{reduce, Snapshot, Viewport, WheelDisposition, WorkspaceEvent};
use panel_kit_core::spec::{BackendKind, Charset as SpecCharset, WorkspaceSpec};
use panel_kit_core::{FocusContext, Region, SpecPanelId, SurfaceCapabilities, SurfaceProfile};
use panel_kit_tui::input::{
    crossterm_key_chord, crossterm_pointer_event, workspace_event_from_key,
    workspace_event_from_pointer,
};
use panel_kit_tui::store::JsonFileLayoutStore;
use panel_kit_tui::widgets::{self, TuiHitBuffer};
use panel_kit_tui::{Charset, ResolvedTuiTheme};
use ratatui::layout::{Position, Rect};
use workspace_canary::content::{render_content, ContentRenderContext, DemoData};

const CHECK_WIDTH: u16 = 128;
const CHECK_HEIGHT: u16 = 52;

struct NativeCanary {
    workspace: ResolvedCanary,
    projection: ProjectionBuffer<SpecPanelId>,
    hits: TuiHitBuffer<SpecPanelId>,
    store: Option<JsonFileLayoutStore>,
    content_jobs: Vec<ContentJob>,
    demo: DemoData,
    evidence: RenderEvidence,
}

struct ResolvedCanary {
    resolved: panel_kit_core::ResolvedWorkspace,
    snapshot: Snapshot<SpecPanelId>,
    theme: ResolvedTuiTheme,
    charset: Charset,
    chrome: ChromeProjectionInput,
}

#[derive(Clone, Copy)]
struct ContentJob {
    body: Rect,
    key: SpecPanelId,
}

impl NativeCanary {
    fn from_spec_env() -> Result<Self, Box<dyn std::error::Error>> {
        let spec = decode_workspace_spec()?;
        let manifest = workspace_canary::provider_manifest(BackendKind::Tui);
        let resolved = spec.resolve(&manifest)?;
        let store = resolved.persistence.enabled.then(|| {
            JsonFileLayoutStore::new(
                std::env::temp_dir().join("panel-kit-tui-workspace-spec-canary.json"),
            )
        });
        let snapshot = restore_or_initial(&resolved, store.as_ref())?;
        let panel_count = resolved.catalog.len();
        let theme = ResolvedTuiTheme::from(&resolved.theme);
        let charset = spec_charset(resolved.glyphs);
        let chrome = chrome_input(&resolved.chrome);

        Ok(Self {
            workspace: ResolvedCanary {
                resolved,
                snapshot,
                theme,
                charset,
                chrome,
            },
            projection: ProjectionBuffer::with_panel_capacity(panel_count),
            hits: TuiHitBuffer::with_capacity(panel_count, panel_count),
            store,
            content_jobs: Vec::with_capacity(panel_count),
            demo: DemoData::new(),
            evidence: RenderEvidence::default(),
        })
    }

    fn draw(&mut self, frame: &mut ratatui::Frame) -> Result<(), LayoutError> {
        self.demo.tick();
        self.sync_viewport(frame.area())?;
        self.hits.clear();
        self.evidence.clear();

        self.content_jobs.clear();
        {
            let surface = self.surface_profile();
            let tile = self.tile_metrics(surface);
            let projected = project_into(
                ProjectionInput {
                    snapshot: &self.workspace.snapshot,
                    surface,
                    chrome: &self.workspace.chrome,
                    clamp: &self.workspace.resolved.layout.clamp,
                    tile: &tile,
                },
                &mut self.projection,
            );

            self.evidence.root = rect_from_region(projected.chrome.root);
            self.evidence.dock = rect_from_region(projected.chrome.dock);
            widgets::root::draw_root(
                frame,
                self.evidence.root,
                &self.workspace.theme,
                self.workspace.charset,
            );

            let mode = projected.mode;
            let dock_area = rect_from_region(projected.chrome.dock);
            for panel in projected.panels.iter().copied() {
                let Some(meta) = self.workspace.resolved.catalog.get(panel.key) else {
                    continue;
                };
                self.evidence
                    .chrome
                    .push((panel.key, rect_from_region(panel.chrome.outer)));
                widgets::panel::draw_panel_surface(
                    frame,
                    panel,
                    &self.workspace.theme,
                    self.workspace.charset,
                    &mut self.hits,
                );
                let body = widgets::panel::draw_panel_chrome(
                    frame,
                    panel,
                    meta,
                    &self.workspace.theme,
                    self.workspace.charset,
                    &mut self.hits,
                );
                widgets::panel::draw_traffic_lights(
                    frame,
                    panel,
                    mode,
                    None,
                    &self.workspace.theme,
                    self.workspace.charset,
                    &mut self.hits,
                );
                widgets::panel::draw_resize_grip(
                    frame,
                    panel,
                    None,
                    &self.workspace.theme,
                    &mut self.hits,
                );
                self.evidence.bodies.push((panel.key, body));
                self.content_jobs.push(ContentJob {
                    body,
                    key: panel.key,
                });
            }

            widgets::dock::draw_dock(
                frame,
                dock_area,
                projected.dock,
                widgets::dock::DockRenderContext {
                    catalog: &self.workspace.resolved.catalog,
                    label: self.workspace.resolved.chrome.dock_label.as_str(),
                    theme: &self.workspace.theme,
                    charset: self.workspace.charset,
                },
                &mut self.hits,
            );
            widgets::root::draw_workspace_scrollbar(frame, &projected, &self.workspace.theme);
        }
        let workspace = &self.workspace;
        let demo = &mut self.demo;
        let content_kinds = &mut self.evidence.content_kinds;
        for job in self.content_jobs.iter().copied() {
            let Some(content) = workspace
                .resolved
                .panels
                .get(job.key.index() as usize)
                .map(|panel| &panel.content)
            else {
                continue;
            };
            render_content(
                frame,
                job.body,
                content,
                ContentRenderContext {
                    resolved: &workspace.resolved,
                    snapshot: &workspace.snapshot,
                    theme: &workspace.theme,
                },
                demo,
                content_kinds,
            );
        }
        Ok(())
    }

    fn handle_key(&mut self, event: crossterm::event::KeyEvent) -> Result<bool, LayoutError> {
        if matches!(event.code, KeyCode::Char('q')) {
            return Ok(true);
        }
        if matches!(event.code, KeyCode::Char('p')) {
            self.toggle_theme();
            return Ok(false);
        }
        if let Some(key) = restore_key(event.code, &self.workspace.resolved.catalog) {
            self.apply_event(WorkspaceEvent::Command {
                target: Some(key),
                command: panel_kit_core::PanelCommand::Restore,
            })?;
            return Ok(false);
        }
        if matches!(event.code, KeyCode::PageUp | KeyCode::PageDown) {
            let delta_y = if matches!(event.code, KeyCode::PageUp) {
                -4.0
            } else {
                4.0
            };
            self.apply_event(WorkspaceEvent::Wheel {
                delta_y,
                disposition: WheelDisposition::BubbleToWorkspace,
            })?;
            return Ok(false);
        }
        if let Some(chord) = crossterm_key_chord(event) {
            self.apply_event(workspace_event_from_key(chord, FocusContext::Workspace))?;
        }
        Ok(false)
    }

    fn handle_mouse(&mut self, event: crossterm::event::MouseEvent) -> Result<(), LayoutError> {
        let at = Position::new(event.column, event.row);
        if event.kind == MouseEventKind::Down(MouseButton::Left) {
            if let Some((_, index)) = self
                .demo
                .badge_zones
                .iter()
                .find(|(rect, _)| rect.contains(at))
            {
                self.demo
                    .actions
                    .push(format!("{:?}", self.demo.badges[*index].primary_action()));
                return Ok(());
            }
            if self.demo.theme_zone.contains(at) {
                self.toggle_theme();
                return Ok(());
            }
        }
        if let Some(pointer) = crossterm_pointer_event(event) {
            if let Some(workspace_event) = workspace_event_from_pointer(&self.hits, pointer) {
                self.apply_event(workspace_event)?;
            }
        }
        Ok(())
    }

    fn apply_event(&mut self, event: WorkspaceEvent<SpecPanelId>) -> Result<(), LayoutError> {
        let context = panel_kit_core::reducer::ReduceContext {
            surface: self.surface_profile(),
            clamp: &self.workspace.resolved.layout.clamp,
            command_step: self.workspace.resolved.input.steps,
            tile: &self.workspace.resolved.layout.tile.resize,
            snap: panel_kit_core::SnapPolicy::default(),
        };
        let reduction = reduce(&mut self.workspace.snapshot, event, context);
        if let Some(store) = &self.store {
            let decision = self
                .workspace
                .resolved
                .persistence
                .save_policy
                .decide(&reduction);
            apply_save_decision(
                decision,
                store,
                &self.workspace.snapshot,
                &self.workspace.resolved.catalog,
            )?;
        }
        Ok(())
    }

    fn sync_viewport(&mut self, area: Rect) -> Result<(), LayoutError> {
        let size = Viewport {
            width: area.width as f64,
            height: area.height as f64,
            units: self.workspace.resolved.layout.units,
        };
        if self.workspace.snapshot.viewport == size {
            return Ok(());
        }
        let event = WorkspaceEvent::ViewportChanged {
            size,
            policy: self.workspace.resolved.surface.resize_policy,
        };
        self.apply_event(event)
    }

    fn surface_profile(&self) -> SurfaceProfile {
        let surface = &self.workspace.resolved.surface;
        SurfaceProfile::from_logical_width(
            self.workspace.snapshot.viewport.width,
            surface.compact_max,
            surface.tablet_max,
            SurfaceCapabilities {
                coarse_pointer: false,
                hover: true,
                keyboard: true,
            },
        )
    }

    fn tile_metrics(&self, surface: SurfaceProfile) -> TileLayoutMetrics {
        let tile = &self.workspace.resolved.layout.tile;
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

    fn toggle_theme(&mut self) {
        self.demo.paper = !self.demo.paper;
        self.workspace.theme = if self.demo.paper {
            ResolvedTuiTheme::from(&panel_kit_core::theme::ThemeTokens::paper())
        } else {
            ResolvedTuiTheme::from(&self.workspace.resolved.theme)
        };
    }
}

fn main() -> std::io::Result<()> {
    if std::env::args().any(|arg| arg == "--check-offscreen") {
        return check_offscreen().map_err(std::io::Error::other);
    }

    let mut app =
        NativeCanary::from_spec_env().map_err(|error| std::io::Error::other(error.to_string()))?;
    let mut terminal = ratatui::init();
    let _ = execute!(std::io::stdout(), EnableMouseCapture);
    loop {
        let mut draw_result = Ok(());
        terminal.draw(|frame| {
            draw_result = app.draw(frame);
        })?;
        draw_result.map_err(to_io_error)?;
        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) if app.handle_key(key).map_err(to_io_error)? => break,
                Event::Mouse(mouse) => app.handle_mouse(mouse).map_err(to_io_error)?,
                _ => {}
            }
        }
    }
    let _ = execute!(std::io::stdout(), DisableMouseCapture);
    ratatui::restore();
    Ok(())
}

fn decode_workspace_spec() -> Result<WorkspaceSpec, Box<dyn std::error::Error>> {
    let Some(path) = option_env!("PANEL_KIT_WORKSPACE_SPEC") else {
        return Err(
            "PANEL_KIT_WORKSPACE_SPEC must point at the Nix-authored workspace spec JSON".into(),
        );
    };
    let json = std::fs::read_to_string(path)?;
    Ok(WorkspaceSpec::from_json_str(&json)?)
}

fn restore_or_initial(
    resolved: &panel_kit_core::ResolvedWorkspace,
    store: Option<&JsonFileLayoutStore>,
) -> Result<Snapshot<SpecPanelId>, LayoutError> {
    if !resolved.persistence.enabled || !resolved.persistence.restore {
        return Ok(resolved.initial.clone());
    }
    let Some(store) = store else {
        return Ok(resolved.initial.clone());
    };
    restore_snapshot(
        store,
        resolved.initial.clone(),
        &resolved.catalog,
        RestoreContext {
            units: resolved.layout.units,
            viewport: (
                resolved.initial.viewport.width,
                resolved.initial.viewport.height,
            ),
        },
    )
}

fn chrome_input(chrome: &panel_kit_core::ChromeSpec) -> ChromeProjectionInput {
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

fn restore_key(
    code: KeyCode,
    catalog: &panel_kit_core::PanelCatalog<SpecPanelId>,
) -> Option<SpecPanelId> {
    let id = match code {
        KeyCode::Char('1') => "Workspace",
        KeyCode::Char('2') => "Activity",
        KeyCode::Char('3') => "Flame",
        KeyCode::Char('4') => "Notes",
        KeyCode::Char('5') => "Badges",
        KeyCode::Char('6') => "Nodes",
        KeyCode::Char('7') => "Capacity",
        KeyCode::Char('8') => "Distribution",
        KeyCode::Char('9') => "Theme",
        _ => return None,
    };
    catalog.get_by_stable_id(id).map(|meta| meta.key)
}

fn rect_from_region(region: Region) -> Rect {
    Rect::new(
        region.x as u16,
        region.y as u16,
        region.w as u16,
        region.h as u16,
    )
}

mod evidence {
    use std::collections::BTreeSet;

    use panel_kit_core::{PanelCatalog, SpecPanelId};
    use ratatui::backend::TestBackend;
    use ratatui::layout::Rect;
    use ratatui::Terminal;

    use crate::{NativeCanary, CHECK_HEIGHT, CHECK_WIDTH};

    /// Offscreen render facts asserted by the deterministic canary check.
    #[derive(Default)]
    pub struct RenderEvidence {
        pub root: Rect,
        pub dock: Rect,
        pub bodies: Vec<(SpecPanelId, Rect)>,
        pub chrome: Vec<(SpecPanelId, Rect)>,
        pub content_kinds: BTreeSet<&'static str>,
    }

    impl RenderEvidence {
        /// Reset per-frame evidence while retaining vector capacity.
        pub fn clear(&mut self) {
            self.root = Rect::default();
            self.dock = Rect::default();
            self.bodies.clear();
            self.chrome.clear();
            self.content_kinds.clear();
        }
    }

    /// Render the native canary into a fixed TestBackend and assert golden regions.
    pub fn check_offscreen() -> Result<(), String> {
        let mut app = NativeCanary::from_spec_env().map_err(|error| error.to_string())?;
        let mut terminal = Terminal::new(TestBackend::new(CHECK_WIDTH, CHECK_HEIGHT))
            .map_err(|error| error.to_string())?;
        let mut draw_result = Ok(());
        terminal
            .draw(|frame| {
                draw_result = app.draw(frame);
            })
            .map_err(|error| error.to_string())?;
        draw_result.map_err(|error| error.to_string())?;
        let buffer = terminal.backend().buffer().clone();

        let mut diff = Vec::new();
        let catalog = &app.workspace.resolved.catalog;
        expect_rect(
            &mut diff,
            "root",
            app.evidence.root,
            Rect::new(0, 0, 128, 52),
        );
        expect_rect(
            &mut diff,
            "workspace body",
            body_rect(&app.evidence, catalog, "Workspace"),
            Rect::new(3, 2, 60, 9),
        );
        expect_rect(
            &mut diff,
            "badges body",
            body_rect(&app.evidence, catalog, "Badges"),
            Rect::new(65, 2, 61, 9),
        );
        expect_rect(
            &mut diff,
            "dock",
            app.evidence.dock,
            Rect::new(1, 48, 126, 3),
        );

        let expected_kinds = expected_content_kinds(&app.workspace.resolved.panels);
        if app.evidence.content_kinds != expected_kinds {
            diff.push(format!(
                "content kinds: got {:?}, expected {:?}",
                app.evidence.content_kinds, expected_kinds
            ));
        }
        expect_text(&mut diff, &buffer, "panel-kit-tui workspace-spec canary");
        expect_text(&mut diff, &buffer, "dock:");
        expect_text(&mut diff, &buffer, "browser-tui");

        if diff.is_empty() {
            return Ok(());
        }
        Err(format!(
            "offscreen workspace-spec TUI canary mismatch:\n{}",
            diff.join("\n")
        ))
    }

    fn body_rect(
        evidence: &RenderEvidence,
        catalog: &PanelCatalog<SpecPanelId>,
        title: &str,
    ) -> Rect {
        evidence
            .bodies
            .iter()
            .find_map(|(key, rect)| title_matches(catalog, *key, title).then_some(*rect))
            .unwrap_or_default()
    }

    fn title_matches(catalog: &PanelCatalog<SpecPanelId>, key: SpecPanelId, title: &str) -> bool {
        catalog
            .get(key)
            .is_some_and(|meta| meta.title.as_ref() == title)
    }

    fn expected_content_kinds(panels: &[panel_kit_core::PanelSpec]) -> BTreeSet<&'static str> {
        panels.iter().map(|panel| panel.content.kind()).collect()
    }

    fn expect_rect(diff: &mut Vec<String>, label: &str, got: Rect, expected: Rect) {
        if got != expected {
            diff.push(format!("{label}: got {got:?}, expected {expected:?}"));
        }
    }

    fn expect_text(diff: &mut Vec<String>, buffer: &ratatui::buffer::Buffer, needle: &str) {
        let mut text = String::new();
        for y in 0..buffer.area.height {
            for x in 0..buffer.area.width {
                text.push_str(buffer[(x, y)].symbol());
            }
            text.push('\n');
        }
        if !text.contains(needle) {
            diff.push(format!("buffer missing text {needle:?}"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draw_records_body_jobs_by_stable_panel_key() {
        let mut app = NativeCanary::from_spec_env().expect("workspace spec should resolve");
        let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(
            CHECK_WIDTH,
            CHECK_HEIGHT,
        ))
        .expect("test backend should initialize");
        let mut draw_result = Ok(());

        terminal
            .draw(|frame| {
                draw_result = app.draw(frame);
            })
            .expect("offscreen draw should complete");
        draw_result.expect("workspace draw should succeed");

        assert_eq!(app.content_jobs.len(), app.evidence.bodies.len());
        assert!(app.content_jobs.iter().all(|job| app
            .workspace
            .resolved
            .catalog
            .get(job.key)
            .is_some()));
        assert!(app.evidence.bodies.iter().all(|(key, _)| app
            .workspace
            .resolved
            .catalog
            .get(*key)
            .is_some()));
    }

    #[test]
    fn viewport_sync_reports_persistence_errors() {
        let mut app = NativeCanary::from_spec_env().expect("workspace spec should resolve");
        let missing_dir = std::env::temp_dir().join(format!(
            "panel-kit-tui-canary-missing-{}",
            std::process::id()
        ));
        app.store = Some(JsonFileLayoutStore::new(missing_dir.join("layout.json")));

        let changed_area = Rect::new(
            0,
            0,
            (app.workspace.snapshot.viewport.width as u16).saturating_add(1),
            app.workspace.snapshot.viewport.height as u16,
        );

        let error = app
            .sync_viewport(changed_area)
            .expect_err("failed viewport save must surface");

        assert!(matches!(error, LayoutError::Store(_)));
    }
}

fn to_io_error(error: LayoutError) -> std::io::Error {
    std::io::Error::other(error)
}
