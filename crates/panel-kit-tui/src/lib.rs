//! Ratatui shell for panel-kit-core: the same panel workspace the Dioxus
//! crate renders to the DOM, drawn in terminal cells instead.
//!
//! Same state machine, same persisted layout shape, same interactions:
//! drag a panel header to move it (floating) or reorder it (tiling), drag
//! the bottom-right grip to resize (span-snapped in tiling), traffic
//! lights for mode/minimize/maximize, a dock line for minimized panels.
//! Units are character cells; the core math doesn't care.
//!
//! ```no_run
//! use panel_kit_core::{LayoutBuilder, PanelKind, PanelWin};
//! use panel_kit_tui::TuiWorkspace;
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
//! enum Panel { Logs, Stats }
//! impl PanelKind for Panel {
//!     fn title(self) -> &'static str {
//!         match self { Panel::Logs => "Logs", Panel::Stats => "Stats" }
//!     }
//! }
//!
//! fn defaults() -> Vec<PanelWin<Panel>> {
//!     let mut b = LayoutBuilder::new();
//!     vec![b.at(Panel::Logs, 2.0, 1.0, 50.0, 14.0), b.at(Panel::Stats, 54.0, 1.0, 36.0, 10.0)]
//! }
//!
//! let mut ws = TuiWorkspace::new(None, defaults);
//! // in the event loop: ws.render(frame, area, &mut |f, rect, kind, _max| { /* body */ });
//! //                    ws.handle_mouse(mouse_event);
//! ```

#![warn(missing_docs)]

pub mod badge;
pub mod charts;
pub mod scroll;
pub mod spinner;
pub mod theme;

pub use theme::Theme;

use std::path::PathBuf;

use panel_kit_core::{
    apply_drag, begin_drag, begin_tile_resize, effective_rect, front_z, merge_defaults,
    reorder_tile, restore, visible_panels, workspace_chrome, ChromeMetrics, Clamp, Drag, DragKind,
    Mode, PanelKind, PanelWin, PointerButton, PointerEvent, PointerEventKind, Region, SavedLayout,
    TileMetrics, WinState, TILE_W_MAX,
};
use ratatui::layout::{Position, Rect};
use ratatui::style::Style;
use ratatui::symbols::border;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

/// Height of one tiling row unit in cells (`tile_h` counts these).
const ROW_CELLS: f64 = 4.0;

/// WebGL-safe chrome glyphs. Ratzilla's WebGL font atlas does not reliably
/// include box-drawing symbols, so the shared TUI chrome sticks to ASCII.
const ASCII_BORDER: border::Set<'static> = border::Set {
    top_left: "+",
    top_right: "+",
    bottom_left: "+",
    bottom_right: "+",
    vertical_left: "|",
    vertical_right: "|",
    horizontal_top: "-",
    horizontal_bottom: "-",
};

fn rect_from_region(r: Region) -> Rect {
    Rect::new(r.x as u16, r.y as u16, r.w as u16, r.h as u16)
}

fn write_header_text(f: &mut Frame, x: u16, y: u16, max_width: u16, text: &str, style: Style) {
    let area = f.area();
    if y >= area.bottom() || x >= area.right() {
        return;
    }
    let max_width = max_width.min(area.right().saturating_sub(x));
    for (offset, ch) in text.chars().take(max_width as usize).enumerate() {
        f.buffer_mut()[(x + offset as u16, y)]
            .set_char(ch)
            .set_style(style);
    }
}

/// Backwards-compatible alias for the shared pointer button type.
pub type TuiMouseButton = PointerButton;
/// Backwards-compatible alias for the shared pointer event kind.
pub type TuiMouseEventKind = PointerEventKind;
/// Backwards-compatible alias for the shared pointer event type.
pub type TuiMouseEvent = PointerEvent;

/// Per-panel hit zones recorded at draw time, in screen cells.
struct Zone {
    /// Index into `panels`.
    idx: usize,
    /// The whole panel rect.
    panel: Rect,
    /// The header/top-border row (drag-to-move / reorder).
    header: Rect,
    /// The three traffic-light cells: mode, minimize, maximize.
    lights: [Rect; 3],
    /// The bottom-right resize grip cells.
    grip: Rect,
}

/// A panel workspace rendered with ratatui.
///
/// Owns the panel `Vec`, the mode, and the in-flight drag — the exact state
/// the Dioxus shell keeps in signals — and feeds pointer cells into the
/// shared core math. Layout persists as JSON (same [`SavedLayout`] shape the
/// web shell stores in localStorage) when a `store` path is given.
pub struct TuiWorkspace<K: PanelKind> {
    /// All panels with their geometry and window state. `Vec` order is the
    /// tiling order.
    pub panels: Vec<PanelWin<K>>,
    /// The current layout [`Mode`].
    pub mode: Mode,
    /// Chrome palette — defaults to the web shell's dark palette
    /// ([`Theme::DARK`]); swap presets to retheme.
    pub theme: Theme,
    drag: Option<Drag>,
    tile_drag: Option<K>,
    store: Option<PathBuf>,
    ws_area: Rect,
    zones: Vec<Zone>,
    dock_chips: Vec<(Rect, usize)>,
    hover: Option<Position>,
}

