use panel_kit_core::widgets::charts::BoxItemView;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::Frame;

use panel_kit_tui::ResolvedTuiTheme;

use super::{rgb_color, series_colors};

pub fn boxplot(f: &mut Frame, area: Rect, t: &ResolvedTuiTheme, items: &[BoxItemView<'_>]) {
    if area.width == 0 || area.height < 2 || items.is_empty() {
        return;
    }
    let colors = series_colors(t);
    let (mut lo, mut hi) = (f64::MAX, f64::MIN);
    for item in items {
        lo = lo.min(item.summary.min);
        hi = hi.max(item.summary.max);
    }
    if lo > hi {
        (lo, hi) = (0.0, 1.0);
    }
    if (hi - lo).abs() < f64::EPSILON {
        hi = lo + 1.0;
    }
    let span = hi - lo;
    let plot_h = area.height.saturating_sub(1).max(1);
    let plot_bottom = area.y + plot_h;
    let n = items.len() as u16;
    let slot_w = (area.width / n).max(1);
    let row_of = |val: f64| -> u16 {
        let frac = ((val - lo) / span).clamp(0.0, 1.0);
        let from_bottom = (frac * (plot_h.saturating_sub(1)) as f64).round() as u16;
        plot_bottom.saturating_sub(1).saturating_sub(from_bottom)
    };

    for (i, item) in items.iter().enumerate() {
        let s = item.summary;
        let color = item.color.map(rgb_color).unwrap_or(colors[i % colors.len()]);
        let slot_x = area.x + i as u16 * slot_w;
        let cx = slot_x + slot_w / 2;
        let box_w = (slot_w.saturating_sub(2)).clamp(1, 7);
        let box_x = cx.saturating_sub(box_w / 2);

        let (r_min, r_q1, r_med, r_q3, r_max) = (
            row_of(s.min),
            row_of(s.q1),
            row_of(s.median),
            row_of(s.q3),
            row_of(s.max),
        );
        let dim = Style::default().fg(t.line2);
        for y in r_max..=r_min {
            if y >= plot_bottom {
                break;
            }
            f.buffer_mut()[(cx.min(area.right().saturating_sub(1)), y)]
                .set_char('│')
                .set_style(dim);
        }
        for &(cap, w) in &[(r_max, box_w), (r_min, box_w)] {
            for off in 0..w {
                let x = box_x + off;
                if x < area.right() && cap < plot_bottom {
                    f.buffer_mut()[(x, cap)].set_char('─').set_style(dim);
                }
            }
        }
        let fill = Style::default().bg(color);
        for y in r_q3..=r_q1 {
            if y >= plot_bottom {
                break;
            }
            for off in 0..box_w {
                let x = box_x + off;
                if x < area.right() {
                    f.buffer_mut()[(x, y)].set_char(' ').set_style(fill);
                }
            }
        }
        if r_med < plot_bottom {
            let med = Style::default().fg(t.fg).bg(color);
            for off in 0..box_w {
                let x = box_x + off;
                if x < area.right() {
                    f.buffer_mut()[(x, r_med)].set_char('━').set_style(med);
                }
            }
        }
        let lw = item.label.chars().take(slot_w as usize).count() as u16;
        let lx = slot_x + slot_w.saturating_sub(lw) / 2;
        for (off, ch) in item.label.chars().take(slot_w as usize).enumerate() {
            let x = lx + off as u16;
            if x < area.right() && plot_bottom < area.bottom() {
                f.buffer_mut()[(x, plot_bottom)]
                    .set_char(ch)
                    .set_style(Style::default().fg(t.dim));
            }
        }
    }
}
