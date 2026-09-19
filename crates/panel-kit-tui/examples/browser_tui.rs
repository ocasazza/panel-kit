//! Browser canary for the ratatui renderer.
//!
//! Run through the Nix check lane so the authoritative workspace spec is passed
//! as a build-time store path:
//!
//! ```sh
//! nix build .#checks.$(nix eval --raw --impure --expr builtins.currentSystem).workspace-spec-browser-tui-wasm
//! ```
//!
//! This is intentionally comprehensive rather than cute: it exercises the same
//! nine-panel workspace spec as the terminal example through a browser-hosted
//! ratatui loop, including keyboard window management, V2 localStorage
//! persistence, wheel scrolling, badges, spinner, theming, charts, and the
//! ASCII glyph override used by Ratzilla's WebGL font atlas.

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    eprintln!(
        "browser_tui is a wasm/Ratzilla example. Run it with trunk or the \
         checks.workspace-spec-browser-tui-wasm flake lane."
    );
}

#[cfg(target_arch = "wasm32")]
mod workspace_canary;

#[cfg(target_arch = "wasm32")]
mod browser {
    mod body {
        use panel_kit_core::badge::BadgeSpec;
        use panel_kit_core::{Mode, SurfaceProfile};
        use panel_kit_tui::charts::{boxplot, flame, gauges, time_series};
        use panel_kit_tui::scroll;
        use panel_kit_tui::spinner::spinner;
        use panel_kit_tui::{badge, ResolvedTuiTheme};
        use ratatui::layout::Rect;
        use ratatui::style::Style;
        use ratatui::text::{Line, Span};
        use ratatui::widgets::Paragraph;

        use crate::workspace_canary::{capacity_items, node_rows, Metrics};

        /// Borrowed drawing inputs shared by the browser-hosted panel body painters.
        pub(crate) struct BodyDrawContext<'a> {
            pub(crate) theme: ResolvedTuiTheme,
            pub(crate) surface: SurfaceProfile,
            pub(crate) mode: Mode,
            pub(crate) tick: u64,
            pub(crate) paper: bool,
            pub(crate) metrics: &'a Metrics,
            pub(crate) badges: &'a [BadgeSpec],
            pub(crate) badge_zones: &'a mut Vec<(Rect, usize)>,
            pub(crate) actions: &'a [String],
            pub(crate) notes_scroll: &'a mut usize,
            pub(crate) theme_zone: &'a mut Rect,
        }

        /// Draws the body content for a resolved canary panel id.
        pub(crate) fn draw_panel_body(
            frame: &mut ratatui::Frame,
            rect: Rect,
            stable_id: &str,
            context: &mut BodyDrawContext<'_>,
        ) {
            match stable_id {
                "Workspace" => draw_workspace_body(frame, rect, context),
                "Badges" => draw_badges_body(frame, rect, context),
                "Activity" => {
                    time_series(frame, rect, &context.theme, "ms", &context.metrics.series())
                }
                "Capacity" => gauges(frame, rect, &context.theme, &capacity_items()),
                "Flame" => flame(frame, rect, &context.theme, &context.metrics.flame()),
                "Distribution" => boxplot(frame, rect, &context.theme, &context.metrics.boxes()),
                "Nodes" => draw_nodes_body(frame, rect, context),
                "Notes" => draw_notes_body(frame, rect, context),
                "Theme" => draw_theme_body(frame, rect, context),
                _ => {}
            }
        }

        fn draw_workspace_body(
            frame: &mut ratatui::Frame,
            rect: Rect,
            context: &BodyDrawContext<'_>,
        ) {
            let mode = match context.mode {
                Mode::Floating => "floating",
                Mode::Tiling => "tiling",
            };
            frame.render_widget(
                Paragraph::new(vec![
                    Line::from(vec![Span::styled(
                        "panel-kit-tui backend canary",
                        Style::default().fg(context.theme.fg),
                    )]),
                    Line::from(""),
                    Line::from("The same ratatui workspace renders in terminal and browser."),
                    Line::from(format!(
                        "Surface: {:?} · effective mode: {mode}",
                        context.surface.class
                    )),
                    Line::from("Persistence: schema V2 · Units::Cells · browser localStorage."),
                    Line::from(""),
                    Line::from("Mouse: drag headers/grip, click lights; wheel scrolls workspace."),
                    Line::from(
                        "Keys: arrows move, Shift resizes, Alt fine-moves; m/f/t, Tab, Enter.",
                    ),
                    Line::from("Palette: p · restore: 1-9 · workspace scroll: PgUp/PgDn."),
                ])
                .style(Style::default().fg(context.theme.dim)),
                rect,
            );
        }

