//! Terminal twin of the web examples — all of them in one workspace:
//! the panel surface (floating/tiling, traffic lights, drag/resize/
//! reorder, dock, persistence), every `BadgeKind` with a live action log,
//! the `Spinner`, and the theming override path (dark/paper presets).
//!
//! Run: `cargo run -p panel-kit-tui --example workspace`
//! Mouse: drag headers to move (floating) / reorder (tiling), drag the ◢
//! grip to resize (span-snapped in tiling), hover the lights for their
//! glyphs — blue ⊞/❐ flips mode, yellow − minimizes, pink ⤢ maximizes.
//! Click badges to fire their actions; click the Theme panel to swap
//! presets; dock chips restore. `q` quits.

use std::time::Duration;

use panel_kit_core::badge::{tag_hue, BadgeClickKind, BadgeKind};
use panel_kit_core::{LayoutBuilder, PanelKind, PanelWin};
use panel_kit_tui::badge::{hue_color, Badge};
use panel_kit_tui::spinner::spinner;
use panel_kit_tui::{Theme, TuiWorkspace};
use ratatui::crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, MouseButton, MouseEventKind,
};
use ratatui::crossterm::execute;
use ratatui::layout::{Position, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
enum Panel {
    Workspace,
    Badges,
    Spinner,
    Theme,
}

impl PanelKind for Panel {
    fn title(self) -> &'static str {
        match self {
            Panel::Workspace => "Workspace",
            Panel::Badges => "Badges",
            Panel::Spinner => "Spinner",
            Panel::Theme => "Theme",
        }
    }
}

fn defaults() -> Vec<PanelWin<Panel>> {
    let mut b = LayoutBuilder::new();
    vec![
        b.at(Panel::Workspace, 1.0, 0.0, 46.0, 11.0),
        b.at(Panel::Badges, 49.0, 0.0, 56.0, 16.0).with_tile(2, 3),
        b.at(Panel::Spinner, 1.0, 12.0, 34.0, 7.0),
        b.at(Panel::Theme, 37.0, 12.0, 40.0, 8.0),
    ]
}

fn demo_badges() -> Vec<Badge> {
    let mut tag = Badge::new(BadgeKind::Tag, "tag", "project/alpha");
    tag.override_color = Some(hue_color(tag_hue("project/alpha")));
    let mut active = Badge::new(BadgeKind::Status, "status", "done");
    active.active = true;
    vec![
        tag,
        Badge::new(BadgeKind::Doctype, "doctype", "meeting"),
        Badge::new(BadgeKind::Folder, "folder", "notes/2026"),
        Badge::new(BadgeKind::Author, "author", "olive"),
        Badge::new(
            BadgeKind::Entity { ty: Some("org".into()) },
            "entity",
            "CERN",
        ),
        Badge::new(
            BadgeKind::Wikilink { resolved: true, target: "pilotmesh".into() },
            "link",
            "pilotmesh",
        ),
        Badge::new(
            BadgeKind::Wikilink { resolved: false, target: "missing-note".into() },
            "link",
            "missing-note",
        ),
        Badge::new(
            BadgeKind::Url { href: "https://ratatui.rs".into(), host: "ratatui.rs".into() },
            "url",
            "https://ratatui.rs",
        ),
        Badge::new(BadgeKind::Date, "date", "2026-06-10"),
        active,
        Badge::new(BadgeKind::Generic, "misc", "anything"),
    ]
}

#[derive(Default)]
struct Demo {
    badges: Vec<Badge>,
    badge_zones: Vec<(Rect, usize)>,
    theme_zone: Rect,
    actions: Vec<String>,
    paper: bool,
    tick: u64,
}

