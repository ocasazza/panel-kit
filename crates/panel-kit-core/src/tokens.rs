//! Re-export of the canonical palette seed.
//!
//! The editable source of the palette — and of the full dark/paper
//! [`ThemeTokens`](crate::theme::ThemeTokens) presets derived from it — is
//! [`crate::theme`]. This module keeps the historical
//! `panel_kit_core::tokens` path working for its existing consumers
//! (`build.rs`, the web stylesheet pinning tests, the terminal preset); the
//! composable-library refactor regenerates those consumers from `theme`
//! directly, and then this shim goes away.

pub use crate::theme::{
    by_name, Token, ACCENT, BADGE_INFO, BG, BLUE, DARK, DIM, FG, GREEN, INV_BG, INV_FG, LINE,
    LINE2, MONO, PANEL, PINK, RED, YELLOW,
};