impl<K: PanelKind> TuiWorkspace<K> {
    /// Create a workspace: restores the persisted layout from `store` if
    /// given (merging in any panel kinds added since it was saved),
    /// otherwise uses `defaults`.
    pub fn new(store: Option<PathBuf>, defaults: fn() -> Vec<PanelWin<K>>) -> Self {
        let mut panels = None;
        let mut mode = Mode::Floating;
        if let Some(path) = &store {
            if let Ok(raw) = std::fs::read_to_string(path) {
                if let Ok(saved) = serde_json::from_str::<SavedLayout<K>>(&raw) {
                    let mut ps = saved.panels;
                    merge_defaults(&mut ps, &defaults());
                    panels = Some(ps);
                    mode = if saved.tiling {
                        Mode::Tiling
                    } else {
                        Mode::Floating
                    };
                }
            }
        }
        Self {
            panels: panels.unwrap_or_else(defaults),
            mode,
            theme: Theme::default(),
            drag: None,
            tile_drag: None,
            store,
            ws_area: Rect::default(),
            zones: Vec::new(),
            dock_chips: Vec::new(),
            hover: None,
        }
    }

    /// Persist the layout now (also called automatically when a drag
    /// settles). No-op without a store path.
    pub fn save(&self) {
        if let Some(path) = &self.store {
            let saved = SavedLayout {
                panels: self.panels.clone(),
                tiling: self.mode == Mode::Tiling,
            };
            if let Ok(json) = serde_json::to_string_pretty(&saved) {
                let _ = std::fs::write(path, json);
            }
        }
    }

    /// True while a move/resize/reorder drag is in flight.
    pub fn dragging(&self) -> bool {
        self.drag.is_some() || self.tile_drag.is_some()
    }

    /// Draw the workspace into `area`: every visible panel with its chrome
    /// (header, traffic lights, resize grip) plus the dock line at the
    /// bottom. `body` draws one panel's content into the given inner rect;
    /// its last argument is whether that panel is maximized.
    pub fn render(
        &mut self,
        f: &mut Frame,
        area: Rect,
        body: &mut dyn FnMut(&mut Frame, Rect, K, bool),
    ) {
        self.zones.clear();
        self.dock_chips.clear();
        if area.height < 5 || area.width < 8 {
            let t = self.theme;
            f.render_widget(
                Paragraph::new("resize")
                    .style(Style::default().fg(t.dim))
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .border_set(ASCII_BORDER)
                            .border_style(Style::default().fg(t.line2)),
                    ),
                area,
            );
            return;
        }
        let t = self.theme;
        let chrome = workspace_chrome(area.width as f64, area.height as f64, &ChromeMetrics::CELLS);
        let root = area;
        let dock = rect_from_region(chrome.dock);
        let ws = rect_from_region(chrome.workspace);
        self.ws_area = ws;

        let root_block = Block::default()
            .borders(Borders::ALL)
            .border_set(ASCII_BORDER)
            .border_style(Style::default().fg(t.line2));
        f.render_widget(root_block, root);

        let dock_block = Block::default()
            .borders(Borders::ALL)
            .border_set(ASCII_BORDER)
            .border_style(Style::default().fg(t.line2));
        let dock_inner = dock_block.inner(dock);
        f.render_widget(dock_block, dock);

        let (visible, maximized) = visible_panels(&self.panels);

        // Floating panels draw back-to-front by z so overlap works.
        let mut order = visible;
        if maximized.is_none() && self.mode == Mode::Floating {
            order.sort_by_key(|&i| self.panels[i].z);
        }

        // Tiling: flow panels into rows of TILE_W_MAX quarter-units.
        let mut tile_rects: Vec<(usize, Rect)> = Vec::new();
        if maximized.is_none() && self.mode == Mode::Tiling {
            let col = (ws.width as f64 / TILE_W_MAX as f64).floor().max(1.0);
            let (mut used, mut row_y, mut row_h) = (0u8, ws.y, 0u16);
            for &i in &order {
                let p = &self.panels[i];
                let (tw, th) = (p.tile_w.min(TILE_W_MAX), p.tile_h);
                if used + tw > TILE_W_MAX {
                    row_y += row_h;
                    used = 0;
                    row_h = 0;
                }
                let h = ((th as f64 * ROW_CELLS) as u16).max(3);
                let x = ws.x + (used as f64 * col) as u16;
                let w = if used + tw == TILE_W_MAX {
                    ws.right().saturating_sub(x)
                } else {
                    (tw as f64 * col) as u16
                };
                if row_y < ws.bottom() {
                    let h = h.min(ws.bottom() - row_y);
                    tile_rects.push((i, Rect::new(x, row_y, w.max(3), h)));
                }
                used += tw;
                row_h = row_h.max(h);
            }
        }

