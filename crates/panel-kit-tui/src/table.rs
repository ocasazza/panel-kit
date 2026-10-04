//! Themed semantic table painter for the shared core table model.
//!
//! The renderer-neutral column/cell data lives in
//! [`panel_kit_core::widgets::table`]. This module keeps the ratatui boundary:
//! it converts borrowed core cells into native [`Table`] rows without requiring
//! callers to build ratatui data structures unless they explicitly opt into
//! [`table_native`].
//!
//! [`table`] scrolls through the host's [`PanelScroll`] registry: it keeps the
//! header row sticky, offsets the body rows vertically, drops leading columns
//! horizontally, and records the measured extent so wheel routing can consume
//! the gesture.

use panel_kit_core::widgets::meter as core_meter;
use panel_kit_core::widgets::table::{ColumnWidth, TableCell, TableColumn, TableView, TextAlign};
use panel_kit_core::PanelKey;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, Row, Table};
use ratatui::Frame;

use crate::scroll::PanelScroll;
use crate::ResolvedTuiTheme;

/// Rows a themed table header occupies; the header stays sticky while the body
/// rows scroll under it.
const HEADER_H: u16 = 1;

/// Ratatui's default horizontal gap between table columns.
const COLUMN_SPACING: u16 = 1;

/// Render a themed semantic table at the offsets registered for `key` and
/// return the `(x, y)` offsets drawn with.
///
/// Vertical offset skips body rows; horizontal offset drops whole leading
/// columns. Content that fits its body scrolls not at all.
pub fn table<K: PanelKey>(
    f: &mut Frame,
    area: Rect,
    t: &ResolvedTuiTheme,
    key: &K,
    panels: &mut PanelScroll<K>,
    view: TableView<'_>,
) -> (u16, u16) {
    let area = area.intersection(f.area());
    if area.width == 0 || area.height == 0 {
        return panels.offset(key);
    }

    let content_h = view.rows.len().min(u16::MAX as usize) as u16;
    let body_h = area.height.saturating_sub(HEADER_H);
    let overflows = content_h > body_h;
    // Reserve the right column for the bar so it never paints over cells.
    let body = match overflows {
        true => Rect {
            width: area.width.saturating_sub(1),
            ..area
        },
        false => area,
    };

    let offset = panels.set_extent(
        key,
        content_width(view.columns),
        content_h,
        body.width,
        body.height.saturating_sub(HEADER_H),
    );
    let columns: &[TableColumn] = &view.columns[offset.0.min(view.columns.len() as u16) as usize..];

    let widths = columns.iter().map(|column| width_constraint(column.width));
    let header = Row::new(
        columns
            .iter()
            .map(|column| Cell::from(align_line(Line::from(column.title.as_str()), column.align))),
    )
    .style(Style::default().fg(t.dim));
    let rows = view.rows.iter().skip(offset.1 as usize).map(|row| {
        Row::new(
            row.cells
                .iter()
                .skip(offset.0 as usize)
                .zip(columns)
                .map(|(cell, column)| Cell::from(align_line(semantic_line(cell, t), column.align))),
        )
    });

    f.render_widget(Table::new(rows, widths).header(header), body);

    if overflows {
        crate::scroll::vertical_scrollbar(f, area, t, content_h, offset.1);
    }

    offset
}

/// Render caller-built ratatui rows with panel-kit header styling.
///
/// This is the native escape hatch for terminal-specific cells. Authored
/// workspace specs and portable providers should use [`table`] instead.
pub fn table_native(
    f: &mut Frame,
    area: Rect,
    t: &ResolvedTuiTheme,
    header: &[&str],
    widths: &[Constraint],
    rows: Vec<Row<'_>>,
) {
    let header = Row::new(header.iter().copied()).style(Style::default().fg(t.dim));
    f.render_widget(
        Table::new(rows, widths.iter().copied()).header(header),
        area,
    );
}

/// Layed-out table width from the authored column widths.
///
/// Flex columns are zero here: ratatui only grants them the space fixed columns
/// leave over, so a table with slack has nothing to scroll toward.
fn content_width(columns: &[TableColumn]) -> u16 {
    let fixed: u32 = columns
        .iter()
        .map(|column| match column.width {
            ColumnWidth::Fixed { value } => u32::from(value),
            ColumnWidth::Flex { .. } => 0,
        })
        .sum();
    let spacing = u32::from(COLUMN_SPACING).saturating_mul(columns.len().saturating_sub(1) as u32);
    u16::try_from(fixed.saturating_add(spacing)).unwrap_or(u16::MAX)
}

fn semantic_line<'a>(cell: &'a TableCell, t: &ResolvedTuiTheme) -> Line<'a> {
    match cell {
        TableCell::Text(text) => Line::from(text.as_str()),
        TableCell::Status { label, color } => crate::status::labeled_borrowed(*color, label),
        TableCell::Meter { ratio, text, color } => {
            let span = match color {
                Some(color) => crate::meter::span(*ratio, 8, *color),
                None => Span::styled(core_meter::bar(*ratio, 8), Style::default().fg(t.fg)),
            };

            if text.is_empty() {
                return Line::from(span);
            }

            Line::from(vec![span, Span::raw(" "), Span::raw(text.as_str())])
        }
    }
}

fn align_line(line: Line<'_>, align: TextAlign) -> Line<'_> {
    match align {
        TextAlign::Left => line,
        TextAlign::Center => line.centered(),
        TextAlign::Right => line.right_aligned(),
    }
}

fn width_constraint(width: ColumnWidth) -> Constraint {
    match width {
        ColumnWidth::Fixed { value } => Constraint::Length(value),
        ColumnWidth::Flex { weight } => Constraint::Fill(weight),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn columns() -> Vec<TableColumn> {
        vec![
            TableColumn {
                key: "a".into(),
                title: "a".into(),
                width: ColumnWidth::Fixed { value: 12 },
                align: TextAlign::Left,
            },
            TableColumn {
                key: "b".into(),
                title: "b".into(),
                width: ColumnWidth::Fixed { value: 10 },
                align: TextAlign::Left,
            },
            TableColumn {
                key: "c".into(),
                title: "c".into(),
                width: ColumnWidth::Flex { weight: 1 },
                align: TextAlign::Left,
            },
        ]
    }

    #[test]
    fn content_width_counts_fixed_columns_and_their_spacing() {
        assert_eq!(content_width(&columns()), 24);
        assert_eq!(content_width(&[]), 0);
        assert_eq!(content_width(&columns()[..1]), 12);
    }
}
