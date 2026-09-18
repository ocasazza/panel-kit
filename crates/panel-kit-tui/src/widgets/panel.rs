//! Panel surface, chrome, controls, and cell-grid conversion parts.

use panel_kit_core::frame::{PanelProjection, Placement, TileGridProjection};
use panel_kit_core::reducer::PanelPart;
use panel_kit_core::{Mode, PanelKey, PanelMeta, Region, WinState};
use ratatui::layout::{Position, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::widgets::TuiHitBuffer;
use crate::{rect_from_region, Charset, ResolvedTuiTheme};

/// Draw only a panel's surface/body and return the projected body rectangle.
///
/// This part intentionally draws no title, traffic lights, resize grip, or dock.
/// Hosts opt into those parts with separate calls.
pub fn draw_panel_surface<K: PanelKey>(
    frame: &mut Frame,
    panel: PanelProjection<K>,
    theme: &ResolvedTuiTheme,
    _charset: Charset,
    hits: &mut TuiHitBuffer<K>,
) -> Rect {
    let outer = rect_from_region(panel.chrome.outer);
    let body = rect_from_region(panel.chrome.body);

    frame.render_widget(Clear, outer);
    frame
        .buffer_mut()
        .set_style(body, Style::default().bg(theme.panel));
    hits.record_panel_surface(panel.key, panel.source_index, outer, body);

    body
}

/// Draw the native ratatui block chrome/title and return `Block::inner`.
///
/// The returned body is the rect applications should use for native widgets.
/// Keeping the body from `Block::inner` preserves ratatui layout semantics
/// instead of replaying a renderer-neutral command tape.
pub fn draw_panel_chrome<K: PanelKey>(
    frame: &mut Frame,
    panel: PanelProjection<K>,
    meta: &PanelMeta<K>,
    theme: &ResolvedTuiTheme,
    charset: Charset,
    hits: &mut TuiHitBuffer<K>,
) -> Rect {
    let outer = rect_from_region(panel.chrome.outer);
    let header = rect_from_region(panel.chrome.header_hit);
    if !has_frame(panel) {
        hits.record_panel_header(panel.key, panel.source_index, header);
        return rect_from_region(panel.chrome.body);
    }

    let title_text = if panel.chrome.mode_hit.is_some() || panel.chrome.minimize_hit.is_some() {
        format!("        {}", meta.title.as_ref())
    } else {
        meta.title.as_ref().to_string()
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_set(charset.border())
        .border_style(panel_border_style(panel, theme))
        .title(Line::from(Span::styled(
            title_text,
            Style::default().fg(theme.fg),
        )));
    let inner = block.inner(outer);
    frame.render_widget(block, outer);
    hits.record_panel_header(panel.key, panel.source_index, header);

    inner
}

/// Draw the panel traffic lights and record their hit zones.
///
/// `mode` is accepted by the part so hosts can keep one call shape for future
/// hover-hint rendering without moving mode policy into the core projection.
pub fn draw_traffic_lights<K: PanelKey>(
    frame: &mut Frame,
    panel: PanelProjection<K>,
    _mode: Mode,
    hover: Option<Position>,
    theme: &ResolvedTuiTheme,
    charset: Charset,
    hits: &mut TuiHitBuffer<K>,
) {
    for (part, region, color) in [
        (PanelPart::ModeControl, panel.chrome.mode_hit, theme.blue),
        (
            PanelPart::MinimizeControl,
            panel.chrome.minimize_hit,
            theme.yellow,
        ),
        (
            PanelPart::MaximizeControl,
            panel.chrome.maximize_hit,
            theme.pink,
        ),
    ] {
        draw_light(
            frame,
            TrafficLight {
                panel,
                part,
                region,
                hover,
                color,
                charset,
            },
            hits,
        );
    }
}

/// Draw the bottom-right resize grip and record its hit zone.
pub fn draw_resize_grip<K: PanelKey>(
    frame: &mut Frame,
    panel: PanelProjection<K>,
    hover: Option<Position>,
    theme: &ResolvedTuiTheme,
    hits: &mut TuiHitBuffer<K>,
) -> Option<Rect> {
    let region = panel.chrome.resize_hit.map(rect_from_region)?;
    hits.record_panel_part(panel.key, panel.source_index, PanelPart::ResizeGrip, region);

    if hover.is_some_and(|point| region.contains(point)) {
        let glyph = Rect::new(region.right().saturating_sub(1), region.y, 1, 1);
        frame.render_widget(
            Paragraph::new("+").style(Style::default().fg(theme.accent)),
            glyph,
        );
    }

    Some(region)
}

/// Convert a shared [`TileGridProjection`] placement into terminal cells.
///
/// The calculation mirrors core/web grid inputs: track width/height, gap,
/// padding, span, and scroll are the only geometry inputs.
pub fn tiled_cell_rect(
    placement: Placement,
    grid: TileGridProjection,
    workspace: Rect,
    scroll: f64,
) -> Option<Rect> {
    let Placement::Tiled {
        column,
        row,
        column_span,
        row_span,
    } = placement
    else {
        return None;
    };

    let x = workspace.x as f64 + grid.padding + column as f64 * (grid.track_w + grid.gap);
    let y = workspace.y as f64 + grid.padding + row as f64 * (grid.track_h + grid.gap) - scroll;
    let w = column_span as f64 * grid.track_w + column_span.saturating_sub(1) as f64 * grid.gap;
    let h = row_span as f64 * grid.track_h + row_span.saturating_sub(1) as f64 * grid.gap;

    Some(Rect::new(
        x.max(0.0) as u16,
        y.max(0.0) as u16,
        w.max(0.0) as u16,
        h.max(0.0) as u16,
    ))
}

/// Draw command for one panel traffic-light control.
struct TrafficLight<K: PanelKey> {
    panel: PanelProjection<K>,
    part: PanelPart,
    region: Option<Region>,
    hover: Option<Position>,
    color: ratatui::style::Color,
    charset: Charset,
}

/// Record and render one panel traffic-light control.
fn draw_light<K: PanelKey>(frame: &mut Frame, light: TrafficLight<K>, hits: &mut TuiHitBuffer<K>) {
    let Some(region) = light.region.map(rect_from_region) else {
        return;
    };

    hits.record_panel_part(
        light.panel.key,
        light.panel.source_index,
        light.part,
        region,
    );
    if region.width == 0 || region.height == 0 {
        return;
    }

    let hovered = light.hover.is_some_and(|point| region.contains(point));
    frame.buffer_mut()[(region.x, region.y)]
        .set_char(light.charset.light(hovered))
        .set_style(Style::default().fg(light.color));
}

fn has_frame<K: PanelKey>(panel: PanelProjection<K>) -> bool {
    panel.chrome.outer != panel.chrome.body || panel.chrome.header_hit.h > 0.0
}

fn panel_border_style<K: PanelKey>(panel: PanelProjection<K>, theme: &ResolvedTuiTheme) -> Style {
    if panel.tile_dragging {
        return Style::default().fg(theme.accent);
    }
    if panel.focused {
        return Style::default().fg(theme.focus_ring);
    }
    if panel.state == WinState::Maximized {
        return Style::default().fg(theme.fg);
    }
    Style::default().fg(theme.line2)
}