        for &i in &order {
            let p = self.panels[i];
            let rect = if maximized.is_some() {
                ws
            } else if self.mode == Mode::Tiling {
                match tile_rects.iter().find(|(ti, _)| *ti == i) {
                    Some((_, r)) => *r,
                    None => continue,
                }
            } else {
                let (x, y, w, h) =
                    effective_rect(&p, ws.width as f64, ws.height as f64, &Clamp::CELLS);
                Rect::new(
                    ws.x + x as u16,
                    ws.y + y as u16,
                    (w as u16).min(ws.width),
                    (h as u16).min(ws.height),
                )
            };
            if rect.width < 3 || rect.height < 2 {
                continue;
            }

            f.render_widget(Clear, rect);
            let t = self.theme;
            let focused = maximized == Some(i)
                || (self.mode == Mode::Floating && p.z == front_z(&self.panels) - 1);
            let border_style = if self.tile_drag == Some(p.kind) {
                Style::default().fg(t.yellow)
            } else if focused {
                Style::default().fg(t.dim)
            } else {
                Style::default().fg(t.line2)
            };
            // Traffic lights sit top-LEFT like the web shell, in its
            // printer-CMY colors: blue mode, yellow minimize, pink
            // maximize. ASCII keeps the browser WebGL backend from dropping
            // non-atlas glyphs while preserving the same hit zones.
            let ly = rect.y;
            let lx = rect.x + 2;
            let light_cells = [
                Rect::new(lx, ly, 1, 1),
                Rect::new(lx + 2, ly, 1, 1),
                Rect::new(lx + 4, ly, 1, 1),
            ];
            let hovered_light = self
                .hover
                .and_then(|h| light_cells.iter().position(|c| c.contains(h)));
            let mut block = Block::default()
                .borders(Borders::ALL)
                .border_set(ASCII_BORDER)
                .border_style(border_style);
            let hovered_hint = hovered_light.map(|slot| match slot {
                0 => {
                    if self.mode == Mode::Tiling {
                        "float"
                    } else {
                        "tile"
                    }
                }
                1 => "minimize",
                _ => {
                    if p.state == WinState::Maximized {
                        "restore"
                    } else {
                        "maximize"
                    }
                }
            });
            if hovered_light.is_some() {
                let hint = hovered_hint.expect("hovered_light is Some");
                block = block.title(
                    Line::from(Span::styled(
                        format!(" {hint} "),
                        Style::default().fg(t.dim),
                    ))
                    .right_aligned(),
                );
            }
            let inner = block.inner(rect);
            f.render_widget(block, rect);
            for (slot, cell) in light_cells.iter().enumerate() {
                let color = match slot {
                    0 => t.blue,
                    1 => t.yellow,
                    _ => t.pink,
                };
                let ch = if hovered_light == Some(slot) {
                    'O'
                } else {
                    'o'
                };
                if cell.x < rect.right() && cell.y < rect.bottom() {
                    f.buffer_mut()[(cell.x, cell.y)]
                        .set_char(ch)
                        .set_style(Style::default().fg(color));
                }
            }
            let title_x = rect.x.saturating_add(9);
            let title_w = rect.right().saturating_sub(title_x.saturating_add(1));
            write_header_text(
                f,
                title_x,
                rect.y,
                title_w,
                p.kind.title(),
                Style::default().fg(t.fg),
            );
            if let Some(hint) = hovered_hint {
                let hint = format!(" {hint} ");
                let hint_w = hint.chars().count() as u16;
                let hint_x = rect.right().saturating_sub(hint_w.saturating_add(1));
                if hint_x > title_x {
                    write_header_text(f, hint_x, rect.y, hint_w, &hint, Style::default().fg(t.dim));
                }
            }
            body(f, inner, p.kind, maximized == Some(i));
            // Resize grip: the bottom-right corner itself, tinted accent
            // under the pointer.
            let grip = Rect::new(
                rect.right().saturating_sub(2),
                rect.bottom().saturating_sub(1),
                2,
                1,
            );
            if self.hover.map(|h| grip.contains(h)).unwrap_or(false) {
                f.render_widget(
                    Paragraph::new("+").style(Style::default().fg(t.accent)),
                    Rect::new(rect.right().saturating_sub(1), grip.y, 1, 1),
                );
            }

            self.zones.push(Zone {
                idx: i,
                panel: rect,
                header: Rect::new(rect.x, rect.y, rect.width, 1),
                lights: light_cells,
                grip,
            });
        }

