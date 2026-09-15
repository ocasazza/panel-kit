use crate::reducer::{ReduceContext, Snapshot, Viewport};
use crate::{
    Clamp, CommandStep, Key, KeyChord, LayoutBuilder, Mode, PanelKind, PanelWin,
    SurfaceCapabilities, SurfaceProfile, TileMetrics, Units, WEB_COMPACT_MAX, WEB_TABLET_MAX,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(super) enum TestPanel {
    First,
    Second,
    Third,
    Gone,
}

impl PanelKind for TestPanel {
    fn title(self) -> &'static str {
        match self {
            Self::First => "First",
            Self::Second => "Second",
            Self::Third => "Third",
            Self::Gone => "Gone",
        }
    }
}

pub(super) fn panels() -> Vec<PanelWin<TestPanel>> {
    let mut layout = LayoutBuilder::new();
    vec![
        layout.at(TestPanel::First, 20.0, 20.0, 300.0, 200.0),
        layout.at(TestPanel::Second, 80.0, 90.0, 300.0, 200.0),
        layout.at(TestPanel::Third, 140.0, 160.0, 300.0, 200.0),
    ]
}

pub(super) fn web_viewport() -> Viewport {
    Viewport {
        width: 1000.0,
        height: 800.0,
        units: Units::CssPx,
    }
}

pub(super) fn web_context() -> ReduceContext<'static> {
    ReduceContext {
        surface: SurfaceProfile::from_logical_width(
            1000.0,
            WEB_COMPACT_MAX,
            WEB_TABLET_MAX,
            SurfaceCapabilities {
                coarse_pointer: false,
                hover: true,
                keyboard: true,
            },
        ),
        clamp: &Clamp::WEB,
        command_step: CommandStep::WEB,
        tile: &TileMetrics::WEB,
    }
}

pub(super) fn default_snapshot() -> Snapshot<TestPanel> {
    Snapshot::from_defaults(panels(), Mode::Floating, web_viewport())
}

pub(super) fn chord(key: Key, shift: bool, alt: bool) -> KeyChord {
    KeyChord {
        key,
        shift,
        alt,
        ctrl: false,
        meta: false,
    }
}
