use panel_kit_core::widgets::charts::{BoxItemView, FiveNum};
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::Frame;

use crate::ResolvedTuiTheme;

use super::boxplot::{BoxGeometry, BoxScale};

#[derive(Clone, Copy)]
struct SummaryRows {
    min: u16,
    q1: u16,
    median: u16,
    q3: u16,
    max: u16,
}

pub(super) fn draw_item(
    f: &mut Frame,
    area: Rect,
    scale: BoxScale,
    geometry: BoxGeometry,
    item: &BoxItemView<'_>,
    color: Color,
    t: &ResolvedTuiTheme,
) {
    let rows = summary_rows(scale, item.summary);
    draw_whisker(
        f,
        area,
        scale.plot_bottom,
        geometry.center_x,
        rows.min,
        rows.max,
        t,
    );
    draw_caps(f, area, scale.plot_bottom, geometry, rows.min, rows.max, t);
    draw_iqr(
        f,
        area,
        scale.plot_bottom,
        geometry,
        rows.q1,
        rows.q3,
        color,
    );
    draw_median(f, area, scale.plot_bottom, geometry, rows.median, color, t);
    draw_label(f, area, scale.plot_bottom, geometry, item.label, t);
}

fn summary_rows(scale: BoxScale, summary: FiveNum) -> SummaryRows {
    SummaryRows {
        min: scale.row_of(summary.min),
        q1: scale.row_of(summary.q1),
        median: scale.row_of(summary.median),
        q3: scale.row_of(summary.q3),
        max: scale.row_of(summary.max),
    }
}

fn draw_whisker(
    f: &mut Frame,
    area: Rect,
    plot_bottom: u16,
    center_x: u16,
    min_row: u16,
    max_row: u16,
    t: &ResolvedTuiTheme,
) {
    let dim = Style::default().fg(t.line2);
    let frame_area = f.area();
    for y in max_row..=min_row {
        if y >= plot_bottom || y >= frame_area.bottom() {
            break;
        }
        let x = center_x
            .min(area.right().saturating_sub(1))
            .min(frame_area.right().saturating_sub(1));
        f.buffer_mut()[(x, y)]
            .set_char('│')
            .set_style(dim);
    }
}

fn draw_caps(
    f: &mut Frame,
    area: Rect,
    plot_bottom: u16,
    geometry: BoxGeometry,
    min_row: u16,
    max_row: u16,
    t: &ResolvedTuiTheme,
) {
    let dim = Style::default().fg(t.line2);
    let frame_area = f.area();
    for cap in [max_row, min_row] {
        if cap >= frame_area.bottom() {
            continue;
        }
        for offset in 0..geometry.box_w {
            let x = geometry.box_x + offset;
            if x < area.right() && x < frame_area.right() && cap < plot_bottom {
                f.buffer_mut()[(x, cap)].set_char('─').set_style(dim);
            }
        }
    }
}

fn draw_iqr(
    f: &mut Frame,
    area: Rect,
    plot_bottom: u16,
    geometry: BoxGeometry,
    q1_row: u16,
    q3_row: u16,
    color: Color,
) {
    let fill = Style::default().bg(color);
    let frame_area = f.area();
    for y in q3_row..=q1_row {
        if y >= plot_bottom || y >= frame_area.bottom() {
            break;
        }
        for offset in 0..geometry.box_w {
            let x = geometry.box_x + offset;
            if x < area.right() && x < frame_area.right() {
                f.buffer_mut()[(x, y)].set_char(' ').set_style(fill);
            }
        }
    }
}

fn draw_median(
    f: &mut Frame,
    area: Rect,
    plot_bottom: u16,
    geometry: BoxGeometry,
    median_row: u16,
    color: Color,
    t: &ResolvedTuiTheme,
) {
    let frame_area = f.area();
    if median_row >= plot_bottom || median_row >= frame_area.bottom() {
        return;
    }

    let median = Style::default().fg(t.fg).bg(color);
    for offset in 0..geometry.box_w {
        let x = geometry.box_x + offset;
        if x < area.right() && x < frame_area.right() {
            f.buffer_mut()[(x, median_row)]
                .set_char('━')
                .set_style(median);
        }
    }
}

fn draw_label(
    f: &mut Frame,
    area: Rect,
    plot_bottom: u16,
    geometry: BoxGeometry,
    label: &str,
    t: &ResolvedTuiTheme,
) {
    let label_width = label.chars().take(geometry.slot_w as usize).count() as u16;
    let label_x = geometry.slot_x + geometry.slot_w.saturating_sub(label_width) / 2;

    let frame_area = f.area();
    for (offset, ch) in label.chars().take(geometry.slot_w as usize).enumerate() {
        let x = label_x + offset as u16;
        if x < area.right() && x < frame_area.right() && plot_bottom < area.bottom() && plot_bottom < frame_area.bottom() {
            f.buffer_mut()[(x, plot_bottom)]
                .set_char(ch)
                .set_style(Style::default().fg(t.dim));
        }
    }
}