        fn draw_badges_body(
            frame: &mut ratatui::Frame,
            rect: Rect,
            context: &mut BodyDrawContext<'_>,
        ) {
            for (row, (i, badge_spec)) in context.badges.iter().enumerate().enumerate() {
                if row as u16 >= rect.height.saturating_sub(4) {
                    break;
                }
                let width = badge::width(badge_spec).min(rect.width);
                let badge_rect = Rect::new(rect.x, rect.y + row as u16, width, 1);
                context.badge_zones.push((badge_rect, i));
                frame.render_widget(
                    Paragraph::new(Line::from(badge::spans(badge_spec, &context.theme))),
                    badge_rect,
                );
            }

            let log_y = rect.y + rect.height.saturating_sub(3);
            let recent: Vec<Line<'_>> = context
                .actions
                .iter()
                .rev()
                .take(3)
                .map(|action| {
                    Line::from(Span::styled(
                        action.as_str(),
                        Style::default().fg(context.theme.badge_info),
                    ))
                })
                .collect();
            if log_y > rect.y {
                frame.render_widget(
                    Paragraph::new(recent),
                    Rect::new(rect.x, log_y, rect.width, 3.min(rect.height)),
                );
            }
        }

        fn draw_nodes_body(frame: &mut ratatui::Frame, rect: Rect, context: &BodyDrawContext<'_>) {
            let table_model = node_rows();
            panel_kit_tui::table::table(
                frame,
                rect,
                &context.theme,
                panel_kit_core::widgets::table::TableView {
                    columns: &table_model.columns,
                    rows: &table_model.rows,
                },
            );
        }

        fn draw_notes_body(
            frame: &mut ratatui::Frame,
            rect: Rect,
            context: &mut BodyDrawContext<'_>,
        ) {
            let mut lines = vec![
                Line::from(Span::styled(
                    "docs-as-code canary",
                    Style::default().fg(context.theme.fg),
                )),
                Line::from(""),
            ];
            for text in [
                "This example renders the ratatui workspace through a host-owned browser loop.",
                "It exercises workspace panels, traffic lights, drag math, restore hooks, badges, action routing, charts, gauges, spinner frames, theming, and scrollbars.",
                "The Nix WorkspaceSpec owns panel identity, geometry, chrome, theme, persistence, and glyph selection.",
                "When the browser example builds under Trunk, the public TUI parts remain web-capable.",
                "Keeping terminal and browser examples broad catches drift between core, Dioxus, and TUI renderers.",
                "The example is not a screenshot fixture: it is executable documentation.",
                "Use p for palette, m/f/t for window state, Tab to cycle focus, 1-9 to restore, and PgUp/PgDn or the wheel to scroll.",
            ] {
                lines.push(Line::from(text));
            }
            lines.push(Line::from(""));
            lines.push(spinner(context.tick, "TUI canary running", &context.theme));
            *context.notes_scroll =
                scroll::lines(frame, rect, &context.theme, lines, *context.notes_scroll);
        }

