//! Browser canary for the ratatui renderer.
//!
//! Run with:
//!
//! ```sh
//! trunk serve crates/panel-kit-tui/browser_tui.html \
//!   --example browser_tui \
//!   --address 127.0.0.1 \
//!   --port 8082
//! ```
//!
//! This is intentionally comprehensive rather than cute: it exercises the
//! same `TuiWorkspace` chrome as the terminal example plus keyboard window
//! management, compact/tablet/regular surface tiers, V2 localStorage
//! persistence, wheel scrolling, badges, spinner, theming, and charts.

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    eprintln!(
        "browser_tui is a WASM/Ratzilla example. Run it with `trunk serve \
         crates/panel-kit-tui/browser_tui.html --example browser_tui`."
    );
}

#[cfg(target_arch = "wasm32")]
mod workspace_canary;

#[cfg(target_arch = "wasm32")]
mod browser {
    use std::{cell::RefCell, rc::Rc};

    use panel_kit_core::badge::{BadgeClickKind, Rgb};
    use panel_kit_core::{
        FocusContext, Key, KeyChord, Mode, PointerButton, PointerEvent, PointerEventKind,
    };
    use panel_kit_tui::badge::Badge;
    use panel_kit_tui::charts::{boxplot, flame, gauges, time_series};
    use panel_kit_tui::scroll;
    use panel_kit_tui::spinner::spinner;
    use panel_kit_tui::{Charset, LayoutStore, Theme, TuiWorkspace};
    use ratatui::layout::{Position, Rect};
    use ratatui::style::{Color, Style};
    use ratatui::text::{Line, Span};
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

    use crate::workspace_canary::{
        capacity_items, defaults, demo_badges, node_rows, Metrics, Panel,
    };

    const STORAGE_KEY: &str = "panel-kit-tui-layout-v2";

    struct BrowserStore;

    impl BrowserStore {
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

    impl LayoutStore for BrowserStore {
        fn load(&self) -> Result<Option<String>, String> {
            let storage = Self::storage()?;
            let value = Self::method(&storage, "getItem")?
                .call1(&storage, &JsValue::from_str(STORAGE_KEY))
                .map_err(|error| format!("{error:?}"))?;
            Ok(value.as_string())
        }

        fn save(&self, json: &str) -> Result<(), String> {
            let storage = Self::storage()?;
            Self::method(&storage, "setItem")?
                .call2(
                    &storage,
                    &JsValue::from_str(STORAGE_KEY),
                    &JsValue::from_str(json),
                )
                .map_err(|error| format!("{error:?}"))?;
            Ok(())
        }
    }

    fn to_key_chord(event: KeyEvent) -> Option<KeyChord> {
        let key = match event.code {
            KeyCode::Left => Key::Left,
            KeyCode::Right => Key::Right,
            KeyCode::Up => Key::Up,
            KeyCode::Down => Key::Down,
            KeyCode::Enter => Key::Enter,
            KeyCode::Esc => Key::Escape,
            KeyCode::Tab => Key::Tab,
            KeyCode::Char(ch) => Key::Char(ch),
            _ => return None,
        };
        Some(KeyChord {
            key,
            shift: event.shift,
            alt: event.alt,
            ctrl: event.ctrl,
            meta: false,
        })
    }

    fn rgb(color: Color) -> Rgb {
        match color {
            Color::Rgb(r, g, b) => (r, g, b),
            _ => unreachable!("panel-kit themes use RGB colors"),
        }
    }

    struct App {
        ws: TuiWorkspace<Panel>,
        badges: Vec<Badge>,
        badge_zones: Vec<(Rect, usize)>,
        theme_zone: Rect,
        actions: Vec<String>,
        notes_scroll: usize,
        paper: bool,
        tick: u64,
        mouse_down: bool,
        metrics: Metrics,
    }

    impl App {
        fn new() -> Self {
            // WebGL font atlas can't render box-drawing / geometric glyphs.
            let mut ws =
                TuiWorkspace::with_store(Box::new(BrowserStore), defaults).charset(Charset::Ascii);
            ws.mode = Mode::Tiling;
            Self {
                ws,
                badges: demo_badges(),
                badge_zones: Vec::new(),
                theme_zone: Rect::default(),
                actions: Vec::new(),
                notes_scroll: 0,
                paper: false,
                tick: 0,
                mouse_down: false,
                metrics: Metrics::new(),
            }
        }