fn main() -> std::io::Result<()> {
    let store = std::env::temp_dir().join("panel-kit-tui-demo.json");
    let mut ws = TuiWorkspace::new(Some(store), defaults);
    let mut demo = Demo {
        badges: demo_badges(),
        ..Default::default()
    };

    let mut terminal = ratatui::init();
    let _ = execute!(std::io::stdout(), EnableMouseCapture);
    loop {
        demo.tick += 1;
        let theme = ws.theme;
        terminal.draw(|f| {
            demo.badge_zones.clear();
            ws.render(f, f.area(), &mut |f, rect, kind, _max| match kind {
                Panel::Workspace => {
                    f.render_widget(
                        Paragraph::new(
                            "the same panel state machine the web\n\
                             shell renders to DOM, in terminal cells\n\n\
                             drag headers · ◢ resizes · hover the\n\
                             lights: ⊞ mode · − minimize · ⤢ maximize\n\
                             q quits · layout persists to /tmp",
                        )
                        .style(Style::default().fg(theme.dim)),
                        rect,
                    );
                }
                Panel::Badges => {
                    for (row, (i, b)) in demo.badges.iter().enumerate().enumerate() {
                        if row as u16 >= rect.height.saturating_sub(3) {
                            break;
                        }
                        let r = Rect::new(rect.x, rect.y + row as u16, b.width().min(rect.width), 1);
                        demo.badge_zones.push((r, i));
                        f.render_widget(Paragraph::new(Line::from(b.spans(&theme))), r);
                    }
                    // Live action log, like the web badge example's event list.
                    let log_y = rect.y + rect.height.saturating_sub(2);
                    let recent: Vec<Line> = demo
                        .actions
                        .iter()
                        .rev()
                        .take(2)
                        .map(|a| {
                            Line::from(Span::styled(
                                a.clone(),
                                Style::default().fg(theme.accent),
                            ))
                        })
                        .collect();
                    if log_y > rect.y {
                        f.render_widget(
                            Paragraph::new(recent),
                            Rect::new(rect.x, log_y, rect.width, 2.min(rect.height)),
                        );
                    }
                }
                Panel::Spinner => {
                    f.render_widget(
                        Paragraph::new(vec![
                            spinner(demo.tick, "", &theme),
                            spinner(demo.tick, "indexing…", &theme),
                            spinner(demo.tick / 3, "slow lane", &theme),
                        ]),
                        rect,
                    );
                }
                Panel::Theme => {
                    demo.theme_zone = rect;
                    let sw = |c, name: &'static str| {
                        Line::from(vec![
                            Span::styled("██ ", Style::default().fg(c)),
                            Span::styled(name, Style::default().fg(theme.dim)),
                        ])
                    };
                    f.render_widget(
                        Paragraph::new(vec![
                            Line::from(Span::styled(
                                if demo.paper { "preset: paper (click to swap)" } else { "preset: dark (click to swap)" },
                                Style::default().fg(theme.fg),
                            )),
                            sw(theme.accent, "accent"),
                            sw(theme.blue, "blue · mode light"),
                            sw(theme.yellow, "yellow · minimize"),
                            sw(theme.pink, "pink · maximize"),
                        ]),
                        rect,
                    );
                }
            });
        })?;
        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(k) if k.code == KeyCode::Char('q') => break,
                Event::Mouse(m) => {
                    let at = Position::new(m.column, m.row);
                    if m.kind == MouseEventKind::Down(MouseButton::Left) {
                        if let Some((_, i)) =
                            demo.badge_zones.iter().find(|(r, _)| r.contains(at))
                        {
                            let action = demo.badges[*i].click(BadgeClickKind::Toggle);
                            demo.actions.push(format!("{action:?}"));
                            continue;
                        }
                        if demo.theme_zone.contains(at) {
                            demo.paper = !demo.paper;
                            ws.theme = if demo.paper { Theme::PAPER } else { Theme::DARK };
                            continue;
                        }
                    }
                    ws.handle_mouse(m);
                }
                _ => {}
            }
        }
    }
    let _ = execute!(std::io::stdout(), DisableMouseCapture);
    ratatui::restore();
    Ok(())
}
