//! Pure TUI spec-plan lowering for parity checks and ratatui painters.
//!
//! The plan is native-Rust data: shared core configuration plus the concrete
//! ratatui theme conversion. Typography and density remain explicit
//! approximations in [`ResolvedTuiTheme`], matching terminal host limits.

use panel_kit_core::spec::{
    Charset, ChromeSpec, InputSpec, LayoutSpec, PanelSpec, PersistenceSpec, ResolvedWorkspace,
    SurfaceSpec,
};
use panel_kit_core::theme::ThemeTokens;

use crate::theme::ResolvedTuiTheme;

/// Concrete production plan consumed by the ratatui backend.
#[derive(Clone, Debug, PartialEq)]
pub struct TuiSpecPlan {
    /// Spec format version.
    pub spec_version: u32,
    /// Stable workspace identifier.
    pub id: String,
    /// Initial layout policy and geometry.
    pub layout: LayoutSpec,
    /// Surface classification fallback policy.
    pub surface: SurfaceSpec,
    /// TUI chrome metrics and feature toggles.
    pub chrome: ChromeSpec,
    /// Keyboard command configuration.
    pub input: InputSpec,
    /// Requested glyph family.
    pub glyphs: Charset,
    /// Source theme tokens retained for parity diagnostics.
    pub theme: ThemeTokens,
    /// Concrete ratatui theme conversion consumed by painters.
    pub resolved_theme: ResolvedTuiTheme,
    /// Persistence policy the host maps to a concrete TUI store.
    pub persistence: PersistenceSpec,
    /// Authored panels in declaration order.
    pub panels: Vec<PanelSpec>,
}

/// Lower a resolved strict workspace into the owned TUI production plan.
pub fn lower_spec(workspace: &ResolvedWorkspace) -> TuiSpecPlan {
    TuiSpecPlan {
        spec_version: workspace.spec_version,
        id: workspace.id.clone(),
        layout: workspace.layout.clone(),
        surface: workspace.surface.clone(),
        chrome: workspace.chrome.clone(),
        input: workspace.input.clone(),
        glyphs: workspace.glyphs,
        theme: workspace.theme.clone(),
        resolved_theme: ResolvedTuiTheme::from(&workspace.theme),
        persistence: workspace.persistence.clone(),
        panels: workspace.panels.clone(),
    }
}
