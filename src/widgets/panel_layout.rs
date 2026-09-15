//! Panel container class and placement style helpers.

use panel_kit_core::frame::{PanelProjection, Placement};
use panel_kit_core::PanelKey;

use super::root::css_px;

/// Inline placement style for one projected panel.
pub fn panel_style<K: PanelKey>(panel: PanelProjection<K>) -> String {
    match panel.placement {
        Placement::Floating => format!(
            "position:absolute;left:{};top:{};width:{};height:{};z-index:{};",
            css_px(panel.region.x),
            css_px(panel.region.y),
            css_px(panel.region.w),
            css_px(panel.region.h),
            panel.z,
        ),
        Placement::Tiled {
            column,
            row,
            column_span,
            row_span,
        } => format!(
            "grid-column:{} / span {};grid-row:{} / span {};",
            u16::from(column) + 1,
            column_span,
            row + 1,
            row_span,
        ),
        Placement::Maximized => "position:absolute;inset:0;".to_string(),
    }
}

pub(super) fn panel_class<K: PanelKey>(panel: PanelProjection<K>, class: Option<&str>) -> String {
    let extra = class.filter(|extra| !extra.is_empty()).unwrap_or_default();
    let tile_dragging = if panel.tile_dragging { " tile-dragging" } else { "" };
    let pointer_dragging = if panel.pointer_dragging { " dragging" } else { "" };
    let focused = if panel.focused { " focused" } else { "" };

    if extra.is_empty() {
        return format!("panel{tile_dragging}{pointer_dragging}{focused}");
    }

    format!("panel {extra}{tile_dragging}{pointer_dragging}{focused}")
}
