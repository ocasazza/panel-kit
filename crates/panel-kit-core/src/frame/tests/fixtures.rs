use serde::{Deserialize, Serialize};

use crate::frame::{project_into, ChromeProjectionInput, PanelProjection, ProjectedFrame, ProjectionBuffer, ProjectionInput, TileLayoutMetrics};
use crate::reducer::{Snapshot, Viewport};
use crate::{
    Clamp, ChromeMetrics, Drag, DragKind, LayoutBuilder, Mode, PanelKind, Region,
    SurfaceCapabilities, SurfaceProfile, TileMetrics, Units, WinState,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(super) enum TestPanel {
    Alpha,
    Beta,
    Gamma,
    Docked,
}

impl PanelKind for TestPanel {
    fn title(self) -> &'static str {
        match self {
            Self::Alpha => "Alpha",
            Self::Beta => "Beta",
            Self::Gamma => "Gamma",
            Self::Docked => "Docked",
        }
    }
}

pub(super) fn semantic_snapshot(mode: Mode) -> Snapshot<TestPanel> {
    let mut layout = LayoutBuilder::new();
    let mut panels = vec![
        layout.at(TestPanel::Alpha, 10.0, 20.0, 240.0, 160.0).with_tile(1, 2),
        layout.at(TestPanel::Beta, 60.0, 80.0, 220.0, 150.0).with_tile(2, 1),
        layout.at(TestPanel::Gamma, 120.0, 140.0, 280.0, 180.0).with_tile(1, 1),
        layout.at(TestPanel::Docked, 0.0, 0.0, 180.0, 110.0).with_tile(1, 1),
    ];
    panels[0].z = 30;
    panels[1].z = 10;
    panels[2].z = 30;
    panels[3].state = WinState::Minimized;

    Snapshot {
        panels,
        preferred_mode: mode,
        viewport: Viewport { width: 800.0, height: 600.0, units: Units::CssPx },
        focused: Some(TestPanel::Gamma),
        drag: Some(Drag { idx: 2, kind: DragKind::Move, start_x: 120.0, start_y: 140.0, start_w: 280.0, start_h: 180.0, start_mx: 130.0, start_my: 150.0 }),
        tile_drag: Some(TestPanel::Beta),
        workspace_scroll: 25.0,
    }
}

pub(super) fn project_snapshot<'a>(
    snapshot: &Snapshot<TestPanel>,
    scratch: &'a mut ProjectionBuffer<TestPanel>,
) -> ProjectedFrame<'a, TestPanel> {
    let chrome = ChromeProjectionInput::full(ChromeMetrics::WEB);
    let tile = TileLayoutMetrics::from_tile_metrics(TileMetrics::WEB, regular_surface());
    project_into(
        ProjectionInput {
            snapshot,
            surface: regular_surface(),
            chrome: &chrome,
            clamp: &Clamp::WEB,
            tile: &tile,
        },
        scratch,
    )
}

pub(super) fn scratch_for(snapshot: &Snapshot<TestPanel>) -> ProjectionBuffer<TestPanel> {
    ProjectionBuffer::with_panel_capacity(snapshot.panels.len())
}
pub(super) fn regular_surface() -> SurfaceProfile {
    SurfaceProfile::from_logical_width(
        800.0,
        crate::WEB_COMPACT_MAX,
        crate::WEB_TABLET_MAX,
        SurfaceCapabilities { coarse_pointer: false, hover: true, keyboard: true },
    )
}

pub(super) fn keys(panels: &[PanelProjection<TestPanel>]) -> Vec<TestPanel> {
    panels.iter().map(|panel| panel.key).collect()
}

pub(super) fn center(region: Region) -> (f64, f64) {
    (region.x + region.w / 2.0, region.y + region.h / 2.0)
}

pub(super) fn assert_rect_clean(region: Region) {
    assert!(region.x.is_finite());
    assert!(region.y.is_finite());
    assert!(region.w.is_finite() && region.w >= 0.0);
    assert!(region.h.is_finite() && region.h >= 0.0);
}