        fn handle_key(&mut self, event: KeyEvent) {
            match event.code.clone() {
                KeyCode::Char('p') => {
                    self.paper = !self.paper;
                    self.ws.theme = if self.paper {
                        Theme::PAPER
                    } else {
                        Theme::DARK
                    };
                }
                KeyCode::Char('1') => self.ws.restore_panel(Panel::Workspace),
                KeyCode::Char('2') => self.ws.restore_panel(Panel::Badges),
                KeyCode::Char('3') => self.ws.restore_panel(Panel::Activity),
                KeyCode::Char('4') => self.ws.restore_panel(Panel::Capacity),
                KeyCode::Char('5') => self.ws.restore_panel(Panel::Notes),
                KeyCode::Char('6') => self.ws.restore_panel(Panel::Theme),
                KeyCode::Char('7') => self.ws.restore_panel(Panel::Nodes),
                KeyCode::Char('8') => self.ws.restore_panel(Panel::Flame),
                KeyCode::Char('9') => self.ws.restore_panel(Panel::Distribution),
                KeyCode::PageUp => self.ws.scroll_by(-4.0),
                KeyCode::PageDown => self.ws.scroll_by(4.0),
                _ => {
                    if let Some(chord) = to_key_chord(event) {
                        self.ws.handle_key(chord, FocusContext::Workspace);
                    }
                }
            }
        }

        fn handle_mouse(&mut self, event: WebMouseEvent) {
            let at = Position::new(event.col, event.row);
            if matches!(
                event.kind,
                WebMouseEventKind::ButtonDown(WebMouseButton::Left)
            ) {
                if let Some((_, i)) = self.badge_zones.iter().find(|(r, _)| r.contains(at)) {
                    let action = self.badges[*i].click(BadgeClickKind::Toggle);
                    self.actions.push(format!("{action:?}"));
                    return;
                }
                if self.theme_zone.contains(at) {
                    self.paper = !self.paper;
                    self.ws.theme = if self.paper {
                        Theme::PAPER
                    } else {
                        Theme::DARK
                    };
                    return;
                }
            }

            if let Some(event) = self.to_pointer_event(event) {
                self.ws.handle_mouse(event);
            }
        }

        #[allow(clippy::wrong_self_convention)]
        fn to_pointer_event(&mut self, event: WebMouseEvent) -> Option<PointerEvent> {
            let kind = match event.kind {
                WebMouseEventKind::ButtonDown(WebMouseButton::Left) => {
                    self.mouse_down = true;
                    PointerEventKind::Down(PointerButton::Primary)
                }
                WebMouseEventKind::ButtonUp(WebMouseButton::Left) => {
                    self.mouse_down = false;
                    PointerEventKind::Up(PointerButton::Primary)
                }
                WebMouseEventKind::Moved if self.mouse_down => {
                    PointerEventKind::Drag(PointerButton::Primary)
                }
                WebMouseEventKind::Moved => PointerEventKind::Moved,
                WebMouseEventKind::SingleClick(WebMouseButton::Left) => {
                    PointerEventKind::Up(PointerButton::Primary)
                }
                _ => return None,
            };

            Some(PointerEvent {
                kind,
                x: event.col as f64,
                y: event.row as f64,
            })
        }

