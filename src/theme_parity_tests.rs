mod assertions;
mod parsing;

use assertions::assert_generated_consumers_match_core;
use panel_kit_core::theme::ThemeTokens;
use parsing::extract_generated_region;

use crate::theme;

#[test]
fn dark_theme_emits_every_web_and_tui_token() {
    assert_generated_consumers_match_core("dark", ThemeTokens::dark());
}

#[test]
fn theme_parity_covers_generated_consumers_for_dark_and_paper() {
    assert_generated_consumers_match_core("dark", ThemeTokens::dark());
    assert_generated_consumers_match_core("paper", ThemeTokens::paper());
}

#[test]
fn checked_in_theme_regions_are_generated_from_core() {
    assert_eq!(
        extract_generated_region(
            super::CSS,
            theme::CSS_THEME_REGION_BEGIN,
            theme::CSS_THEME_REGION_END,
        ),
        theme::css_root_block(&ThemeTokens::dark()),
        "assets/panel-kit.css must be regenerated from ThemeTokens::dark()",
    );

    assert_eq!(
        extract_generated_region(
            include_str!("../DESIGN.md"),
            theme::DESIGN_THEME_REGION_BEGIN,
            theme::DESIGN_THEME_REGION_END,
        ),
        theme::design_token_region(&ThemeTokens::dark()),
        "DESIGN.md token frontmatter must be regenerated from ThemeTokens::dark()",
    );
}
