use panel_kit_core::widgets::charts::GaugeModel;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::Span;
use ratatui::widgets::Gauge;
use ratatui::Frame;

use crate::ResolvedTuiTheme;

/// Stack horizontal bar gauges, one row each.
///
/// This paints the "current vs max" allocation idiom used for memory buffers,
/// queue depth, and disk capacity.
pub fn gauges(f: &mut Frame, area: Rect, t: &ResolvedTuiTheme, items: &[GaugeModel]) {
    let area = area.intersection(f.area());
    if area.width == 0 || area.height == 0 {
        return;
    }
    let label_w = gauge_label_width(area, items);

    for (row, item) in items.iter().enumerate() {
        if row as u16 >= area.height {
            break;
        }

        let y = area.y + row as u16;
        f.render_widget(
            ratatui::widgets::Paragraph::new(Span::styled(
                item.label.as_str(),
                Style::default().fg(t.fg),
            )),
            Rect::new(area.x, y, label_w, 1),
        );
        f.render_widget(gauge_widget(item, t), gauge_bar_area(area, y, label_w));
    }
}

fn gauge_label_width(area: Rect, items: &[GaugeModel]) -> u16 {
    items
        .iter()
        .map(|item| item.label.chars().count() as u16)
        .max()
        .unwrap_or(0)
        .min(area.width / 3)
}

fn gauge_bar_area(area: Rect, y: u16, label_w: u16) -> Rect {
    Rect::new(
        area.x + label_w + 1,
        y,
        area.width.saturating_sub(label_w + 1),
        1,
    )
}

fn gauge_widget<'a>(item: &'a GaugeModel, t: &ResolvedTuiTheme) -> Gauge<'a> {
    let ratio = item.ratio.clamp(0.0, 1.0);
    Gauge::default()
        .ratio(ratio)
        .label(Span::styled(item.text.as_str(), Style::default().fg(t.fg)))
        // Unfilled track = panel background so an empty gauge reads empty;
        // only the filled fraction shows the fill color.
        .gauge_style(Style::default().fg(gauge_fill(ratio, t)).bg(t.bg))
        .use_unicode(true)
}

fn gauge_fill(ratio: f64, t: &ResolvedTuiTheme) -> Color {
    if ratio > 0.92 {
        return t.red;
    }
    if ratio > 0.75 {
        return t.yellow;
    }
    t.green
}
