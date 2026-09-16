//! One theme source: the single editable home for the palette (dark and
//! paper), typography, and density (design §8.1).
//!
//! The colour values previously lived in [`crate::tokens`] as a flat seed —
//! and before that in three hand-maintained copies (the web `:root` block,
//! the terminal `Theme`, and the boot stylesheet) whose discipline had
//! already failed once. This module keeps that story and finishes it:
//!
//! - the flat `Token` seed constants live here, and
//!   [`tokens`](crate::tokens) re-exports them for existing boot-stylesheet
//!   substitution code;
//! - [`ThemeTokens::dark`] derives from that seed, so the flat constants
//!   and the structured token set cannot drift apart;
//! - [`ThemeTokens::paper`] seeds the light preset in core for the first
//!   time — until now the terminal crate held the only copy;
//! - `focus_ring` is a first-class slot even when a backend paints it with
//!   the same colour as foreground text;
//! - the web `:root` block, terminal palette, and `DESIGN.md` token region
//!   are generated consumers of the emitters in this module.

mod color;
mod emitter;
mod presets;
mod schema;
mod semantic;
mod tokens;

pub use color::{Color, ColorParseError};
pub use emitter::{
    css_root_block, design_token_region, replace_generated_region, CSS_THEME_REGION_BEGIN,
    CSS_THEME_REGION_END, DESIGN_THEME_REGION_BEGIN, DESIGN_THEME_REGION_END,
};
pub use schema::{ColorTokens, DensityTokens, ThemeTokens, TypographyTokens};
pub use semantic::{ColorRef, ThemeColor};
pub use tokens::{
    by_name, Token, ACCENT, BADGE_INFO, BG, BLUE, DARK, DIM, FG, GREEN, INV_BG, INV_FG, LINE,
    LINE2, MONO, PANEL, PINK, RED, YELLOW,
};

#[cfg(test)]
mod tests;
