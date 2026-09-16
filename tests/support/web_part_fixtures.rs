use dioxus::prelude::EventHandler;
use panel_kit_core::frame::{PanelChromeProjection, PanelProjection, Placement};
use panel_kit_core::panel::PanelMeta;
use panel_kit_core::reducer::WorkspaceEvent;
use panel_kit_core::{PanelKind, Region, WinState};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProbePanel {
    Nodes,
    Logs,
}

impl PanelKind for ProbePanel {
    fn title(self) -> &'static str {
        match self {
            ProbePanel::Nodes => "Nodes",
            ProbePanel::Logs => "Logs",
        }
    }
}

pub fn projected_panel(placement: Placement, focused: bool) -> PanelProjection<ProbePanel> {
    PanelProjection {
        source_index: 0,
        key: ProbePanel::Nodes,
        region: Region {
            x: 16.0,
            y: 24.0,
            w: 320.0,
            h: 180.0,
        },
        placement,
        z: 7,
        state: WinState::Floating,
        focused,
        pointer_dragging: false,
        tile_dragging: false,
        chrome: PanelChromeProjection {
            outer: Region {
                x: 16.0,
                y: 24.0,
                w: 320.0,
                h: 180.0,
            },
            body: Region {
                x: 16.0,
                y: 46.0,
                w: 320.0,
                h: 158.0,
            },
            header_hit: Region {
                x: 16.0,
                y: 24.0,
                w: 320.0,
                h: 22.0,
            },
            mode_hit: Some(Region {
                x: 24.0,
                y: 24.0,
                w: 24.0,
                h: 22.0,
            }),
            minimize_hit: Some(Region {
                x: 54.0,
                y: 24.0,
                w: 24.0,
                h: 22.0,
            }),
            maximize_hit: Some(Region {
                x: 84.0,
                y: 24.0,
                w: 24.0,
                h: 22.0,
            }),
            resize_hit: Some(Region {
                x: 312.0,
                y: 180.0,
                w: 24.0,
                h: 24.0,
            }),
        },
    }
}

pub fn nodes_meta() -> PanelMeta<ProbePanel> {
    PanelMeta {
        key: ProbePanel::Nodes,
        stable_id: "Nodes".into(),
        title: "Nodes".into(),
        slug: "nodes".into(),
    }
}

pub fn noop_workspace_event_handler() -> EventHandler<WorkspaceEvent<ProbePanel>> {
    EventHandler::new(|_: WorkspaceEvent<ProbePanel>| {})
}
