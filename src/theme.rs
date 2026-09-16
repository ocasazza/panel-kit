//! Web theme emitter for generated CSS custom properties.
//!
//! Runtime applications can call [`css_custom_properties`] when they need the
//! same variables the bundled stylesheet receives at build time. The actual
//! formatting lives in `panel-kit-core` so the build script and runtime emitter
//! cannot drift.

use panel_kit_core::theme::{self as core_theme, ThemeTokens};
pub use panel_kit_core::theme::{
    CSS_THEME_REGION_BEGIN, CSS_THEME_REGION_END, DESIGN_THEME_REGION_BEGIN,
    DESIGN_THEME_REGION_END,
};

/// Render `ThemeTokens` as a complete `:root` custom-property block.
pub fn css_root_block(tokens: &ThemeTokens) -> String {
    core_theme::css_root_block(tokens)
}

/// Render `ThemeTokens` as custom properties for one-time stylesheet injection.
pub fn css_custom_properties(tokens: &ThemeTokens) -> String {
    core_theme::css_root_block(tokens)
}

/// Render the `DESIGN.md` frontmatter token region for parity checks.
pub fn design_token_region(tokens: &ThemeTokens) -> String {
    core_theme::design_token_region(tokens)
}
