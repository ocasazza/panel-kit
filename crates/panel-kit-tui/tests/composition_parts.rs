#[path = "support/composition.rs"]
mod support;

use panel_kit_core::frame::{
    project_dock_into, project_panel, ChromeProjectionInput, PanelProjectionInput, Placement,
    ProjectionBuffer, TileGridProjection,
};
use panel_kit_core::reducer::{HitTarget, PanelPart};
use panel_kit_core::{ChromeMetrics, Clamp, LayoutBuilder, Mode, PanelCatalog, Region, WinState};
use panel_kit_tui::widgets::{self, TuiHitBuffer};
use panel_kit_tui::{Charset, ResolvedTuiTheme};
use ratatui::layout::Rect;
use ratatui::widgets::Paragraph;

use support::{
    buffer_contains, fixed_snapshot, project_cells, rect_from_region, regular_cells_surface,
    render_to_buffer, TestPanel,
};

#[test]
fn tui_panel_inner_matches_projection() {
    let (snapshot, catalog) = fixed_snapshot(Mode::Floating);
    let mut scratch = ProjectionBuffer::with_panel_capacity(snapshot.panels.len());
    let projected = project_cells(&snapshot, &mut scratch);
    let panel = projected
        .panels
        .iter()
        .find(|panel| panel.key == TestPanel::Alpha)
        .copied()
        .expect("alpha projects");
    let meta = catalog.get(panel.key).expect("alpha metadata exists");
    let mut hit_buffer = TuiHitBuffer::with_capacity(1, 0);
    let expected = rect_from_region(panel.chrome.body);
    let mut actual = None;

    render_to_buffer(80, 24, |frame| {
        actual = Some(widgets::panel::draw_panel_chrome(
            frame,
            panel,
            meta,
            &ResolvedTuiTheme::default(),
            Charset::Ascii,
            &mut hit_buffer,
        ));
    });

    assert_eq!(actual, Some(expected));
}

#[test]
fn standalone_panel_surface_draws_no_implicit_chrome_or_dock() {
    let mut layout = LayoutBuilder::new();
    let panel = layout.at(TestPanel::Alpha, 2.0, 1.0, 14.0, 5.0);
    let chrome = ChromeProjectionInput::surface_only(ChromeMetrics::CELLS);
    let projected = project_panel(
        &panel,
        0,
        PanelProjectionInput {
            viewport: Region::new(0.0, 0.0, 40.0, 12.0),
            preferred_mode: Mode::Floating,
            surface: regular_cells_surface(),
            focused: true,
            pointer_dragging: false,
            tile_dragging: false,
            workspace_scroll: 0.0,
            clamp: &Clamp::CELLS,
            chrome: &chrome,
            tile_grid: None,
            tiled_origin: None,
        },
    )
    .expect("surface-only panel projects");
    let mut hit_buffer = TuiHitBuffer::with_capacity(1, 0);
    let expected = rect_from_region(projected.region);
    let mut actual = None;

    let buffer = render_to_buffer(40, 12, |frame| {
        let body = widgets::panel::draw_panel_surface(
            frame,
            projected,
            &ResolvedTuiTheme::default(),
            Charset::Ascii,
            &mut hit_buffer,
        );
        actual = Some(body);
        frame.render_widget(Paragraph::new("BODY"), body);
    });

    assert_eq!(actual, Some(expected));
    assert_cells(&buffer, expected.x, expected.y, "BODY");
    assert_row_is_blank(&buffer, expected.x, expected.y + 1, expected.width);
    assert!(!buffer_contains(&buffer, "Alpha"));
    assert!(!buffer_contains(&buffer, "dock:"));
    assert_eq!(
        hit_buffer.hit_test((expected.x as f64, expected.y as f64)),
        Some(HitTarget::Panel {
            key: TestPanel::Alpha,
            part: PanelPart::Surface
        })
    );
}

#[test]
fn draw_dock_is_callable_alone_for_replace_only_dock_composition() {
    let mut layout = LayoutBuilder::new();
    let mut panels = vec![
        layout.at(TestPanel::Alpha, 0.0, 0.0, 20.0, 5.0),
        layout.at(TestPanel::Docked, 0.0, 0.0, 20.0, 5.0),
    ];
    panels[1].state = WinState::Minimized;
    let catalog = PanelCatalog::from_panel_kind_layout(&panels).expect("catalog builds");
    let dock_region = Region::new(0.0, 8.0, 40.0, 3.0);
    let mut dock = Vec::with_capacity(1);
    project_dock_into(&panels, dock_region, &mut dock);
    let mut hit_buffer = TuiHitBuffer::with_capacity(0, 1);

    let theme = ResolvedTuiTheme::default();
    let buffer = render_to_buffer(40, 12, |frame| {
        widgets::dock::draw_dock(
            frame,
            rect_from_region(dock_region),
            &dock,
            widgets::dock::DockRenderContext {
                catalog: &catalog,
                label: "dock:",
                theme: &theme,
                charset: Charset::Ascii,
            },
            &mut hit_buffer,
        );
    });

    assert!(buffer_contains(&buffer, "dock:"));
    assert!(buffer_contains(&buffer, "Docked"));
    assert_eq!(
        hit_buffer.hit_test((dock[0].region.x + 0.5, dock[0].region.y + 0.5)),
        Some(HitTarget::Dock {
            key: TestPanel::Docked
        })
    );
}

#[test]
fn web_and_tui_use_same_tile_grid_projection() {
    let grid = TileGridProjection {
        columns: 4,
        rows: 3,
        track_w: 5.0,
        track_h: 2.0,
        gap: 1.0,
        padding: 1.0,
    };
    let placement = Placement::Tiled {
        column: 1,
        row: 2,
        column_span: 2,
        row_span: 1,
    };

    assert_eq!(
        widgets::panel::tiled_cell_rect(placement, grid, Rect::new(10, 5, 30, 10), 1.0),
        Some(Rect::new(17, 11, 11, 2))
    );
}

fn assert_cells(buffer: &ratatui::buffer::Buffer, x: u16, y: u16, expected: &str) {
    for (offset, ch) in expected.chars().enumerate() {
        assert_eq!(buffer[(x + offset as u16, y)].symbol(), ch.to_string());
    }
}

fn assert_row_is_blank(buffer: &ratatui::buffer::Buffer, x: u16, y: u16, width: u16) {
    for offset in 0..width {
        assert_eq!(buffer[(x + offset, y)].symbol(), " ");
    }
}
