use panel_kit_core::frame::{
    project_into, ChromeProjectionInput, ProjectionBuffer, ProjectionInput, TileLayoutMetrics,
};
use panel_kit_core::reducer::{Snapshot, Viewport};
use panel_kit_core::{
    ChromeMetrics, Clamp, LayoutBuilder, Mode, PanelKind, SurfaceCapabilities, SurfaceProfile,
    TileMetrics, Units, WEB_COMPACT_MAX, WEB_TABLET_MAX,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
enum StandalonePanel {
    Main,
}

impl PanelKind for StandalonePanel {
    fn title(self) -> &'static str {
        match self {
            Self::Main => "Main",
        }
    }
}

fn main() {
    let mut layout = LayoutBuilder::new();
    let snapshot = Snapshot::from_defaults(
        vec![layout.at(StandalonePanel::Main, 24.0, 32.0, 320.0, 180.0)],
        Mode::Floating,
        Viewport { width: 960.0, height: 640.0, units: Units::CssPx },
    );
    let surface = SurfaceProfile::from_logical_width(
        snapshot.viewport.width,
        WEB_COMPACT_MAX,
        WEB_TABLET_MAX,
        SurfaceCapabilities { coarse_pointer: false, hover: true, keyboard: true },
    );
    let tile = TileLayoutMetrics::from_tile_metrics(TileMetrics::WEB, surface);
    let chrome = ChromeProjectionInput::full(ChromeMetrics::WEB);
    let mut scratch = ProjectionBuffer::with_panel_capacity(snapshot.panels.len());
    let frame = project_into(
        ProjectionInput {
            snapshot: &snapshot,
            surface,
            chrome: &chrome,
            clamp: &Clamp::WEB,
            tile: &tile,
        },
        &mut scratch,
    );

    for panel in frame.panels {
        println!("{}: {:?}", panel.key.title(), panel.region);
    }
}
