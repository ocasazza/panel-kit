use super::ThemeTokens;
use crate::frame::ChromeProjectionInput;
use crate::ChromeMetrics;

/// Side of a traffic-light square, in CSS pixels (`.light`).
pub const LIGHT_PX: f64 = 12.0;
/// Gap between traffic lights, in CSS pixels (`.lights`).
pub const LIGHT_GAP_PX: f64 = 6.0;
/// Side of the resize-grip triangle, in CSS pixels (`.resize::after`).
pub const GRIP_PX: f64 = 14.0;
/// Panel outline width, in CSS pixels (`.panel`).
pub const PANEL_BORDER_PX: f64 = 1.0;

/// Pixel geometry of the panel chrome a pixel renderer paints.
///
/// The web stylesheet and every native pixel backend draw the same frame:
/// a [`PANEL_BORDER_PX`] outline, a header band of the projected header height
/// inset into the top of the frame, the traffic lights, the title, a padded
/// body, and a resize grip. Spacing derives from [`ThemeTokens::density`] and
/// the title gap from the root type size, exactly as `assets/panel-kit.css`
/// derives them from `--space-*` and `rem`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PanelPaint {
    /// Frame outline width.
    pub border: f64,
    /// Header band height (`--panel-head-h`), the projected header height.
    pub head_h: f64,
    /// Header inset from each side of the frame's padding box.
    pub head_inset: f64,
    /// Header horizontal padding; the lights pull back across it.
    pub head_pad: f64,
    /// Traffic-light side.
    pub light: f64,
    /// Gap between traffic lights.
    pub light_gap: f64,
    /// Gap between the light cluster and the title (`.5rem`).
    pub title_gap: f64,
    /// Body top padding: clears the header band.
    pub body_pad_top: f64,
    /// Body left and right padding.
    pub body_pad_x: f64,
    /// Body bottom padding.
    pub body_pad_bottom: f64,
    /// Resize-grip triangle side.
    pub grip: f64,
}

impl PanelPaint {
    /// Chrome geometry in CSS pixels for `tokens`.
    pub fn css_px(tokens: &ThemeTokens) -> Self {
        let density = tokens.density;
        let head_h = ChromeProjectionInput::full(ChromeMetrics::WEB).panel_header_h;
        Self {
            border: PANEL_BORDER_PX,
            head_h,
            head_inset: density.spacing_md - density.spacing_xs,
            head_pad: density.spacing_xs,
            light: LIGHT_PX,
            light_gap: LIGHT_GAP_PX,
            title_gap: 0.5 * tokens.typography.body_size,
            body_pad_top: head_h + density.spacing_sm,
            body_pad_x: density.spacing_md,
            body_pad_bottom: density.spacing_md,
            grip: GRIP_PX,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spacing_follows_density_and_root_size() {
        let mut tokens = ThemeTokens::dark();
        tokens.density.spacing_xs = 3.0;
        tokens.density.spacing_sm = 5.0;
        tokens.density.spacing_md = 10.0;
        tokens.typography.body_size = 16.0;
        let paint = PanelPaint::css_px(&tokens);
        assert_eq!(paint.head_inset, 7.0);
        assert_eq!(paint.head_pad, 3.0);
        assert_eq!(paint.title_gap, 8.0);
        assert_eq!(paint.body_pad_top, paint.head_h + 5.0);
        assert_eq!(paint.body_pad_x, 10.0);
        assert_eq!(paint.body_pad_bottom, 10.0);
    }
}
