//! Explicit TUI dispositions for non-colour theme tokens.

const HOST_FONT_REASON: &str = "terminal host controls the active monospace font";
const CELL_SIZE_REASON: &str = "terminal cell metrics control rendered text size";
const ROW_HEIGHT_REASON: &str = "terminal row height controls text line height";
const WEIGHT_REASON: &str = "terminal styling cannot represent CSS numeric font weights";
const TRACKING_REASON: &str = "terminal grids use fixed cell spacing";
const PANEL_RADIUS_REASON: &str = "ratatui borders use box glyphs instead of pixel radii";
const BADGE_RADIUS_REASON: &str = "terminal badges render as text spans, not rounded boxes";
const CELL_SPACING_REASON: &str = "terminal layouts apply spacing in whole cells";

/// Whether a non-colour theme token is applied exactly or approximated.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TuiThemeDisposition {
    /// The token is directly represented by ratatui output.
    Applied,
    /// The terminal backend cannot express the token exactly; the reason names
    /// the native terminal limitation instead of silently omitting the field.
    Approximated(&'static str),
}

/// TUI dispositions for every [`panel_kit_core::theme::TypographyTokens`] field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TuiTypographyTheme {
    /// Font family disposition.
    pub family: TuiThemeDisposition,
    /// Body font-size disposition.
    pub body_size: TuiThemeDisposition,
    /// Body line-height disposition.
    pub body_line_height: TuiThemeDisposition,
    /// Label font-size disposition.
    pub label_size: TuiThemeDisposition,
    /// Label font-weight disposition.
    pub label_weight: TuiThemeDisposition,
    /// Label letter-spacing disposition.
    pub label_tracking: TuiThemeDisposition,
}

/// TUI dispositions for every [`panel_kit_core::theme::DensityTokens`] field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TuiDensityTheme {
    /// Panel corner-radius disposition.
    pub panel_radius: TuiThemeDisposition,
    /// Badge corner-radius disposition.
    pub badge_radius: TuiThemeDisposition,
    /// Extra-small spacing disposition.
    pub spacing_xs: TuiThemeDisposition,
    /// Small spacing disposition.
    pub spacing_sm: TuiThemeDisposition,
    /// Medium spacing disposition.
    pub spacing_md: TuiThemeDisposition,
}

impl TuiThemeDisposition {
    /// Construct an explicit approximation with a non-empty reason.
    pub const fn approximated(reason: &'static str) -> Self {
        Self::Approximated(reason)
    }
}

impl TuiTypographyTheme {
    /// Mark every typography token as approximated by terminal font/cell rules.
    pub const fn approximated() -> Self {
        Self {
            family: TuiThemeDisposition::approximated(HOST_FONT_REASON),
            body_size: TuiThemeDisposition::approximated(CELL_SIZE_REASON),
            body_line_height: TuiThemeDisposition::approximated(ROW_HEIGHT_REASON),
            label_size: TuiThemeDisposition::approximated(CELL_SIZE_REASON),
            label_weight: TuiThemeDisposition::approximated(WEIGHT_REASON),
            label_tracking: TuiThemeDisposition::approximated(TRACKING_REASON),
        }
    }
}

impl TuiDensityTheme {
    /// Mark every density token as approximated by terminal cell geometry.
    pub const fn approximated() -> Self {
        Self {
            panel_radius: TuiThemeDisposition::approximated(PANEL_RADIUS_REASON),
            badge_radius: TuiThemeDisposition::approximated(BADGE_RADIUS_REASON),
            spacing_xs: TuiThemeDisposition::approximated(CELL_SPACING_REASON),
            spacing_sm: TuiThemeDisposition::approximated(CELL_SPACING_REASON),
            spacing_md: TuiThemeDisposition::approximated(CELL_SPACING_REASON),
        }
    }
}
