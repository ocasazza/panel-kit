#[path = "support/composition.rs"]
mod support;

use panel_kit_core::frame::ProjectionBuffer;
use panel_kit_core::reducer::{HitTarget, PanelPart, WheelDisposition, WorkspaceEvent};
use panel_kit_core::widgets::table::{
    ColumnWidth, TableCell, TableColumn, TableRow, TableView, TextAlign,
};
use panel_kit_core::Mode;
use panel_kit_tui::input::{route_wheel, route_wheel_to, WheelDelta};
use panel_kit_tui::scroll::PanelScroll;
use panel_kit_tui::widgets::panel::{draw_panel_chrome, draw_panel_surface};
use panel_kit_tui::widgets::TuiHitBuffer;
use panel_kit_tui::{scroll, table, Charset, ResolvedTuiTheme};
use ratatui::layout::Rect;
use ratatui::text::Line;

use support::{fixed_snapshot, project_cells, render_to_buffer, TestPanel};

/// A drawn panel body plus the hit buffer the painters recorded.
struct Drawn {
    hits: TuiHitBuffer<TestPanel>,
    /// A point inside the panel body.
    body_point: (f64, f64),
    /// A point inside the same panel's header chrome.
    header_point: (f64, f64),
}

fn text_lines(count: usize) -> Vec<Line<'static>> {
    (0..count)
        .map(|i| Line::from(format!("{i:>2} long content line")))
        .collect()
}

fn wide_lines(count: usize) -> Vec<Line<'static>> {
    (0..count)
        .map(|i| Line::from(format!("row-{i}-{}", "x".repeat(40))))
        .collect()
}

/// Draw `key` with `content` as body text and record the hits the painters made.
fn drawn_body(
    key: TestPanel,
    content: Vec<Line<'static>>,
    registry: &mut PanelScroll<TestPanel>,
) -> Drawn {
    let (snapshot, catalog) = fixed_snapshot(Mode::Floating);
    let mut scratch = ProjectionBuffer::with_panel_capacity(snapshot.panels.len());
    let projected = project_cells(&snapshot, &mut scratch);
    let panel = projected
        .panels
        .iter()
        .copied()
        .find(|panel| panel.key == key)
        .expect("panel projects");
    let meta = catalog.get(key).expect("panel metadata exists");
    let theme = ResolvedTuiTheme::default();
    let mut hits = TuiHitBuffer::with_capacity(2, 0);

    render_to_buffer(80, 24, |frame| {
        draw_panel_surface(frame, panel, &theme, Charset::Ascii, &mut hits);
        let body = draw_panel_chrome(frame, panel, meta, &theme, Charset::Ascii, &mut hits);
        scroll::lines(frame, body, &theme, &key, registry, content.clone());
    });

    let body = Rect {
        x: panel.chrome.body.x as u16,
        y: panel.chrome.body.y as u16,
        width: panel.chrome.body.w as u16,
        height: panel.chrome.body.h as u16,
    };
    let body_point = (f64::from(body.x) + 2.0, f64::from(body.y) + 1.0);
    assert_eq!(
        hits.hit_test(body_point),
        Some(HitTarget::Panel {
            key,
            part: PanelPart::Surface
        })
    );

    Drawn {
        hits,
        body_point,
        header_point: (f64::from(body.x) + 2.0, f64::from(body.y) - 1.0),
    }
}

fn node_columns() -> Vec<TableColumn> {
    vec![
        TableColumn {
            key: "node".into(),
            title: "node".into(),
            width: ColumnWidth::Fixed { value: 8 },
            align: TextAlign::Left,
        },
        TableColumn {
            key: "load".into(),
            title: "load".into(),
            width: ColumnWidth::Fixed { value: 6 },
            align: TextAlign::Left,
        },
    ]
}

fn node_rows(count: usize) -> Vec<TableRow> {
    (0..count)
        .map(|i| TableRow {
            cells: vec![
                TableCell::Text(format!("n{i:02}")),
                TableCell::Text(format!("{}", i % 10)),
            ],
        })
        .collect()
}