        fn draw_theme_body(
            frame: &mut ratatui::Frame,
            rect: Rect,
            context: &mut BodyDrawContext<'_>,
        ) {
            *context.theme_zone = rect;
            let sw = |color, name: &'static str| {
                Line::from(vec![
                    Span::styled("## ", Style::default().fg(color)),
                    Span::styled(name, Style::default().fg(context.theme.dim)),
                ])
            };
            frame.render_widget(
                Paragraph::new(vec![
                    Line::from(Span::styled(
                        if context.paper {
                            "preset: paper (click or press p)"
                        } else {
                            "preset: dark (click or press p)"
                        },
                        Style::default().fg(context.theme.fg),
                    )),
                    sw(context.theme.blue, "blue · mode light"),
                    sw(context.theme.yellow, "yellow · minimize"),
                    sw(context.theme.pink, "pink · maximize"),
                    spinner(context.tick / 2, "spinner", &context.theme),
                ]),
                rect,
            );
        }
    }

    mod storage {
        use panel_kit_core::persist::LayoutStore;
        use ratzilla::web_sys::js_sys::{Function, Reflect};
        use ratzilla::web_sys::wasm_bindgen::{JsCast, JsValue};

        /// Browser `localStorage` adapter for the workspace layout store contract.
        pub(crate) struct BrowserLocalStorage {
            key: String,
        }

        impl BrowserLocalStorage {
            /// Creates a store bound to one logical browser persistence key.
            pub(crate) fn new(key: impl Into<String>) -> Self {
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

    use std::{cell::RefCell, rc::Rc};

    use panel_kit_core::badge::BadgeSpec;
    use panel_kit_core::frame::{
        project_into, ChromeProjectionInput, FrameStatus, ProjectionBuffer, ProjectionInput,
        TileLayoutMetrics,
    };
    use panel_kit_core::persist::{
        apply_save_decision, restore_snapshot, LayoutError, RestoreContext, SavePolicy,
    };
    use panel_kit_core::reducer::{
        reduce, HitTarget, Snapshot, Viewport, WheelDisposition, WorkspaceEvent,
    };
    use panel_kit_core::theme::ThemeTokens;
    use panel_kit_core::{
        BackendKind, ChromeSpec, FocusContext, InputSpec, LayoutSpec, PanelCatalog,
        PersistenceSpec, PointerButton, PointerEvent, PointerEventKind, Region, SpecPanelId,
        SurfaceProfile, SurfaceSpec, Units, WorkspaceSpec,
    };
    use panel_kit_tui::input::{
        ratzilla_key_chord, workspace_event_from_key, workspace_event_from_pointer,
        RatzillaPointerTranslator,
    };
    use panel_kit_tui::widgets::dock::{draw_dock, DockRenderContext};
    use panel_kit_tui::widgets::panel::{
        draw_panel_chrome, draw_panel_surface, draw_resize_grip, draw_traffic_lights,
    };
    use panel_kit_tui::widgets::root::{draw_root, draw_workspace_scrollbar};
    use panel_kit_tui::widgets::TuiHitBuffer;
    use panel_kit_tui::{Charset, ResolvedTuiTheme};
    use ratatui::layout::{Position, Rect};
    use ratatui::style::{Color, Style};
    use ratatui::widgets::Paragraph;
    use ratzilla::event::{
        KeyCode, KeyEvent, MouseButton as WebMouseButton, MouseEvent as WebMouseEvent,
        MouseEventKind as WebMouseEventKind,
    };
    use ratzilla::web_sys::js_sys::{Function, Reflect};
    use ratzilla::web_sys::wasm_bindgen::{closure::Closure, JsCast, JsValue};
    use ratzilla::{
        backend::webgl2::{FontAtlasConfig, WebGl2BackendOptions},
        CursorShape, WebGl2Backend, WebRenderer,
    };

    use self::body::{draw_panel_body, BodyDrawContext};
    use self::storage::BrowserLocalStorage;
    use crate::workspace_canary::{demo_badges, Metrics};

    const WORKSPACE_SPEC_JSON: &str = include_str!(env!("PANEL_KIT_WORKSPACE_SPEC"));
    const RESTORE_IDS: [&str; 9] = [
        "Workspace",
        "Badges",
        "Activity",
        "Capacity",
        "Notes",
        "Theme",
        "Nodes",
        "Flame",
        "Distribution",
    ];

    struct App {
        catalog: PanelCatalog<SpecPanelId>,
        snapshot: Snapshot<SpecPanelId>,
        layout: LayoutSpec,
        surface_spec: SurfaceSpec,
        chrome: ChromeSpec,
        input: InputSpec,
        persistence: PersistenceSpec,
        store: Option<BrowserLocalStorage>,
        save_policy: SavePolicy,
        projection: ProjectionBuffer<SpecPanelId>,
        hits: TuiHitBuffer<SpecPanelId>,
        pointer: RatzillaPointerTranslator,
        theme: ResolvedTuiTheme,
        charset: Charset,
        badges: Vec<BadgeSpec>,
        badge_zones: Vec<(Rect, usize)>,
        theme_zone: Rect,
        actions: Vec<String>,
        notes_scroll: usize,
        paper: bool,
        tick: u64,
        metrics: Metrics,
        layout_ready: bool,
        hover: Option<Position>,
    }

    impl App {
        fn new() -> Result<Self, Box<dyn std::error::Error>> {
            let resolved = WorkspaceSpec::from_json_str(WORKSPACE_SPEC_JSON)?.resolve(
                &crate::workspace_canary::provider_manifest(BackendKind::BrowserTui),
            )?;
            let panel_count = resolved.catalog.len();
            let charset = tui_charset(resolved.glyphs);
            let save_policy = resolved.persistence.save_policy;
            let store = resolved
                .persistence
                .enabled
                .then(|| BrowserLocalStorage::new(resolved.persistence.key.clone()));
            let theme = ResolvedTuiTheme::from(&resolved.theme);

            Ok(Self {
                catalog: resolved.catalog,
                snapshot: resolved.initial,
                layout: resolved.layout,
                surface_spec: resolved.surface,
                chrome: resolved.chrome,
                input: resolved.input,
                persistence: resolved.persistence,
                store,
                save_policy,
                projection: ProjectionBuffer::with_panel_capacity(panel_count),
                hits: TuiHitBuffer::with_capacity(panel_count, panel_count),
                pointer: RatzillaPointerTranslator::new(),
                theme,
                charset,
                badges: demo_badges(),
                badge_zones: Vec::new(),
                theme_zone: Rect::default(),
                actions: Vec::new(),
                notes_scroll: 0,
                paper: false,
                tick: 0,
                metrics: Metrics::new(),
                layout_ready: false,
                hover: None,
            })
        }

        fn handle_key(&mut self, event: KeyEvent) {
            match event.code.clone() {
                KeyCode::Char('p') => self.toggle_theme(),
                KeyCode::Char(ch) if ('1'..='9').contains(&ch) => {
                    self.restore_panel_by_number(ch);
                }
                KeyCode::PageUp => self.reduce_workspace_event(WorkspaceEvent::Wheel {
                    delta_y: -4.0,
                    disposition: WheelDisposition::BubbleToWorkspace,
                }),
                KeyCode::PageDown => self.reduce_workspace_event(WorkspaceEvent::Wheel {
                    delta_y: 4.0,
                    disposition: WheelDisposition::BubbleToWorkspace,
                }),
                _ => {
                    if let Some(chord) = ratzilla_key_chord(event) {
                        self.reduce_workspace_event(workspace_event_from_key(
                            chord,
                            FocusContext::Workspace,
                        ));
                    }
                }
            }
        }

        fn handle_mouse(&mut self, event: WebMouseEvent) {
            let at = Position::new(event.col, event.row);
            self.hover = Some(at);
            if self.host_consumes_primary_down(&event, at) {
                return;
            }

            let Some(pointer) = self.pointer.pointer_event(event) else {
                return;
            };
            if let Some(event) = workspace_event_from_pointer(&self.hits, pointer) {
                self.reduce_workspace_event(event);
            }
        }

        fn draw(&mut self, frame: &mut ratatui::Frame) {
            self.tick += 1;
            self.metrics.tick();
            self.badge_zones.clear();
            self.hits.clear();
            self.sync_viewport(frame.area());
            expect_persistence(self.restore_pending_layout());

            let surface = self.surface_profile();
            let chrome = self.chrome_projection();
            let tile = self.tile_metrics(surface);
            self.projection.reserve_panels(self.snapshot.panels.len());
            self.hits
                .reserve(self.snapshot.panels.len(), self.snapshot.panels.len());
            let projected = project_into(
                ProjectionInput {
                    snapshot: &self.snapshot,
                    surface,
                    chrome: &chrome,
                    clamp: &self.layout.clamp,
                    tile: &tile,
                },
                &mut self.projection,
            );

            draw_root(frame, frame.area(), &self.theme, self.charset);
            if projected.status == FrameStatus::TooSmall {
                frame.render_widget(
                    Paragraph::new("resize").style(Style::default().fg(self.theme.dim)),
                    frame.area(),
                );
                return;
            }

            let mut body = BodyDrawContext {
                theme: self.theme,
                surface: projected.surface,
                mode: projected.mode,
                tick: self.tick,
                paper: self.paper,
                metrics: &self.metrics,
                badges: &self.badges,
                badge_zones: &mut self.badge_zones,
                actions: &self.actions,
                notes_scroll: &mut self.notes_scroll,
                theme_zone: &mut self.theme_zone,
            };

            for panel in projected.panels.iter().copied() {
                let Some(meta) = self.catalog.get(panel.key) else {
                    continue;
                };
                draw_panel_surface(frame, panel, &self.theme, self.charset, &mut self.hits);
                let body_rect = draw_panel_chrome(
                    frame,
                    panel,
                    meta,
                    &self.theme,
                    self.charset,
                    &mut self.hits,
                );
                draw_traffic_lights(
                    frame,
                    panel,
                    projected.mode,
                    self.hover,
                    &self.theme,
                    self.charset,
                    &mut self.hits,
                );
                draw_resize_grip(frame, panel, self.hover, &self.theme, &mut self.hits);
                let body_rect = body_rect.intersection(frame.area());
                if body_rect.width > 0 && body_rect.height > 0 {
                    draw_panel_body(frame, body_rect, meta.stable_id.as_ref(), &mut body);
                }
            }

            draw_dock(
                frame,
                rect_from_region(projected.chrome.dock),
                projected.dock,
                DockRenderContext {
                    catalog: &self.catalog,
                    label: &self.chrome.dock_label,
                    theme: &self.theme,
                    charset: self.charset,
                },
                &mut self.hits,
            );
            draw_workspace_scrollbar(frame, &projected, &self.theme);
            self.snapshot.workspace_scroll = projected.workspace_scroll;
        }

        fn host_consumes_primary_down(&mut self, event: &WebMouseEvent, at: Position) -> bool {
            if !matches!(
                event.kind,
                WebMouseEventKind::ButtonDown(WebMouseButton::Left)
            ) {
                return false;
            }
            if let Some((_, i)) = self.badge_zones.iter().find(|(r, _)| r.contains(at)) {
                let action = self.badges[*i].primary_action();
                self.actions.push(format!("{action:?}"));
                return true;
            }
            if self.theme_zone.contains(at) {
                self.toggle_theme();
                return true;
            }
            false
        }

        fn restore_panel_by_number(&mut self, ch: char) {
            let Some(index) = ch.to_digit(10).and_then(|digit| digit.checked_sub(1)) else {
                return;
            };
            let Some(stable_id) = RESTORE_IDS.get(index as usize) else {
                return;
            };
            let Some(key) = self
                .catalog
                .get_by_stable_id(stable_id)
                .map(|meta| meta.key)
            else {
                return;
            };
            self.reduce_workspace_event(WorkspaceEvent::Pointer {
                target: HitTarget::Dock { key },
                event: PointerEvent {
                    kind: PointerEventKind::Down(PointerButton::Primary),
                    x: 0.0,
                    y: 0.0,
                },
            });
        }

        fn reduce_workspace_event(&mut self, event: WorkspaceEvent<SpecPanelId>) {
            let surface = self.surface_profile();
            let clamp = self.layout.clamp;
            let tile = self.layout.tile.resize;
            let context = panel_kit_core::reducer::ReduceContext {
                surface,
                clamp: &clamp,
                command_step: self.input.steps,
                tile: &tile,
                snap: panel_kit_core::SnapPolicy::default(),
            };
            let reduction = reduce(&mut self.snapshot, event, context);
            if !self.layout_ready {
                return;
            }
            let Some(store) = self.store.as_ref() else {
                return;
            };
            let decision = self.save_policy.decide(&reduction);
            expect_persistence(apply_save_decision(
                decision,
                store,
                &self.snapshot,
                &self.catalog,
            ));
        }

        fn restore_pending_layout(&mut self) -> Result<(), LayoutError> {
            if self.layout_ready {
                return Ok(());
            }
            self.layout_ready = true;
            if !self.persistence.restore {
                return Ok(());
            }
            let Some(store) = self.store.as_ref() else {
                return Ok(());
            };
            self.snapshot = restore_snapshot(
                store,
                self.snapshot.clone(),
                &self.catalog,
                RestoreContext {
                    units: Units::Cells,
                    viewport: (self.snapshot.viewport.width, self.snapshot.viewport.height),
                },
            )?;
            Ok(())
        }

        fn sync_viewport(&mut self, area: Rect) {
            self.reduce_workspace_event(WorkspaceEvent::ViewportChanged {
                size: Viewport {
                    width: area.width as f64,
                    height: area.height as f64,
                    units: Units::Cells,
                },
                policy: self.surface_spec.resize_policy,
            });
        }

        fn toggle_theme(&mut self) {
            self.paper = !self.paper;
            self.theme = if self.paper {
                ResolvedTuiTheme::from(&ThemeTokens::paper())
            } else {
                ResolvedTuiTheme::from(&ThemeTokens::dark())
            };
        }

        fn surface_profile(&self) -> SurfaceProfile {
            SurfaceProfile::from_logical_width(
                self.snapshot.viewport.width,
                self.surface_spec.compact_max,
                self.surface_spec.tablet_max,
                self.surface_spec.fallback_capabilities,
            )
        }

        fn tile_metrics(&self, surface: SurfaceProfile) -> TileLayoutMetrics {
            TileLayoutMetrics {
                resize: self.layout.tile.resize,
                columns: surface.tile_columns(),
                row_min: self.layout.tile.row_min,
                gap: self.layout.tile.gap,
                padding: self.layout.tile.padding,
                fill_viewport: self.layout.tile.fill_viewport,
                fill_order: panel_kit_core::frame::TileFillOrder::RowMajor,
            }
        }

        fn chrome_projection(&self) -> ChromeProjectionInput {
            ChromeProjectionInput {
                metrics: self.chrome.metrics,
                hit_target_min: self.chrome.hit_target_min,
                panel_header_h: self.chrome.panel_header_h,
                panel_frame: self.chrome.panel_frame,
                title_in_border: self.chrome.title_in_border,
                mode_control: self.chrome.mode_control,
                minimize_control: self.chrome.minimize_control,
                maximize_control: self.chrome.maximize_control,
                resize_grip: self.chrome.resize_grip,
                dock: self.chrome.dock,
            }
        }
    }

    fn tui_charset(charset: panel_kit_core::Charset) -> Charset {
        match charset {
            panel_kit_core::Charset::Unicode => Charset::Unicode,
            panel_kit_core::Charset::Ascii => Charset::Ascii,
        }
    }

    fn rect_from_region(region: Region) -> Rect {
        Rect::new(
            region.x as u16,
            region.y as u16,
            region.w as u16,
            region.h as u16,
        )
    }

    fn install_wheel_translation(app: Rc<RefCell<App>>) -> Result<(), JsValue> {
        let document = ratzilla::web_sys::window()
            .and_then(|window| window.document())
            .ok_or_else(|| JsValue::from_str("document unavailable"))?;
        let target = document
            .get_element_by_id("panel-kit-tui")
            .ok_or_else(|| JsValue::from_str("panel-kit-tui element unavailable"))?;
        let wheel = Closure::wrap(Box::new(move |event: JsValue| {
            let delta = Reflect::get(&event, &JsValue::from_str("deltaY"))
                .ok()
                .and_then(|value| value.as_f64())
                .unwrap_or(0.0);
            if delta != 0.0 {
                app.borrow_mut()
                    .reduce_workspace_event(WorkspaceEvent::Wheel {
                        delta_y: delta.signum() * 3.0,
                        disposition: WheelDisposition::BubbleToWorkspace,
                    });
            }
            if let Ok(prevent_default) = Reflect::get(&event, &JsValue::from_str("preventDefault"))
                .and_then(|value| value.dyn_into::<Function>())
            {
                let _ = prevent_default.call0(&event);
            }
        }) as Box<dyn FnMut(JsValue)>);
        target.add_event_listener_with_callback("wheel", wheel.as_ref().unchecked_ref())?;
        wheel.forget();
        Ok(())
    }

    pub fn main() -> Result<(), Box<dyn std::error::Error>> {
        std::panic::set_hook(Box::new(console_error_panic_hook::hook));
        let backend = WebGl2Backend::new_with_options(
            WebGl2BackendOptions::new()
                .grid_id("panel-kit-tui")
                .cursor_shape(CursorShape::None)
                .canvas_padding_color(Color::Black)
                .font_atlas_config(FontAtlasConfig::dynamic(
                    &["Fira Code", "JetBrains Mono", "monospace"],
                    16.0,
                )),
        )?;
        let mut terminal = ratatui::Terminal::new(backend)?;
        let app = Rc::new(RefCell::new(App::new()?));

        install_wheel_translation(app.clone())
            .map_err(|error| std::io::Error::other(format!("{error:?}")))?;
        terminal.on_key_event({
            let app = app.clone();
            move |key| app.borrow_mut().handle_key(key)
        })?;

        terminal.on_mouse_event({
            let app = app.clone();
            move |event| app.borrow_mut().handle_mouse(event)
        })?;

        terminal.draw_web(move |frame| app.borrow_mut().draw(frame));
        Ok(())
    }

    fn expect_persistence<T>(result: Result<T, LayoutError>) -> T {
        result.expect("layout persistence should succeed in the browser TUI canary")
    }
}

#[cfg(target_arch = "wasm32")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    if let Err(e) = browser::main() {
        ratzilla::web_sys::console::error_1(&format!("browser_tui error: {e:?}").into());
        return Err(e);
    }
    Ok(())
}
