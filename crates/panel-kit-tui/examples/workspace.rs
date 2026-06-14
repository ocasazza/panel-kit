//! Native terminal backend for the shared panel-kit TUI canary.
//!
//! The browser/WASM example and this terminal example intentionally render
//! the same seven-panel workspace. Only the backend loop differs.
//!
//! Run: `cargo run -p panel-kit-tui --example workspace`
//! Mouse: drag headers to move/reorder, drag the corner grip to resize,
//! click lights, click badges, click Theme to swap presets, dock chips
//! restore minimized panels. Keys: `t`, `1`-`6`, arrows, `q`.

mod workspace_canary;

use std::time::Duration;

use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, MouseButton, MouseEventKind,
};
use crossterm::execute;
use panel_kit_core::badge::BadgeClickKind;
use panel_kit_core::Mode;
use panel_kit_tui::badge::Badge;
use panel_kit_tui::charts::{boxplot, flame, gauges, time_series};
use panel_kit_tui::scroll;
use panel_kit_tui::spinner::spinner;
use panel_kit_tui::{Theme, TuiMouseButton, TuiMouseEvent, TuiMouseEventKind, TuiWorkspace};
use ratatui::layout::{Position, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use workspace_canary::{capacity_items, defaults, demo_badges, node_rows, Metrics, Panel};

fn to_tui_mouse(m: crossterm::event::MouseEvent) -> Option<TuiMouseEvent> {
    let kind = match m.kind {
        MouseEventKind::Down(MouseButton::Left) => TuiMouseEventKind::Down(TuiMouseButton::Primary),
        MouseEventKind::Up(MouseButton::Left) => TuiMouseEventKind::Up(TuiMouseButton::Primary),
        MouseEventKind::Drag(MouseButton::Left) => TuiMouseEventKind::Drag(TuiMouseButton::Primary),
        MouseEventKind::Moved => TuiMouseEventKind::Moved,
        MouseEventKind::ScrollDown => TuiMouseEventKind::Scroll { delta_y: 1.0 },
        MouseEventKind::ScrollUp => TuiMouseEventKind::Scroll { delta_y: -1.0 },
        _ => return None,
    };
    Some(TuiMouseEvent {
        kind,
        x: m.column as f64,
        y: m.row as f64,
    })
}

struct Demo {
    badges: Vec<Badge>,
    badge_zones: Vec<(Rect, usize)>,
    theme_zone: Rect,
    actions: Vec<String>,
    notes_scroll: usize,
    paper: bool,
    tick: u64,
    metrics: Metrics,
}

impl Demo {
    fn new() -> Self {
        Self {
            badges: demo_badges(),
            badge_zones: Vec::new(),
            theme_zone: Rect::default(),
            actions: Vec::new(),
            notes_scroll: 0,
            paper: false,
            tick: 0,
            metrics: Metrics::new(),
        }
    }

    fn handle_key(&mut self, ws: &mut TuiWorkspace<Panel>, key: KeyCode) -> bool {
        match key {
            KeyCode::Char('q') => return true,
            KeyCode::Char('t') => self.toggle_theme(ws),
            KeyCode::Char('1') => ws.restore_panel(Panel::Workspace),
            KeyCode::Char('2') => ws.restore_panel(Panel::Badges),
            KeyCode::Char('3') => ws.restore_panel(Panel::Activity),
            KeyCode::Char('4') => ws.restore_panel(Panel::Capacity),
            KeyCode::Char('5') => ws.restore_panel(Panel::Notes),
            KeyCode::Char('6') => ws.restore_panel(Panel::Theme),
            KeyCode::Char('7') => ws.restore_panel(Panel::Nodes),
            KeyCode::Char('8') => ws.restore_panel(Panel::Flame),
            KeyCode::Char('9') => ws.restore_panel(Panel::Distribution),
            KeyCode::Up => {
                self.notes_scroll = self.notes_scroll.saturating_sub(1);
            }
            KeyCode::Down => {
                self.notes_scroll = self.notes_scroll.saturating_add(1);
            }
            KeyCode::PageUp => ws.scroll_by(-4.0),
            KeyCode::PageDown => ws.scroll_by(4.0),
            _ => {}
        }
        false
    }

    fn handle_mouse(&mut self, ws: &mut TuiWorkspace<Panel>, m: crossterm::event::MouseEvent) {
        let at = Position::new(m.column, m.row);
        if m.kind == MouseEventKind::Down(MouseButton::Left) {
            if let Some((_, i)) = self.badge_zones.iter().find(|(r, _)| r.contains(at)) {
                let action = self.badges[*i].click(BadgeClickKind::Toggle);
                self.actions.push(format!("{action:?}"));
                return;
            }
            if self.theme_zone.contains(at) {
                self.toggle_theme(ws);
                return;
            }
        }
        if let Some(m) = to_tui_mouse(m) {
            ws.handle_mouse(m);
        }
    }

    fn toggle_theme(&mut self, ws: &mut TuiWorkspace<Panel>) {
        self.paper = !self.paper;
        ws.theme = if self.paper {
            Theme::PAPER
        } else {
            Theme::DARK
        };
    }

    fn draw(&mut self, ws: &mut TuiWorkspace<Panel>, frame: &mut ratatui::Frame) {
        self.tick += 1;
        self.metrics.tick();
        self.badge_zones.clear();
        let theme = ws.theme;
        let actions = self.actions.clone();
        let tick = self.tick;
        let paper = self.paper;
        let metrics = &self.metrics;
        ws.render(frame, frame.area(), &mut |f, rect, kind, _max| match kind {
            Panel::Workspace => {
                f.render_widget(
                    Paragraph::new(vec![
                        Line::from(vec![Span::styled(
                            "panel-kit-tui backend canary",
                            Style::default().fg(theme.fg),
                        )]),
                        Line::from(""),
                        Line::from("The same ratatui workspace renders in terminal and browser."),
                        Line::from("The state machine is shared with the Dioxus renderer."),
                        Line::from(""),
                        Line::from("Mouse: drag headers, drag the corner grip, click lights."),
                        Line::from("Keys: t swaps theme, 1-9 restore panels, arrows scroll notes."),
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
                        let color = if *ok { theme.green } else { theme.red };
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
                    Line::from(Span::styled(
                        "docs-as-code canary",
                        Style::default().fg(theme.fg),
                    )),
                    Line::from(""),
                ];
                for text in [
                    "This example renders the ratatui workspace through a backend.",
                    "It exercises workspace panels, traffic lights, drag math, restore hooks, badges, action routing, charts, gauges, spinner frames, theming, and scrollbars.",
                    "When this native terminal example builds, the TUI path is still terminal-capable.",
                    "When the browser example builds under Trunk, the same public TUI API is still web-capable.",
                    "Keeping both examples broad catches drift between core, Dioxus, and TUI renderers.",
                    "The example is not a screenshot fixture: it is executable documentation.",
                    "Use t for theme, 1-9 to restore minimized panels, and arrow keys to scroll this panel.",
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
                            if paper {
                                "preset: paper (click or press t)"
                            } else {
                                "preset: dark (click or press t)"
                            },
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

fn main() -> std::io::Result<()> {
    let store = std::env::temp_dir().join("panel-kit-tui-demo.json");
    let mut ws = TuiWorkspace::new(Some(store), defaults);
    ws.mode = Mode::Tiling;
    let mut demo = Demo::new();

    let mut terminal = ratatui::init();
    let _ = execute!(std::io::stdout(), EnableMouseCapture);
    loop {
        terminal.draw(|f| demo.draw(&mut ws, f))?;
        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(k) if demo.handle_key(&mut ws, k.code) => break,
                Event::Mouse(m) => demo.handle_mouse(&mut ws, m),
                _ => {}
            }
        }
    }
    let _ = execute!(std::io::stdout(), DisableMouseCapture);
    ratatui::restore();
    Ok(())
}