#[test]
fn wheel_over_an_overflowing_body_moves_its_offset_and_is_consumed() {
    let mut panels = PanelScroll::new();
    let drawn = drawn_body(TestPanel::Alpha, text_lines(40), &mut panels);
    assert!(panels.is_scrollable(&TestPanel::Alpha));

    let event = route_wheel(
        &drawn.hits,
        &mut panels,
        drawn.body_point,
        WheelDelta::new(0.0, 3.0),
    );

    assert_eq!(
        event,
        WorkspaceEvent::Wheel {
            delta_y: 3.0,
            disposition: WheelDisposition::ContentConsumed,
        }
    );
    assert_eq!(panels.offset(&TestPanel::Alpha).1, 3);
}

#[test]
fn wheel_at_the_content_edge_bubbles_to_the_workspace() {
    let mut panels = PanelScroll::new();
    let drawn = drawn_body(TestPanel::Alpha, text_lines(40), &mut panels);
    let bottom = f64::from(panels.max(&TestPanel::Alpha).1);

    let to_end = route_wheel(
        &drawn.hits,
        &mut panels,
        drawn.body_point,
        WheelDelta::new(0.0, bottom + 10.0),
    );
    assert_eq!(
        to_end,
        WorkspaceEvent::Wheel {
            delta_y: bottom + 10.0,
            disposition: WheelDisposition::ContentConsumed,
        }
    );
    assert_eq!(f64::from(panels.offset(&TestPanel::Alpha).1), bottom);

    let past_end = route_wheel(
        &drawn.hits,
        &mut panels,
        drawn.body_point,
        WheelDelta::new(0.0, 1.0),
    );
    assert_eq!(
        past_end,
        WorkspaceEvent::Wheel {
            delta_y: 1.0,
            disposition: WheelDisposition::BubbleToWorkspace,
        }
    );

    let back_up = route_wheel(
        &drawn.hits,
        &mut panels,
        drawn.body_point,
        WheelDelta::new(0.0, -1.0),
    );
    assert_eq!(
        back_up,
        WorkspaceEvent::Wheel {
            delta_y: -1.0,
            disposition: WheelDisposition::ContentConsumed,
        }
    );
    assert_eq!(f64::from(panels.offset(&TestPanel::Alpha).1), bottom - 1.0);
}

#[test]
fn wheel_over_a_panel_whose_content_fits_bubbles() {
    let mut panels = PanelScroll::new();
    let drawn = drawn_body(
        TestPanel::Beta,
        vec![Line::from("short"), Line::from("lines")],
        &mut panels,
    );

    for delta in [WheelDelta::new(0.0, 1.0), WheelDelta::new(0.0, -1.0)] {
        let event = route_wheel(&drawn.hits, &mut panels, drawn.body_point, delta);
        assert_eq!(
            event,
            WorkspaceEvent::Wheel {
                delta_y: delta.y,
                disposition: WheelDisposition::BubbleToWorkspace,
            }
        );
    }
    assert_eq!(panels.offset(&TestPanel::Beta), (0, 0));
}

#[test]
fn a_measured_panel_only_owns_its_own_wheel_gestures() {
    let mut panels = PanelScroll::new();
    let alpha = drawn_body(TestPanel::Alpha, text_lines(40), &mut panels);

    route_wheel(
        &alpha.hits,
        &mut panels,
        alpha.body_point,
        WheelDelta::new(0.0, 2.0),
    );
    assert_eq!(panels.offset(&TestPanel::Alpha).1, 2);

    let over_beta = route_wheel(
        &alpha.hits,
        &mut panels,
        (60.0, 5.0),
        WheelDelta::new(0.0, 2.0),
    );
    assert_eq!(
        over_beta,
        WorkspaceEvent::Wheel {
            delta_y: 2.0,
            disposition: WheelDisposition::BubbleToWorkspace,
        }
    );
    assert_eq!(panels.offset(&TestPanel::Beta), (0, 0));
}