        // Dock line.
        let t = self.theme;
        let mut spans = vec![Span::styled("dock:", Style::default().fg(t.dim))];
        let mut x = dock_inner.x + 5;
        let minimized: Vec<usize> = self
            .panels
            .iter()
            .enumerate()
            .filter(|(_, p)| p.state == WinState::Minimized)
            .map(|(i, _)| i)
            .collect();
        if minimized.is_empty() {
            spans.push(Span::styled(
                " — nothing minimized —",
                Style::default().fg(t.dim),
            ));
        }
        for i in minimized {
            let label = format!(" [{}]", self.panels[i].kind.title());
            let w = label.chars().count() as u16;
            self.dock_chips.push((Rect::new(x, dock_inner.y, w, 1), i));
            spans.push(Span::styled(label, Style::default().fg(t.accent)));
            x += w;
        }
        f.render_widget(Paragraph::new(Line::from(spans)), dock_inner);
    }

    /// Feed a mouse event in terminal-cell coordinates: clicks hit traffic lights, dock chips,
    /// headers (move/reorder), the grip (resize), and panel bodies (raise);
    /// drags apply through the shared core math; release settles + saves.
    pub fn handle_mouse(&mut self, m: TuiMouseEvent) {
        let at = Position::new(m.x as u16, m.y as u16);
        let (mx, my) = (m.x, m.y);
        match m.kind {
            TuiMouseEventKind::Down(TuiMouseButton::Primary) => {
                for (rect, i) in &self.dock_chips {
                    if rect.contains(at) {
                        let kind = self.panels[*i].kind;
                        restore(&mut self.panels, kind);
                        self.save();
                        return;
                    }
                }
                // Topmost panel first (zones are drawn back-to-front).
                let zone = self.zones.iter().rev().find(|z| z.panel.contains(at));
                let Some(z) = zone else { return };
                let i = z.idx;
                if z.lights[0].contains(at) {
                    self.mode = if self.mode == Mode::Tiling {
                        Mode::Floating
                    } else {
                        Mode::Tiling
                    };
                    self.save();
                } else if z.lights[1].contains(at) {
                    self.panels[i].state = WinState::Minimized;
                    self.save();
                } else if z.lights[2].contains(at) {
                    let p = &mut self.panels[i];
                    p.state = if p.state == WinState::Maximized {
                        WinState::Floating
                    } else {
                        WinState::Maximized
                    };
                    self.save();
                } else if z.grip.contains(at) {
                    self.drag = if self.mode == Mode::Tiling {
                        begin_tile_resize(&self.panels, i, mx, my)
                    } else {
                        let (vw, vh) = (self.ws_area.width as f64, self.ws_area.height as f64);
                        begin_drag(
                            &mut self.panels,
                            i,
                            DragKind::Resize,
                            mx,
                            my,
                            vw,
                            vh,
                            &Clamp::CELLS,
                        )
                    };
                } else if z.header.contains(at) {
                    if self.mode == Mode::Tiling {
                        self.tile_drag = Some(self.panels[i].kind);
                    } else {
                        let (vw, vh) = (self.ws_area.width as f64, self.ws_area.height as f64);
                        self.drag = begin_drag(
                            &mut self.panels,
                            i,
                            DragKind::Move,
                            mx,
                            my,
                            vw,
                            vh,
                            &Clamp::CELLS,
                        );
                        let z = front_z(&self.panels);
                        self.panels[i].z = z;
                    }
                } else if self.mode == Mode::Floating {
                    let z = front_z(&self.panels);
                    self.panels[i].z = z;
                }
            }
            TuiMouseEventKind::Drag(TuiMouseButton::Primary) => {
                if let Some(d) = self.drag {
                    let tiling = self.mode == Mode::Tiling;
                    let vw = self.ws_area.width as f64;
                    apply_drag(
                        &mut self.panels,
                        &d,
                        mx,
                        my,
                        tiling,
                        vw,
                        &Clamp::CELLS,
                        &TileMetrics::CELLS,
                    );
                } else if let Some(dragged) = self.tile_drag {
                    if let Some(z) = self.zones.iter().rev().find(|z| z.panel.contains(at)) {
                        let target = self.panels[z.idx].kind;
                        if target != dragged {
                            reorder_tile(&mut self.panels, dragged, target);
                        }
                    }
                }
            }
            TuiMouseEventKind::Up(TuiMouseButton::Primary) => {
                if self.dragging() {
                    self.drag = None;
                    self.tile_drag = None;
                    self.save();
                }
            }
            TuiMouseEventKind::Moved => {
                self.hover = Some(at);
            }
        }
    }

    /// Restore and raise the panel of `kind` — hook for key bindings.
    pub fn restore_panel(&mut self, kind: K) {
        restore(&mut self.panels, kind);
        self.save();
    }
}
