//! Pure web spec-plan lowering for native parity checks and web painters.
//!
//! This module deliberately contains no DOM, Dioxus, `web-sys`, or browser
//! storage types. It owns only scalar/core configuration lowered from a strict
//! [`ResolvedWorkspace`], so the native checker can link the root crate with
//! `default-features = false`.

use panel_kit_core::spec::{
    Charset, ChromeSpec, InputSpec, LayoutSpec, PanelSpec, PersistenceSpec, ResolvedWorkspace,
    SurfaceSpec,
};
use panel_kit_core::theme::ThemeTokens;
use serde::Serialize;

/// Concrete production plan consumed by the Dioxus web backend.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct WebSpecPlan {
    /// Spec format version.
    pub spec_version: u32,
    /// Stable workspace identifier.
    pub id: String,
    /// Initial layout policy and geometry.
    pub layout: LayoutSpec,
    /// Surface classification fallback policy.
    pub surface: SurfaceSpec,
    /// Web chrome metrics and feature toggles.
    pub chrome: ChromeSpec,
    /// Keyboard command configuration.
    pub input: InputSpec,
    /// Requested glyph family; DOM rendering has no glyph remapping but keeps the leaf covered.
    pub glyphs: Charset,
    /// Theme tokens applied as CSS variables by the web backend.
    pub theme: ThemeTokens,
    /// Persistence policy the host maps to a concrete web store.
    pub persistence: PersistenceSpec,
    /// Authored panels in declaration order.
    pub panels: Vec<PanelSpec>,
}

/// Lower a resolved strict workspace into the owned web production plan.
pub fn lower_spec(workspace: &ResolvedWorkspace) -> WebSpecPlan {
    WebSpecPlan {
        spec_version: workspace.spec_version,
        id: workspace.id.clone(),
        layout: workspace.layout.clone(),
        surface: workspace.surface.clone(),
        chrome: workspace.chrome.clone(),
        input: workspace.input.clone(),
        glyphs: workspace.glyphs,
        theme: workspace.theme.clone(),
        persistence: workspace.persistence.clone(),
        panels: workspace.panels.clone(),
    }
}