#[test]
fn horizontal_wheel_scrolls_wide_lines_and_bubbles_at_the_side_edges() {
    let mut panels = PanelScroll::new();
    let drawn = drawn_body(TestPanel::Alpha, wide_lines(4), &mut panels);
    assert!(panels.is_scrollable(&TestPanel::Alpha));

    let consumed = route_wheel(
        &drawn.hits,
        &mut panels,
        drawn.body_point,
        WheelDelta::new(4.0, 0.0),
    );
    assert_eq!(
        consumed,
        WorkspaceEvent::Wheel {
            delta_y: 0.0,
            disposition: WheelDisposition::ContentConsumed,
        }
    );
    assert_eq!(panels.offset(&TestPanel::Alpha).0, 4);

    let edge = f64::from(panels.max(&TestPanel::Alpha).0);
    route_wheel(
        &drawn.hits,
        &mut panels,
        drawn.body_point,
        WheelDelta::new(edge * 2.0, 0.0),
    );
    assert_eq!(f64::from(panels.offset(&TestPanel::Alpha).0), edge);

    let bubbled = route_wheel(
        &drawn.hits,
        &mut panels,
        drawn.body_point,
        WheelDelta::new(1.0, 0.0),
    );
    assert_eq!(
        bubbled,
        WorkspaceEvent::Wheel {
            delta_y: 0.0,
            disposition: WheelDisposition::BubbleToWorkspace,
        }
    );
}

#[test]
fn wheel_over_panel_chrome_rather_than_a_body_bubbles() {
    let mut panels = PanelScroll::new();
    let drawn = drawn_body(TestPanel::Alpha, text_lines(40), &mut panels);
    route_wheel(
        &drawn.hits,
        &mut panels,
        drawn.body_point,
        WheelDelta::new(0.0, 2.0),
    );

    let event = route_wheel(
        &drawn.hits,
        &mut panels,
        drawn.header_point,
        WheelDelta::new(0.0, 2.0),
    );

    assert_eq!(
        event,
        WorkspaceEvent::Wheel {
            delta_y: 2.0,
            disposition: WheelDisposition::BubbleToWorkspace,
        }
    );
    assert_eq!(panels.offset(&TestPanel::Alpha).1, 2);
}

#[test]
fn routing_to_a_named_panel_skips_hit_testing() {
    let mut panels = PanelScroll::new();
    drawn_body(TestPanel::Alpha, text_lines(40), &mut panels);

    let focused = route_wheel_to(&mut panels, &TestPanel::Alpha, WheelDelta::new(0.0, 4.0));
    assert_eq!(
        focused,
        WorkspaceEvent::Wheel {
            delta_y: 4.0,
            disposition: WheelDisposition::ContentConsumed,
        }
    );
    assert_eq!(panels.offset(&TestPanel::Alpha).1, 4);

    let unmeasured = route_wheel_to(&mut panels, &TestPanel::Docked, WheelDelta::new(0.0, 4.0));
    assert_eq!(
        unmeasured,
        WorkspaceEvent::Wheel {
            delta_y: 4.0,
            disposition: WheelDisposition::BubbleToWorkspace,
        }
    );
}

#[test]
fn table_bodies_scroll_rows_under_a_sticky_header() {
    let theme = ResolvedTuiTheme::default();
    let columns = node_columns();
    let rows = node_rows(20);
    let mut panels = PanelScroll::new();
    let area = Rect::new(0, 0, 20, 6);

    let buffer = render_to_buffer(20, 6, |frame| {
        table::table(
            frame,
            area,
            &theme,
            &TestPanel::Alpha,
            &mut panels,
            TableView {
                columns: &columns,
                rows: &rows,
            },
        );
    });
    assert!(support::buffer_contains(&buffer, "node"));
    assert!(support::buffer_contains(&buffer, "n00"));
    // One header row plus a scrollbar column leaves five body rows visible.
    assert_eq!(panels.max(&TestPanel::Alpha).1, 15);

    panels.absorb(&TestPanel::Alpha, 0.0, 4.0);
    let scrolled = render_to_buffer(20, 6, |frame| {
        table::table(
            frame,
            area,
            &theme,
            &TestPanel::Alpha,
            &mut panels,
            TableView {
                columns: &columns,
                rows: &rows,
            },
        );
    });

    assert!(support::buffer_contains(&scrolled, "node"));
    assert!(support::buffer_contains(&scrolled, "n04"));
    assert!(!support::buffer_contains(&scrolled, "n00 "));
}

