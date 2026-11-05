//! Panel container class and placement style helpers.

use panel_kit_core::frame::{PanelProjection, Placement};
use panel_kit_core::tile_drag::{lift_opacity, lift_scale, TileRole};
use panel_kit_core::PanelKey;

use super::root::css_px;

/// Inline placement style for one projected panel.
///
/// A tiled panel keeps its committed grid area. While a tile drag moves it
/// off that slot, a transform carries it to the projected `region` (and an
/// explicit size when the region's size differs), and an elevated tile gets
/// its lift scale, opacity, and `--pk-lift` for the stylesheet's shadow.
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
        } => {
            let mut style = format!(
                "grid-column:{} / span {};grid-row:{} / span {};",
                u16::from(column) + 1,
                column_span,
                row + 1,
                row_span,
            );
            push_tile_drag_style(&mut style, panel);
            style
        }
        Placement::Maximized => "position:absolute;inset:0;".to_string(),
    }
}

fn push_tile_drag_style<K: PanelKey>(style: &mut String, panel: PanelProjection<K>) {
    let (region, slot) = (panel.region, panel.slot);
    if region == slot && panel.lift <= 0.0 {
        return;
    }
    if region.w != slot.w || region.h != slot.h {
        style.push_str(&format!(
            "width:{};height:{};",
            css_px(region.w),
            css_px(region.h)
        ));
    }
    style.push_str(&format!(
        "transform:translate({}, {}) scale({});",
        css_px(region.x - slot.x),
        css_px(region.y - slot.y),
        lift_scale(panel.lift),
    ));
    if panel.tile_role.is_elevated() {
        style.push_str(&format!(
            "opacity:{};--pk-lift:{};",
            lift_opacity(panel.lift),
            panel.lift
        ));
    }
}

/// CSS class list for a projected panel: `panel`, an optional host class,
/// and the drag/focus state modifiers.
pub fn panel_class<K: PanelKey>(panel: PanelProjection<K>, class: Option<&str>) -> String {
    let extra = class.filter(|extra| !extra.is_empty()).unwrap_or_default();
    let tile_role = match panel.tile_role {
        TileRole::Resting => "",
        TileRole::Held => " tile-held",
        TileRole::Lifted => " tile-lifted",
        TileRole::Landing => " tile-landing",
        TileRole::Target => " tile-target",
    };
    let pointer_dragging = if panel.pointer_dragging {
        " dragging"
    } else {
        ""
    };
    let focused = if panel.focused { " focused" } else { "" };

    if extra.is_empty() {
        return format!("panel{tile_role}{pointer_dragging}{focused}");
    }

    format!("panel {extra}{tile_role}{pointer_dragging}{focused}")
}
