#[allow(dead_code)]
#[path = "../examples/workspace_canary.rs"]
mod workspace_canary;
#[path = "support/legacy_chart_painters.rs"]
mod legacy_chart_painters;


use panel_kit_core::badge::{display_label, BadgeAction, BadgeClickKind, BadgeKind, BadgeSpec};
use panel_kit_core::widgets::charts::{
    BoxItemView, FiveNum, FlameSpanModel, GaugeModel, SeriesView,
};
use panel_kit_core::widgets::meter::{self as core_meter, MeterModel};
use panel_kit_core::widgets::status::{StatusModel, StatusState};
use panel_kit_core::widgets::table::{
    ColumnWidth, TableCell, TableColumn, TableModel, TableRow, TableView, TextAlign,
};
use panel_kit_tui::{badge, charts, meter, status, table, ResolvedTuiTheme};
use ratatui::backend::TestBackend;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, Paragraph, Row};
use ratatui::Terminal;

#[test]
fn badge_actions_match_across_backends() {
    let mut spec = BadgeSpec::new("tag", "browser-tui", BadgeKind::Tag);
    spec.click_kind = BadgeClickKind::Clicked;
    spec.with_plus = true;
    spec.with_x = true;
    spec.emit_hover = true;

    assert_eq!(
        spec.primary_action(),
        BadgeAction::Clicked {
            field: "tag".into(),
            value: "browser-tui".into(),
        }
    );
    assert_eq!(
        spec.plus_action(),
        BadgeAction::AddFilter {
            field: "tag".into(),
            value: "browser-tui".into(),
        }
    );
    assert_eq!(
        spec.x_action(),
        BadgeAction::Toggle {
            field: "tag".into(),
            value: "browser-tui".into(),
        }
    );
    assert_eq!(
        spec.hover_action(),
        Some(BadgeAction::Hovered {
            field: "tag".into(),
            value: "browser-tui".into(),
        })
    );

    let theme = ResolvedTuiTheme::default();
    let spans = badge::spans(&spec, &theme);

    assert_eq!(badge::width(&spec), 13);
    assert_eq!(spans.len(), 3);
}