#[test]
fn table_wheels_scroll_columns_horizontally_when_fixed_widths_overflow() {
    let theme = ResolvedTuiTheme::default();
    let columns = node_columns();
    let rows = node_rows(2);
    let mut panels = PanelScroll::new();

    render_to_buffer(10, 4, |frame| {
        table::table(
            frame,
            Rect::new(0, 0, 10, 4),
            &theme,
            &TestPanel::Alpha,
            &mut panels,
            TableView {
                columns: &columns,
                rows: &rows,
            },
        );
    });
    // 8 + 6 cells of fixed columns plus one spacing column do not fit 10 cells.
    assert_eq!(panels.max(&TestPanel::Alpha).0, 15 - 10);

    assert_eq!(
        panels.absorb(&TestPanel::Alpha, 1.0, 0.0),
        WheelDisposition::ContentConsumed
    );
    assert_eq!(panels.offset(&TestPanel::Alpha).0, 1);

    let scrolled = render_to_buffer(10, 4, |frame| {
        table::table(
            frame,
            Rect::new(0, 0, 10, 4),
            &theme,
            &TestPanel::Alpha,
            &mut panels,
            TableView {
                columns: &columns,
                rows: &rows,
            },
        );
    });
    assert!(support::buffer_contains(&scrolled, "load"));
    assert!(!support::buffer_contains(&scrolled, "node"));
}

#[test]
fn lines_paint_the_offset_window_and_record_the_extent() {
    let theme = ResolvedTuiTheme::default();
    let mut panels = PanelScroll::new();
    let area = Rect::new(0, 0, 12, 4);
    let content: Vec<Line<'static>> = (0..10).map(|i| Line::from(format!("row-{i}"))).collect();

    let buffer = render_to_buffer(12, 4, |frame| {
        scroll::lines(
            frame,
            area,
            &theme,
            &TestPanel::Alpha,
            &mut panels,
            content.clone(),
        );
    });
    assert!(support::buffer_contains(&buffer, "row-0"));
    assert!(!support::buffer_contains(&buffer, "row-4"));
    // Six characters in an 11-cell body that reserved a scrollbar column.
    assert_eq!(panels.max(&TestPanel::Alpha), (0, 6));

    panels.absorb(&TestPanel::Alpha, 0.0, 3.0);
    let scrolled = render_to_buffer(12, 4, |frame| {
        scroll::lines(
            frame,
            area,
            &theme,
            &TestPanel::Alpha,
            &mut panels,
            content.clone(),
        );
    });
    assert!(support::buffer_contains(&scrolled, "row-3"));
    assert!(!support::buffer_contains(&scrolled, "row-0 "));
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn crossterm_scroll_events_route_through_the_panel_registry() {
    use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
    use panel_kit_tui::input::crossterm_workspace_event;

    let mut panels = PanelScroll::new();
    let drawn = drawn_body(TestPanel::Alpha, wide_lines(40), &mut panels);
    let at = |kind, modifiers| MouseEvent {
        kind,
        column: drawn.body_point.0 as u16,
        row: drawn.body_point.1 as u16,
        modifiers,
    };

    let consumed = crossterm_workspace_event(
        &drawn.hits,
        &mut panels,
        at(MouseEventKind::ScrollDown, KeyModifiers::NONE),
    );
    assert_eq!(
        consumed,
        Some(WorkspaceEvent::Wheel {
            delta_y: 1.0,
            disposition: WheelDisposition::ContentConsumed,
        })
    );
    assert_eq!(panels.offset(&TestPanel::Alpha).1, 1);

    let horizontal = crossterm_workspace_event(
        &drawn.hits,
        &mut panels,
        at(MouseEventKind::ScrollDown, KeyModifiers::SHIFT),
    );
    assert_eq!(
        horizontal,
        Some(WorkspaceEvent::Wheel {
            delta_y: 0.0,
            disposition: WheelDisposition::ContentConsumed,
        })
    );
    assert_eq!(panels.offset(&TestPanel::Alpha).0, 1);

    let click = crossterm_workspace_event(
        &drawn.hits,
        &mut panels,
        at(MouseEventKind::Down(MouseButton::Left), KeyModifiers::NONE),
    );
    assert!(matches!(click, Some(WorkspaceEvent::Pointer { .. })));
}
