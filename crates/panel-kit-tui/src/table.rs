//! Themed semantic table painter for the shared core table model.
//!
//! The renderer-neutral column/cell data lives in
//! [`panel_kit_core::widgets::table`]. This module keeps the ratatui boundary:
//! it converts borrowed core cells into native [`Table`] rows without requiring
//! callers to build ratatui data structures unless they explicitly opt into
//! [`table_native`].

use panel_kit_core::widgets::meter as core_meter;
use panel_kit_core::widgets::table::{ColumnWidth, TableCell, TableView, TextAlign};
use ratatui::layout::{Constraint, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, Row, Table};
use ratatui::Frame;

use crate::ResolvedTuiTheme;

/// Render a themed semantic table from a borrowed core [`TableView`].
pub fn table(f: &mut Frame, area: Rect, t: &ResolvedTuiTheme, view: TableView<'_>) {
    let widths = view.columns.iter().map(|column| width_constraint(column.width));
    let header = Row::new(view.columns.iter().map(|column| {
        Cell::from(align_line(Line::from(column.title.as_str()), column.align))
    }))
    .style(Style::default().fg(t.dim));
    let rows = view.rows.iter().map(|row| {
        Row::new(row.cells.iter().enumerate().map(|(index, cell)| {
            let align = view
                .columns
                .get(index)
                .map(|column| column.align)
                .unwrap_or(TextAlign::Left);
            Cell::from(align_line(semantic_line(cell, t), align))
        }))
    });

    f.render_widget(Table::new(rows, widths).header(header), area);
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

fn semantic_line<'a>(cell: &'a TableCell, t: &ResolvedTuiTheme) -> Line<'a> {
    match cell {
        TableCell::Text(text) => Line::from(text.as_str()),
        TableCell::Status { label, color } => {
            crate::status::labeled_borrowed(*color, label)
        }
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

