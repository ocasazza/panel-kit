use panel_kit_core::widgets::charts::FlameSpanModel;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::Frame;

use panel_kit_tui::ResolvedTuiTheme;

use super::{mix, rgb_color, series_colors};

pub fn flame(f: &mut Frame, area: Rect, t: &ResolvedTuiTheme, spans: &[FlameSpanModel]) {
    if area.width == 0 || area.height == 0 || spans.is_empty() {
        return;
    }
    let colors = series_colors(t);
    struct Open {
        depth: u16,
        x0: f64,
        x1: f64,
        cursor: f64,
        siblings_total: f64,
    }
    let mut child_total = vec![0.0f64; spans.len()];
    let mut idx_stack: Vec<usize> = Vec::new();
    for (i, s) in spans.iter().enumerate() {
        while let Some(&p) = idx_stack.last() {
            if spans[p].depth < s.depth {
                break;
            }
            idx_stack.pop();
        }
        if let Some(&p) = idx_stack.last() {
            child_total[p] += s.value.max(0.0);
        }
        idx_stack.push(i);
    }
    let root_total: f64 = spans
        .iter()
        .filter(|s| s.depth == 0)
        .map(|s| s.value.max(0.0))
        .sum::<f64>()
        .max(f64::MIN_POSITIVE);

    let w = area.width as f64;
    let mut open: Vec<Open> = Vec::new();
    for (i, s) in spans.iter().enumerate() {
        while let Some(o) = open.last() {
            if o.depth < s.depth {
                break;
            }
            open.pop();
        }
        let (px0, px1, ptotal) = match open.last() {
            Some(p) => (p.cursor, p.x1, p.siblings_total),
            None => (0.0, w, root_total),
        };
        let share = if ptotal > 0.0 {
            s.value.max(0.0) / ptotal
        } else {
            0.0
        };
        let parent_span = match open.last() {
            Some(p) => p.x1 - p.x0,
            None => w,
        };
        let x0 = px0;
        let x1 = (x0 + share * parent_span).min(px1);
        if let Some(p) = open.last_mut() {
            p.cursor = x1;
        }
        open.push(Open {
            depth: s.depth,
            x0,
            x1,
            cursor: x0,
            siblings_total: child_total[i],
        });

        let row = area.y + s.depth;
        if row >= area.bottom() {
            continue;
        }
        let cell_x0 = area.x + x0.floor() as u16;
        let cell_w = (x1.floor() - x0.floor()).max(0.0) as u16;
        if cell_w == 0 {
            continue;
        }
        let cell_w = cell_w.min(area.right().saturating_sub(cell_x0));
        let color = s.color.map(rgb_color).unwrap_or_else(|| {
            let base = colors[s.depth as usize % colors.len()];
            mix(base, t.bg, (s.depth as f64 * 0.08).min(0.45))
        });
        let style = Style::default().bg(color);
        for cx in cell_x0..cell_x0 + cell_w {
            f.buffer_mut()[(cx, row)].set_char(' ').set_style(style);
        }
        let ink = contrast_ink(color, t);
        for (off, ch) in s.label.chars().take(cell_w as usize).enumerate() {
            f.buffer_mut()[(cell_x0 + off as u16, row)]
                .set_char(ch)
                .set_style(Style::default().fg(ink).bg(color));
        }
    }
}

fn contrast_ink(bg: Color, t: &ResolvedTuiTheme) -> Color {
    match bg {
        Color::Rgb(r, g, b) => {
            let luma = 0.299 * r as f64 + 0.587 * g as f64 + 0.114 * b as f64;
            if luma > 140.0 {
                t.bg
            } else {
                t.fg
            }
        }
        _ => t.fg,
    }
}