        fn draw(&mut self, frame: &mut ratatui::Frame) {
            self.tick += 1;
            self.metrics.tick();
            self.badge_zones.clear();
            let theme = self.ws.theme;
            let actions = self.actions.clone();
            let tick = self.tick;
            let paper = self.paper;
            let metrics = &self.metrics;
            let surface = self.ws.surface_profile().class;
            let mode = match self.ws.effective_mode() {
                Mode::Floating => "floating",
                Mode::Tiling => "tiling",
            };
            self.ws.render(frame, frame.area(), &mut |f, rect, kind, _max| match kind {
                Panel::Workspace => {
                    f.render_widget(
                        Paragraph::new(vec![
                            Line::from(vec![
                                Span::styled("panel-kit-tui backend canary", Style::default().fg(theme.fg)),
                            ]),
                            Line::from(""),
                            Line::from("The same ratatui workspace renders in terminal and browser."),
                            Line::from(format!("Surface: {surface:?} · effective mode: {mode}")),
                            Line::from("Persistence: schema V2 · Units::Cells · browser localStorage."),
                            Line::from(""),
                            Line::from("Mouse: drag headers/grip, click lights; wheel scrolls workspace."),
                            Line::from("Keys: arrows move, Shift resizes, Alt fine-moves; m/f/t, Tab, Enter."),
                            Line::from("Palette: p · restore: 1-9 · workspace scroll: PgUp/PgDn."),
                        ])
                        .style(Style::default().fg(theme.dim)),
                        rect,
                    );
                }
                Panel::Badges => {
                    for (row, (i, b)) in self.badges.iter().enumerate().enumerate() {
                        if row as u16 >= rect.height.saturating_sub(4) {
                            break;
                        }
                        let r = Rect::new(rect.x, rect.y + row as u16, b.width().min(rect.width), 1);
                        self.badge_zones.push((r, i));
                        f.render_widget(Paragraph::new(Line::from(b.spans(&theme))), r);
                    }
                    let log_y = rect.y + rect.height.saturating_sub(3);
                    let recent: Vec<Line> = actions
                        .iter()
                        .rev()
                        .take(3)
                        .map(|a| Line::from(Span::styled(a.clone(), Style::default().fg(theme.badge_info))))
                        .collect();
                    if log_y > rect.y {
                        f.render_widget(
                            Paragraph::new(recent),
                            Rect::new(rect.x, log_y, rect.width, 3.min(rect.height)),
                        );
                    }
                }
                Panel::Activity => {
                    time_series(f, rect, &theme, "ms", &metrics.series());
                }
                Panel::Capacity => {
                    let items = capacity_items();
                    gauges(f, rect, &theme, &items);
                }
                Panel::Flame => {
                    flame(f, rect, &theme, &metrics.flame());
                }
                Panel::Distribution => {
                    boxplot(f, rect, &theme, &metrics.boxes());
                }
                Panel::Nodes => {
                    let rows: Vec<ratatui::widgets::Row> = node_rows()
                        .iter()
                        .map(|(name, ok, load, detail)| {
                            let color = if *ok { rgb(theme.green) } else { rgb(theme.red) };
                            ratatui::widgets::Row::new(vec![
                                ratatui::widgets::Cell::from(panel_kit_tui::status::labeled(color, *name)),
                                ratatui::widgets::Cell::from(panel_kit_tui::meter::span(*load, 8, color)),
                                ratatui::widgets::Cell::from(*detail),
                            ])
                        })
                        .collect();
                    panel_kit_tui::table::table(
                        f,
                        rect,
                        &theme,
                        &["node", "load", ""],
                        &[
                            ratatui::layout::Constraint::Length(12),
                            ratatui::layout::Constraint::Length(10),
                            ratatui::layout::Constraint::Length(10),
                        ],
                        rows,
                    );
                }
                Panel::Notes => {
                    let mut lines = vec![
                        Line::from(Span::styled("docs-as-code canary", Style::default().fg(theme.fg))),
                        Line::from(""),
                    ];
                    for text in [
                        "This example renders the ratatui workspace through a backend.",
                        "It exercises workspace panels, traffic lights, drag math, restore hooks, badges, action routing, charts, gauges, spinner frames, theming, and scrollbars.",
                        "When the native terminal example builds, the same public TUI API is still terminal-capable.",
                        "When the browser example builds under Trunk, the same public TUI API is still web-capable.",
                        "Keeping both examples broad catches drift between core, Dioxus, and TUI renderers.",
                        "The example is not a screenshot fixture: it is executable documentation.",
                        "Use p for palette, m/f/t for window state, Tab to cycle focus, 1-9 to restore, and PgUp/PgDn or the wheel to scroll.",
                    ] {
                        lines.push(Line::from(text));
                    }
                    lines.push(Line::from(""));
                    lines.push(spinner(tick, "TUI canary running", &theme));
                    self.notes_scroll = scroll::lines(f, rect, &theme, lines, self.notes_scroll);
                }
                Panel::Theme => {
                    self.theme_zone = rect;
                    let sw = |c, name: &'static str| {
                        Line::from(vec![
                            Span::styled("## ", Style::default().fg(c)),
                            Span::styled(name, Style::default().fg(theme.dim)),
                        ])
                    };
                    f.render_widget(
                        Paragraph::new(vec![
                            Line::from(Span::styled(
                                if paper { "preset: paper (click or press p)" } else { "preset: dark (click or press p)" },
                                Style::default().fg(theme.fg),
                            )),
                            sw(theme.blue, "blue · mode light"),
                            sw(theme.yellow, "yellow · minimize"),
                            sw(theme.pink, "pink · maximize"),
                            spinner(tick / 2, "spinner", &theme),
                        ]),
                        rect,
                    );
                }
            });
        }
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
                app.borrow_mut().ws.handle_mouse(PointerEvent {
                    kind: PointerEventKind::Scroll {
                        delta_y: delta.signum() * 3.0,
                    },
                    x: 0.0,
                    y: 0.0,
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
                .disable_auto_css_resize()
                .font_atlas_config(FontAtlasConfig::dynamic(
                    &["Fira Code", "JetBrains Mono", "monospace"],
                    16.0,
                )),
        )?;
        let mut terminal = ratatui::Terminal::new(backend)?;
        let app = Rc::new(RefCell::new(App::new()));

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
}

#[cfg(target_arch = "wasm32")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    browser::main()
}
