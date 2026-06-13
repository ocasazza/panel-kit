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
//! same `TuiWorkspace` chrome as the terminal example plus badges, spinner,
//! theming, scrollable content, and charts. That makes it useful both as
//! docs-as-code and as a browser/WASM canary.

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    eprintln!(
        "browser_tui is a WASM/Ratzilla example. Run it with `trunk serve \
         crates/panel-kit-tui/browser_tui.html --example browser_tui`."
    );
}

#[cfg(target_arch = "wasm32")]
mod browser {
    use std::{cell::RefCell, rc::Rc};

    use panel_kit_core::badge::{tag_hue, BadgeClickKind, BadgeKind};
    use panel_kit_core::{LayoutBuilder, Mode, PanelKind, PanelWin};
    use panel_kit_tui::badge::{hue_color, Badge};
    use panel_kit_tui::charts::{gauges, time_series, GaugeItem, Series};
    use panel_kit_tui::scroll;
    use panel_kit_tui::spinner::spinner;
    use panel_kit_tui::{Theme, TuiMouseButton, TuiMouseEvent, TuiMouseEventKind, TuiWorkspace};
    use ratatui::layout::{Position, Rect};
    use ratatui::style::{Color, Style};
    use ratatui::text::{Line, Span};
    use ratatui::widgets::Paragraph;
    use ratzilla::event::{
        KeyCode, MouseButton as WebMouseButton, MouseEvent as WebMouseEvent,
        MouseEventKind as WebMouseEventKind,
    };
    use ratzilla::{
        backend::webgl2::{FontAtlasConfig, WebGl2BackendOptions},
        CursorShape, WebGl2Backend, WebRenderer,
    };
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
    enum Panel {
        Workspace,
        Badges,
        Activity,
        Capacity,
        Notes,
        Theme,
    }

