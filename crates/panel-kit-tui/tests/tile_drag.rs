#[path = "support/composition.rs"]
mod support;

use panel_kit_core::frame::{PanelProjection, ProjectionBuffer};
use panel_kit_core::reducer::{HitTarget, PanelPart, Snapshot};
use panel_kit_core::tile_drag::{TileDrag, TileDragPhase, TileRole};
use panel_kit_core::{Mode, PanelCatalog, Region};
use panel_kit_tui::widgets::{self, TuiHitBuffer};
use panel_kit_tui::{ResolvedTuiTheme, TerminalCharset};
use ratatui::buffer::Buffer;

use support::{fixed_snapshot, project_cells, rect_from_region, render_to_buffer, TestPanel};

/// Alpha lifted and dragged right by `offset` cells over Beta's slot.
fn lifted_alpha_over_beta(offset: f64) -> (Snapshot<TestPanel>, PanelCatalog<TestPanel>) {
    let (mut snapshot, catalog) = fixed_snapshot(Mode::Tiling);
    let mut drag = TileDrag::pressed(TestPanel::Alpha, (5.0, 2.0));
    drag.phase = TileDragPhase::Lifted;
    drag.pointer = (5.0 + offset, 2.0);
    drag.target = Some(TestPanel::Beta);
    drag.lift = 1.0;
    snapshot.tile_drag = Some(drag);
    (snapshot, catalog)
}

/// Paint every projected panel in frame order with unicode chrome.
fn paint(
    snapshot: &Snapshot<TestPanel>,
    catalog: &PanelCatalog<TestPanel>,
    hits: &mut TuiHitBuffer<TestPanel>,
) -> (Buffer, Vec<PanelProjection<TestPanel>>) {
    let theme = ResolvedTuiTheme::default();
    let mut scratch = ProjectionBuffer::with_panel_capacity(snapshot.panels.len());
    let frame = project_cells(snapshot, &mut scratch);
    let panels = frame.panels.to_vec();
    let buffer = render_to_buffer(80, 24, |terminal_frame| {
        hits.clear();
        for panel in panels.iter().copied() {
            let meta = catalog.get(panel.key).unwrap();
            widgets::panel::draw_panel_surface(
                terminal_frame,
                panel,
                &theme,
                TerminalCharset::Unicode,
                hits,
            );
            widgets::panel::draw_panel_chrome(
                terminal_frame,
                panel,
                meta,
                &theme,
                TerminalCharset::Unicode,
                hits,
            );
        }
    });
    (buffer, panels)
}

fn find(panels: &[PanelProjection<TestPanel>], key: TestPanel) -> PanelProjection<TestPanel> {
    *panels.iter().find(|panel| panel.key == key).unwrap()
}

fn center(region: Region) -> (f64, f64) {
    (region.x + region.w / 2.0, region.y + region.h / 2.0)
}

#[test]
fn recorded_hits_resolve_a_lifted_drag_against_resting_slots() {
    let (snapshot, catalog) = lifted_alpha_over_beta(30.0);
    let mut hits = TuiHitBuffer::with_capacity(2, 0);
    let (_, panels) = paint(&snapshot, &catalog, &mut hits);
    let (alpha, beta) = (
        find(&panels, TestPanel::Alpha),
        find(&panels, TestPanel::Beta),
    );
    let beta_slot = center(beta.slot);
    assert!(
        rect_from_region(alpha.region).contains(ratatui::layout::Position::new(
            beta_slot.0 as u16,
            beta_slot.1 as u16
        )),
        "the lifted tile is drawn over the target's slot"
    );

    let surface = |key| {
        Some(HitTarget::Panel {
            key,
            part: PanelPart::Surface,
        })
    };
    assert_eq!(hits.hit_test(beta_slot), surface(TestPanel::Beta));
    assert_eq!(hits.hit_test(center(alpha.slot)), surface(TestPanel::Alpha));
}

#[test]
fn a_lifted_tile_draws_thick_and_its_target_dashed() {
    let (snapshot, catalog) = lifted_alpha_over_beta(30.0);
    let mut hits = TuiHitBuffer::with_capacity(2, 0);
    let (buffer, panels) = paint(&snapshot, &catalog, &mut hits);
    let (alpha, beta) = (
        find(&panels, TestPanel::Alpha),
        find(&panels, TestPanel::Beta),
    );
    assert_eq!(
        (alpha.tile_role, beta.tile_role),
        (TileRole::Lifted, TileRole::Target)
    );

    let lifted = rect_from_region(alpha.chrome.outer);
    assert_eq!(buffer[(lifted.x, lifted.y)].symbol(), "┏");
    let shadow = &buffer[(lifted.right(), lifted.y + 1)];
    assert_eq!(
        shadow.bg,
        ResolvedTuiTheme::default().divider,
        "one-cell drop shadow"
    );

    let target = rect_from_region(beta.chrome.outer);
    assert_eq!(buffer[(target.x, target.bottom() - 1)].symbol(), "└");
    assert_eq!(buffer[(target.x, target.y + 1)].symbol(), "╎");
}