#[test]
fn canary_providers_return_core_widget_models() {
    let badges: Vec<BadgeSpec> = workspace_canary::demo_badges();
    let table: TableModel = workspace_canary::node_rows();
    let metrics = workspace_canary::Metrics::new();
    let series: [SeriesView<'_>; 2] = metrics.series();
    let boxes: Vec<BoxItemView<'_>> = metrics.boxes();
    let flame: Vec<FlameSpanModel> = metrics.flame();
    let gauges: [GaugeModel; 4] = workspace_canary::capacity_items();

    assert_eq!(badges[0].value, "browser-tui");
    assert_eq!(table.columns[0].title, "node");
    assert_eq!(series[0].name, "eval/ms");
    assert_eq!(boxes.len(), workspace_canary::STAGES.len());
    assert_eq!(flame[0].label, "session");
    assert_eq!(gauges[0].label, "vfs");
}

#[test]
fn canary_node_table_is_buffer_identical_after_core_model_switch() {
    let theme = ResolvedTuiTheme::default();
    let model = workspace_canary::node_rows();
    let area = Rect::new(0, 0, 38, 4);
    let view = TableView {
        columns: &model.columns,
        rows: &model.rows,
    };

    let core_buffer = render_to_buffer(|frame| table::table(frame, area, &theme, view));
    let native_rows = native_rows_from_core_table(&model);
    let native_widths = [
        Constraint::Length(12),
        Constraint::Length(10),
        Constraint::Length(10),
    ];
    let native_buffer = render_to_buffer(|frame| {
        table::table_native(
            frame,
            area,
            &theme,
            &["node", "load", ""],
            &native_widths,
            native_rows.clone(),
        )
    });

    assert_eq!(core_buffer, native_buffer);
}

#[test]
fn canary_provider_widgets_are_buffer_identical_after_core_model_switch() {
    let core_model_buffer = render_core_canary_provider_buffer();
    let legacy_buffer = render_legacy_canary_provider_buffer();

    assert_eq!(core_model_buffer, legacy_buffer);
}


#[test]
fn widget_painters_render_core_models_to_test_backend() {
    let theme = ResolvedTuiTheme::default();
    let mut terminal = Terminal::new(TestBackend::new(64, 16)).expect("test backend initializes");
    let columns = vec![
        TableColumn {
            key: "node".into(),
            title: "node".into(),
            width: ColumnWidth::Fixed { value: 12 },
            align: TextAlign::Left,
        },
        TableColumn {
            key: "load".into(),
            title: "load".into(),
            width: ColumnWidth::Fixed { value: 10 },
            align: TextAlign::Left,
        },
        TableColumn {
            key: "detail".into(),
            title: "detail".into(),
            width: ColumnWidth::Flex { weight: 1 },
            align: TextAlign::Left,
        },
    ];
    let rows = vec![TableRow {
        cells: vec![
            TableCell::Status {
                label: "pdx-01".into(),
                color: (39, 201, 63),
            },
            TableCell::Meter {
                ratio: 0.5,
                text: "50%".into(),
                color: Some((39, 201, 63)),
            },
            TableCell::Text("leader".into()),
        ],
    }];
    let view = TableView {
        columns: &columns,
        rows: &rows,
    };
    let status_model = StatusModel {
        label: "ready".into(),
        state: StatusState::Ok,
        color: (39, 201, 63),
    };
    let meter_model = MeterModel {
        label: "queue".into(),
        ratio: 0.5,
        text: "50%".into(),
        color: Some((39, 201, 63)),
    };
    let series = [SeriesView {
        name: "eval/ms",
        points: &[(0.0, 1.0), (1.0, 2.0)],
    }];
    let gauges = [GaugeModel {
        label: "queue".into(),
        ratio: 0.5,
        text: "50%".into(),
    }];
    let flame = [FlameSpanModel {
        label: "root".into(),
        depth: 0,
        value: 1.0,
        color: None,
    }];
    let boxes = [BoxItemView {
        label: "stage",
        summary: FiveNum {
            min: 1.0,
            q1: 2.0,
            median: 3.0,
            q3: 4.0,
            max: 5.0,
        },
        color: None,
    }];

    terminal
        .draw(|frame| {
            table::table(frame, Rect::new(0, 0, 38, 4), &theme, view);
            let status_line = status::line(&status_model);
            frame.render_widget(
                ratatui::widgets::Paragraph::new(status_line),
                Rect::new(40, 0, 20, 1),
            );
            let meter_span = meter::span_model(&meter_model, 8);
            frame.render_widget(
                ratatui::widgets::Paragraph::new(meter_span),
                Rect::new(40, 1, 20, 1),
            );
            charts::time_series(frame, Rect::new(0, 5, 20, 5), &theme, "ms", &series);
            charts::gauges(frame, Rect::new(22, 5, 20, 2), &theme, &gauges);
            charts::flame(frame, Rect::new(44, 5, 12, 2), &theme, &flame);
            charts::boxplot(frame, Rect::new(0, 11, 20, 5), &theme, &boxes);
        })
        .expect("core model painters render");

    let buffer = terminal.backend().buffer();
    assert_cells(buffer, 0, 0, "node");
    assert_eq!(buffer[(40, 0)].symbol(), "●");
    assert_eq!(buffer[(40, 1)].symbol(), "█");
}

fn assert_cells(buffer: &ratatui::buffer::Buffer, x: u16, y: u16, expected: &str) {
    for (offset, ch) in expected.chars().enumerate() {
        assert_eq!(buffer[(x + offset as u16, y)].symbol(), ch.to_string());
    }
}

fn render_to_buffer(render: impl FnMut(&mut ratatui::Frame)) -> ratatui::buffer::Buffer {
    render_to_buffer_size(64, 16, render)
}

fn render_to_buffer_size(
    width: u16,
    height: u16,
    mut render: impl FnMut(&mut ratatui::Frame),
) -> ratatui::buffer::Buffer {
    let mut terminal =
        Terminal::new(TestBackend::new(width, height)).expect("test backend initializes");
    terminal.draw(|frame| render(frame)).expect("render succeeds");
    terminal.backend().buffer().clone()
}

fn native_rows_from_core_table(model: &TableModel) -> Vec<Row<'_>> {
    model
        .rows
        .iter()
        .map(|row| Row::new(row.cells.iter().map(native_cell_from_core_cell)))
        .collect()
}

fn native_cell_from_core_cell(cell: &TableCell) -> Cell<'_> {
    match cell {
        TableCell::Text(text) => Cell::from(text.as_str()),
        TableCell::Status { label, color } => {
            Cell::from(panel_kit_tui::status::labeled_borrowed(*color, label))
        }
        TableCell::Meter { ratio, text, color } => {
            let span = match color {
                Some(color) => panel_kit_tui::meter::span(*ratio, 8, *color),
                None => ratatui::text::Span::raw(core_meter::bar(*ratio, 8)),
            };
            if text.is_empty() {
                return Cell::from(ratatui::text::Line::from(span));
            }
            Cell::from(ratatui::text::Line::from(vec![
                span,
                ratatui::text::Span::raw(" "),
                ratatui::text::Span::raw(text.as_str()),
            ]))
        }
    }
}

