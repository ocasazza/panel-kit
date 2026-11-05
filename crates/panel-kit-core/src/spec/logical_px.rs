//! A cell-authored workspace expressed in a pixel host's logical pixels.

use crate::frame::ChromeProjectionInput;
use crate::reducer::Viewport;
use crate::{ChromeMetrics, PanelCommand, Units};

use super::ResolvedWorkspace;

impl ResolvedWorkspace {
    /// Express a [`Units::Cells`] workspace in the logical pixels of a pixel
    /// host whose surface measures `width` × `height`.
    ///
    /// The authored viewport maps onto that surface: one column is
    /// `width / columns` px, one row `height / rows` px. Horizontal lengths
    /// scale by the column, vertical lengths by the row, and lengths used on
    /// both axes (clamp `inner`/`edge`, tile gaps and padding, keyboard steps)
    /// by the column. Chrome is not scaled: a terminal draws it in whole
    /// cells and a pixel host at its own sizes, so chrome metrics become the
    /// pixel chrome profile (`ChromeProjectionInput::full(ChromeMetrics::WEB)`)
    /// while the chrome toggles are kept.
    ///
    /// A CSS-px workspace, or a surface or authored viewport that is not
    /// positive and finite, is returned unchanged.
    #[must_use]
    pub fn into_logical_px(mut self, width: f64, height: f64) -> Self {
        let [columns, rows] = self.layout.viewport;
        let positive = |value: f64| value.is_finite() && value > 0.0;
        if self.layout.units != Units::Cells
            || ![width, height, columns, rows].into_iter().all(positive)
        {
            return self;
        }
        let (col, row) = (width / columns, height / rows);

        self.layout.units = Units::CssPx;
        self.layout.viewport = [width, height];

        let clamp = &mut self.layout.clamp;
        clamp.outer_w *= col;
        clamp.floor_w *= col;
        clamp.min_w *= col;
        clamp.outer_h *= row;
        clamp.floor_h *= row;
        clamp.min_h *= row;
        clamp.inner *= col;
        clamp.edge *= col;

        let tile = &mut self.layout.tile;
        tile.resize.row *= row;
        tile.resize.col_floor *= col;
        tile.resize.outer *= col;
        tile.resize.gap *= col;
        tile.resize.padding *= col;
        tile.row_min *= row;
        tile.gap *= col;
        tile.padding *= col;

        self.surface.compact_max *= col;
        self.surface.tablet_max *= col;

        self.input.steps.coarse *= col;
        self.input.steps.fine *= col;
        for binding in &mut self.input.bindings {
            match &mut binding.command {
                PanelCommand::Move { dx, dy } => {
                    *dx *= col;
                    *dy *= row;
                }
                PanelCommand::Resize { dw, dh } => {
                    *dw *= col;
                    *dh *= row;
                }
                _ => {}
            }
        }

        let pixel_chrome = ChromeProjectionInput::full(ChromeMetrics::WEB);
        self.chrome.metrics = pixel_chrome.metrics;
        self.chrome.hit_target_min = pixel_chrome.hit_target_min;
        self.chrome.panel_header_h = pixel_chrome.panel_header_h;

        for panel in &mut self.panels {
            let window = &mut panel.window;
            window.x *= col;
            window.w *= col;
            window.y *= row;
            window.h *= row;
        }

        let initial = &mut self.initial;
        initial.viewport = Viewport {
            width,
            height,
            units: Units::CssPx,
        };
        for panel in &mut initial.panels {
            panel.x *= col;
            panel.w *= col;
            panel.y *= row;
            panel.h *= row;
        }
        initial.workspace_scroll *= row;

        self
    }
}

#[cfg(all(test, feature = "spec-json"))]
mod tests {
    use super::super::tests::{resolved_from, valid_spec_json};
    use crate::frame::ChromeProjectionInput;
    use crate::{ChromeMetrics, PanelCommand, Units};

    /// The fixture's 120×40-cell viewport onto 1440×800 px: a 12 px column
    /// and a 20 px row.
    #[test]
    fn cell_lengths_scale_by_their_axis_and_chrome_takes_the_pixel_profile() {
        let px = resolved_from(valid_spec_json()).into_logical_px(1440.0, 800.0);

        assert_eq!(px.layout.units, Units::CssPx);
        assert_eq!(px.layout.viewport, [1440.0, 800.0]);
        assert_eq!(
            (px.initial.viewport.width, px.initial.viewport.height, px.initial.viewport.units),
            (1440.0, 800.0, Units::CssPx)
        );
        // Notes window: x 2, y 2, w 36, h 12 cells.
        let notes = px.initial.panels[0];
        assert_eq!((notes.x, notes.y, notes.w, notes.h), (24.0, 40.0, 432.0, 240.0));
        let window = &px.panels[0].window;
        assert_eq!((window.x, window.y, window.w, window.h), (24.0, 40.0, 432.0, 240.0));

        // Clamp::CELLS: floor 24×8, min 20×5, inner 2, edge 0.
        let clamp = px.layout.clamp;
        assert_eq!((clamp.floor_w, clamp.floor_h), (288.0, 160.0));
        assert_eq!((clamp.min_w, clamp.min_h), (240.0, 100.0));
        assert_eq!((clamp.inner, clamp.edge, clamp.max_frac), (24.0, 0.0, 1.0));

        // TileMetrics::CELLS row 4 / col_floor 12; tile row_min 1, gap 1.
        let tile = &px.layout.tile;
        assert_eq!((tile.resize.row, tile.resize.col_floor), (80.0, 144.0));
        assert_eq!((tile.row_min, tile.gap, tile.padding), (20.0, 12.0, 0.0));

        assert_eq!((px.surface.compact_max, px.surface.tablet_max), (720.0, 1320.0));
        // CommandStep::CELLS 2 / 1; the authored binding moves dx -16 cells.
        assert_eq!((px.input.steps.coarse, px.input.steps.fine), (24.0, 12.0));
        assert_eq!(
            px.input.bindings[0].command,
            PanelCommand::Move { dx: -192.0, dy: 0.0 }
        );

        let pixel = ChromeProjectionInput::full(ChromeMetrics::WEB);
        assert_eq!(px.chrome.metrics, ChromeMetrics::WEB);
        assert_eq!(px.chrome.hit_target_min, pixel.hit_target_min);
        assert_eq!(px.chrome.panel_header_h, pixel.panel_header_h);
        assert!(px.chrome.title_in_border && px.chrome.dock);
    }

    #[test]
    fn css_px_workspaces_and_unusable_surfaces_pass_through() {
        let mut css = valid_spec_json();
        css["layout"]["units"] = "CssPx".into();
        let authored = resolved_from(css.clone());
        let converted = resolved_from(css).into_logical_px(1440.0, 800.0);
        assert_eq!(converted.layout, authored.layout);
        assert!(converted.initial == authored.initial);

        let cells = resolved_from(valid_spec_json());
        for (width, height) in [(0.0, 800.0), (1440.0, f64::NAN), (-1.0, 800.0)] {
            let kept = resolved_from(valid_spec_json()).into_logical_px(width, height);
            assert_eq!(kept.layout, cells.layout, "{width}×{height}");
            assert!(kept.initial == cells.initial, "{width}×{height}");
        }
    }
}
