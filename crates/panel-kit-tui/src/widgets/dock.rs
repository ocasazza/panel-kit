//! Replaceable dock composition part for minimized panels.

use panel_kit_core::frame::DockProjection;
use panel_kit_core::{PanelCatalog, PanelKey};
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::Frame;

use crate::widgets::{draw_border, TuiHitBuffer};
use crate::{write_header_text, Charset, ResolvedTuiTheme};

/// Dependencies shared by dock rendering and hit recording.
pub struct DockRenderContext<'a, K: PanelKey> {
    /// Catalog used to resolve dock entry keys to borrowed display labels.
    pub catalog: &'a PanelCatalog<K>,
    /// Prefix drawn before minimized panel chips.
    pub label: &'a str,
    /// Resolved terminal colors for border, prefix, and chips.
    pub theme: &'a ResolvedTuiTheme,
    /// Border glyph set used for the dock frame.
    pub charset: Charset,
}

/// Draw a dock from borrowed projected dock entries.
///
/// The function is independently callable: a host can skip panel-kit panels and
/// render only this dock, or skip this function and paint application navigation
/// from the same `ProjectedFrame::dock` records.
pub fn draw_dock<K: PanelKey>(
    frame: &mut Frame,
    area: Rect,
    dock: &[DockProjection<K>],
    context: DockRenderContext<'_, K>,
    hits: &mut TuiHitBuffer<K>,
) {
    draw_border(
        frame,
        area,
        context.charset,
        Style::default().fg(context.theme.line2),
    );
    let inner = Rect::new(
        area.x.saturating_add(1),
        area.y.saturating_add(1),
        area.width.saturating_sub(2),
        area.height.saturating_sub(2),
    );

    let mut x = inner.x;
    write_text(
        frame,
        &mut x,
        inner,
        context.label,
        Style::default().fg(context.theme.dim),
    );
    if dock.is_empty() {
        write_text(
            frame,
            &mut x,
            inner,
            " -- nothing minimized --",
            Style::default().fg(context.theme.dim),
        );
        return;
    }

    for entry in dock.iter().copied() {
        let chip_start = x;
        write_dock_chip(
            frame,
            &mut x,
            inner,
            context.catalog,
            entry.key,
            context.theme,
        );
        let chip_width = x.saturating_sub(chip_start);
        hits.record_dock(
            entry.key,
            entry.source_index,
            Rect::new(chip_start, inner.y, chip_width, inner.height.max(1)),
        );
    }
}

fn write_dock_chip<K: PanelKey>(
    frame: &mut Frame,
    x: &mut u16,
    area: Rect,
    catalog: &PanelCatalog<K>,
    key: K,
    theme: &ResolvedTuiTheme,
) {
    let Some(meta) = catalog.get(key) else {
        return;
    };

    write_text(frame, x, area, " [", Style::default().fg(theme.fg));
    write_text(
        frame,
        x,
        area,
        meta.title.as_ref(),
        Style::default().fg(theme.fg),
    );
    write_text(frame, x, area, "]", Style::default().fg(theme.fg));
}

fn write_text(frame: &mut Frame, x: &mut u16, area: Rect, text: &str, style: Style) {
    if area.height == 0 || *x >= area.right() {
        return;
    }

    let max_width = area.right().saturating_sub(*x);
    write_header_text(frame, *x, area.y, max_width, text, style);
    *x = x
        .saturating_add(text.chars().count() as u16)
        .min(area.right());
}