fn render_core_canary_provider_buffer() -> ratatui::buffer::Buffer {
    let theme = ResolvedTuiTheme::default();
    let badges = workspace_canary::demo_badges();
    let table_model = workspace_canary::node_rows();
    let metrics = workspace_canary::Metrics::new();
    let capacity = workspace_canary::capacity_items();
    let series = metrics.series();
    let flame = metrics.flame();
    let boxes = metrics.boxes();
    let table_view = TableView {
        columns: &table_model.columns,
        rows: &table_model.rows,
    };

    render_to_buffer_size(96, 24, |frame| {
        render_badges(frame, Rect::new(0, 0, 36, 11), &theme, &badges, badge::spans);
        charts::time_series(frame, Rect::new(38, 0, 30, 7), &theme, "ms", &series);
        charts::gauges(frame, Rect::new(70, 0, 25, 4), &theme, &capacity);
        charts::flame(frame, Rect::new(38, 8, 40, 4), &theme, &flame);
        table::table(frame, Rect::new(0, 12, 38, 5), &theme, table_view);
        charts::boxplot(frame, Rect::new(40, 13, 50, 7), &theme, &boxes);
    })
}

fn render_legacy_canary_provider_buffer() -> ratatui::buffer::Buffer {
    let theme = ResolvedTuiTheme::default();
    let badges = workspace_canary::demo_badges();
    let table_model = workspace_canary::node_rows();
    let metrics = workspace_canary::Metrics::new();
    let capacity = workspace_canary::capacity_items();
    let series = metrics.series();
    let flame = metrics.flame();
    let boxes = metrics.boxes();
    let header: Vec<&str> = table_model
        .columns
        .iter()
        .map(|column| column.title.as_str())
        .collect();
    let widths: Vec<Constraint> = table_model
        .columns
        .iter()
        .map(|column| native_width_from_core_column(column.width))
        .collect();
    let rows = native_rows_from_core_table(&table_model);

    render_to_buffer_size(96, 24, |frame| {
        render_badges(
            frame,
            Rect::new(0, 0, 36, 11),
            &theme,
            &badges,
            legacy_badge_spans,
        );
        legacy_chart_painters::time_series(
            frame,
            Rect::new(38, 0, 30, 7),
            &theme,
            "ms",
            &series,
        );
        legacy_chart_painters::gauges(frame, Rect::new(70, 0, 25, 4), &theme, &capacity);
        legacy_chart_painters::flame(
            frame,
            Rect::new(38, 8, 40, 4),
            &theme,
            &flame,
        );
        table::table_native(
            frame,
            Rect::new(0, 12, 38, 5),
            &theme,
            &header,
            &widths,
            rows.clone(),
        );
        legacy_chart_painters::boxplot(
            frame,
            Rect::new(40, 13, 50, 7),
            &theme,
            &boxes,
        );
    })
}

fn render_badges(
    frame: &mut ratatui::Frame,
    area: Rect,
    theme: &ResolvedTuiTheme,
    badges: &[BadgeSpec],
    spans: fn(&BadgeSpec, &ResolvedTuiTheme) -> Vec<Span<'static>>,
) {
    for (row, spec) in badges.iter().take(area.height as usize).enumerate() {
        let rect = Rect::new(
            area.x,
            area.y + row as u16,
            badge::width(spec).min(area.width),
            1,
        );
        frame.render_widget(Paragraph::new(Line::from(spans(spec, theme))), rect);
    }
}

fn legacy_badge_spans(spec: &BadgeSpec, theme: &ResolvedTuiTheme) -> Vec<Span<'static>> {
    let mut style = Style::default().fg(legacy_badge_color(spec, theme));
    if spec.active {
        style = style.add_modifier(Modifier::REVERSED);
    }

    vec![
        Span::styled("[", Style::default().fg(theme.line2)),
        Span::styled(display_label(&spec.kind, &spec.value), style),
        Span::styled("]", Style::default().fg(theme.line2)),
    ]
}

fn legacy_badge_color(spec: &BadgeSpec, theme: &ResolvedTuiTheme) -> Color {
    if let Some((r, g, b)) = spec.override_color {
        return Color::Rgb(r, g, b);
    }

    match &spec.kind {
        BadgeKind::Tag => theme.badge_info,
        BadgeKind::Doctype | BadgeKind::Author => theme.badge_info,
        BadgeKind::Folder => theme.dim,
        BadgeKind::Entity { .. } | BadgeKind::Date => theme.yellow,
        BadgeKind::Wikilink { resolved: true, .. } => theme.badge_info,
        BadgeKind::Wikilink {
            resolved: false, ..
        } => theme.red,
        BadgeKind::Url { .. } | BadgeKind::Status => theme.green,
        BadgeKind::Generic => theme.line2,
    }
}

fn native_width_from_core_column(width: ColumnWidth) -> Constraint {
    match width {
        ColumnWidth::Fixed { value } => Constraint::Length(value),
        ColumnWidth::Flex { weight } => Constraint::Fill(weight),
    }
}