    impl PanelKind for Panel {
        fn title(self) -> &'static str {
            match self {
                Panel::Workspace => "Workspace",
                Panel::Badges => "Badges",
                Panel::Activity => "Activity",
                Panel::Capacity => "Capacity",
                Panel::Notes => "Notes",
                Panel::Theme => "Theme",
            }
        }
    }

    fn defaults() -> Vec<PanelWin<Panel>> {
        let mut b = LayoutBuilder::new();
        vec![
            b.at(Panel::Workspace, 1.0, 0.0, 48.0, 12.0).with_tile(2, 2),
            b.at(Panel::Badges, 51.0, 0.0, 58.0, 16.0).with_tile(2, 3),
            b.at(Panel::Activity, 1.0, 13.0, 58.0, 15.0).with_tile(2, 3),
            b.at(Panel::Capacity, 61.0, 17.0, 48.0, 10.0)
                .with_tile(2, 2),
            b.at(Panel::Notes, 1.0, 29.0, 64.0, 12.0).with_tile(3, 2),
            b.at(Panel::Theme, 67.0, 28.0, 42.0, 10.0).with_tile(1, 2),
        ]
    }

    fn demo_badges() -> Vec<Badge> {
        let mut tag = Badge::new(BadgeKind::Tag, "tag", "browser-tui");
        tag.override_color = Some(hue_color(tag_hue("browser-tui")));
        let mut active = Badge::new(BadgeKind::Status, "status", "canary");
        active.active = true;
        vec![
            tag,
            Badge::new(BadgeKind::Doctype, "doctype", "example"),
            Badge::new(BadgeKind::Folder, "folder", "crates/panel-kit-tui"),
            Badge::new(BadgeKind::Author, "author", "olive"),
            Badge::new(
                BadgeKind::Entity {
                    ty: Some("crate".into()),
                },
                "entity",
                "panel-kit-core",
            ),
            Badge::new(
                BadgeKind::Wikilink {
                    resolved: true,
                    target: "TuiWorkspace".into(),
                },
                "link",
                "TuiWorkspace",
            ),
            Badge::new(
                BadgeKind::Wikilink {
                    resolved: false,
                    target: "missing-doc".into(),
                },
                "link",
                "missing-doc",
            ),
            Badge::new(
                BadgeKind::Url {
                    href: "https://github.com/ratatui/ratzilla".into(),
                    host: "github.com".into(),
                },
                "url",
                "ratzilla",
            ),
            Badge::new(BadgeKind::Date, "date", "2026-06-13"),
            active,
            Badge::new(BadgeKind::Generic, "mode", "wasm"),
        ]
    }

    const EVAL_SERIES: &[(f64, f64)] = &[
        (0.0, 8.0),
        (1.0, 12.0),
        (2.0, 11.0),
        (3.0, 19.0),
        (4.0, 24.0),
        (5.0, 21.0),
        (6.0, 28.0),
        (7.0, 34.0),
        (8.0, 31.0),
    ];
    const FRAME_SERIES: &[(f64, f64)] = &[
        (0.0, 16.0),
        (1.0, 16.5),
        (2.0, 16.1),
        (3.0, 17.2),
        (4.0, 16.4),
        (5.0, 16.0),
        (6.0, 15.9),
        (7.0, 16.3),
        (8.0, 16.1),
    ];

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
    }

    impl App {
        fn new() -> Self {
            let mut ws = TuiWorkspace::new(None, defaults);
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
            }
        }

        fn handle_key(&mut self, key: KeyCode) {
            match key {
                KeyCode::Char('t') => {
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
                KeyCode::Up => {
                    self.notes_scroll = self.notes_scroll.saturating_sub(1);
                }
                KeyCode::Down => {
                    self.notes_scroll = self.notes_scroll.saturating_add(1);
                }
                _ => {}
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

            if let Some(event) = self.to_tui_mouse(event) {
                self.ws.handle_mouse(event);
            }
        }

        fn to_tui_mouse(&mut self, event: WebMouseEvent) -> Option<TuiMouseEvent> {
            let kind = match event.kind {
                WebMouseEventKind::ButtonDown(WebMouseButton::Left) => {
                    self.mouse_down = true;
                    TuiMouseEventKind::Down(TuiMouseButton::Primary)
                }
                WebMouseEventKind::ButtonUp(WebMouseButton::Left) => {
                    self.mouse_down = false;
                    TuiMouseEventKind::Up(TuiMouseButton::Primary)
                }
                WebMouseEventKind::Moved if self.mouse_down => {
                    TuiMouseEventKind::Drag(TuiMouseButton::Primary)
                }
                WebMouseEventKind::Moved => TuiMouseEventKind::Moved,
                WebMouseEventKind::SingleClick(WebMouseButton::Left) => {
                    TuiMouseEventKind::Up(TuiMouseButton::Primary)
                }
                _ => return None,
            };

            Some(TuiMouseEvent {
                kind,
                x: event.col as f64,
                y: event.row as f64,
            })
        }

        fn draw(&mut self, frame: &mut ratatui::Frame) {
            self.tick += 1;
            self.badge_zones.clear();
            let theme = self.ws.theme;
            let actions = self.actions.clone();
            let tick = self.tick;
            let paper = self.paper;
            self.ws.render(frame, frame.area(), &mut |f, rect, kind, _max| match kind {
                Panel::Workspace => {
                    f.render_widget(
                        Paragraph::new(vec![
                            Line::from(vec![
                                Span::styled("panel-kit-tui in the browser", Style::default().fg(theme.fg)),
                            ]),
                            Line::from(""),
                            Line::from("Ratzilla renders the ratatui shell to a WebGL canvas."),
                            Line::from("The state machine is shared with the Dioxus renderer."),
                            Line::from(""),
                            Line::from("Mouse: drag headers, drag the corner grip, click lights."),
                            Line::from("Keys: t swaps theme, 1-6 restore panels, arrows scroll notes."),
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
                        .map(|a| Line::from(Span::styled(a.clone(), Style::default().fg(theme.accent))))
                        .collect();
                    if log_y > rect.y {
                        f.render_widget(
                            Paragraph::new(recent),
                            Rect::new(rect.x, log_y, rect.width, 3.min(rect.height)),
                        );
                    }
                }
                Panel::Activity => {
                    let series = [
                        Series { name: "eval/ms".into(), points: EVAL_SERIES },
                        Series { name: "frame/ms".into(), points: FRAME_SERIES },
                    ];
                    time_series(f, rect, &theme, "ms", &series);
                }
                Panel::Capacity => {
                    let items = [
                        GaugeItem { label: "vfs".into(), ratio: 0.21, text: "31 / 148 files".into() },
                        GaugeItem { label: "wasm".into(), ratio: 0.63, text: "6.3 MB / 10 MB".into() },
                        GaugeItem { label: "events".into(), ratio: 0.78, text: "78% queue".into() },
                        GaugeItem { label: "layout".into(), ratio: 0.94, text: "94% stress".into() },
                    ];
                    gauges(f, rect, &theme, &items);
                }
                Panel::Notes => {
                    let mut lines = vec![
                        Line::from(Span::styled("docs-as-code canary", Style::default().fg(theme.fg))),
                        Line::from(""),
                    ];
                    for text in [
                        "This browser example compiles the ratatui renderer to wasm.",
                        "It exercises workspace panels, traffic lights, drag math, restore hooks, badges, action routing, charts, gauges, spinner frames, theming, and scrollbars.",
                        "When this builds under Trunk, the TUI path is still web-capable.",
                        "When the native terminal example builds, the same public TUI API is still terminal-capable.",
                        "Keeping both examples broad catches drift between core, Dioxus, and TUI renderers.",
                        "The example is not a screenshot fixture: it is executable documentation.",
                        "Use t for theme, 1-6 to restore minimized panels, and arrow keys to scroll this panel.",
                    ] {
                        lines.push(Line::from(text));
                    }
                    lines.push(Line::from(""));
                    lines.push(spinner(tick, "browser TUI canary running", &theme));
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
                                if paper { "preset: paper (click or press t)" } else { "preset: dark (click or press t)" },
                                Style::default().fg(theme.fg),
                            )),
                            sw(theme.accent, "accent"),
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

        terminal.on_key_event({
            let app = app.clone();
            move |key| app.borrow_mut().handle_key(key.code)
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
