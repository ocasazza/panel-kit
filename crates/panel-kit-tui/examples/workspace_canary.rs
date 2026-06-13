use panel_kit_core::badge::{tag_hue, BadgeKind};
use panel_kit_core::{LayoutBuilder, PanelKind, PanelWin};
use panel_kit_tui::badge::{hue_color, Badge};
use panel_kit_tui::charts::GaugeItem;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Panel {
    Workspace,
    Badges,
    Activity,
    Capacity,
    Notes,
    Theme,
}

impl PanelKind for Panel {
    fn title(self) -> &'static str {
        match self {
            Panel::Workspace => "Workspace",
            Panel::Badges => "Badges",
            Panel::Activity => "Activity",
            Panel::Capacity => "Capacity",
            Panel::Notes => "Notes",
            Panel::Theme => "Theme",
        }
    }
}

pub fn defaults() -> Vec<PanelWin<Panel>> {
    let mut b = LayoutBuilder::new();
    vec![
        b.at(Panel::Workspace, 1.0, 0.0, 48.0, 12.0).with_tile(2, 2),
        b.at(Panel::Badges, 51.0, 0.0, 58.0, 16.0).with_tile(2, 3),
        b.at(Panel::Activity, 1.0, 13.0, 58.0, 15.0).with_tile(2, 3),
        b.at(Panel::Capacity, 61.0, 17.0, 48.0, 10.0)
            .with_tile(2, 2),
        b.at(Panel::Notes, 1.0, 29.0, 64.0, 12.0).with_tile(3, 2),
        b.at(Panel::Theme, 67.0, 28.0, 42.0, 10.0).with_tile(1, 2),
    ]
}

pub fn demo_badges() -> Vec<Badge> {
    let mut tag = Badge::new(BadgeKind::Tag, "tag", "browser-tui");
    tag.override_color = Some(hue_color(tag_hue("browser-tui")));
    let mut active = Badge::new(BadgeKind::Status, "status", "canary");
    active.active = true;
    vec![
        tag,
        Badge::new(BadgeKind::Doctype, "doctype", "example"),
        Badge::new(BadgeKind::Folder, "folder", "crates/panel-kit-tui"),
        Badge::new(BadgeKind::Author, "author", "olive"),
        Badge::new(
            BadgeKind::Entity {
                ty: Some("crate".into()),
            },
            "entity",
            "panel-kit-core",
        ),
        Badge::new(
            BadgeKind::Wikilink {
                resolved: true,
                target: "TuiWorkspace".into(),
            },
            "link",
            "TuiWorkspace",
        ),
        Badge::new(
            BadgeKind::Wikilink {
                resolved: false,
                target: "missing-doc".into(),
            },
            "link",
            "missing-doc",
        ),
        Badge::new(
            BadgeKind::Url {
                href: "https://github.com/ratatui/ratzilla".into(),
                host: "github.com".into(),
            },
            "url",
            "ratzilla",
        ),
        Badge::new(BadgeKind::Date, "date", "2026-06-13"),
        active,
        Badge::new(BadgeKind::Generic, "mode", "wasm"),
    ]
}

pub const EVAL_SERIES: &[(f64, f64)] = &[
    (0.0, 8.0),
    (1.0, 12.0),
    (2.0, 11.0),
    (3.0, 19.0),
    (4.0, 24.0),
    (5.0, 21.0),
    (6.0, 28.0),
    (7.0, 34.0),
    (8.0, 31.0),
];

pub const FRAME_SERIES: &[(f64, f64)] = &[
    (0.0, 16.0),
    (1.0, 16.5),
    (2.0, 16.1),
    (3.0, 17.2),
    (4.0, 16.4),
    (5.0, 16.0),
    (6.0, 15.9),
    (7.0, 16.3),
    (8.0, 16.1),
];

pub fn capacity_items() -> [GaugeItem; 4] {
    [
        GaugeItem {
            label: "vfs".into(),
            ratio: 0.21,
            text: "31 / 148 files".into(),
        },
        GaugeItem {
            label: "wasm".into(),
            ratio: 0.63,
            text: "6.3 MB / 10 MB".into(),
        },
        GaugeItem {
            label: "events".into(),
            ratio: 0.78,
            text: "78% queue".into(),
        },
        GaugeItem {
            label: "layout".into(),
            ratio: 0.94,
            text: "94% stress".into(),
        },
    ]
}
