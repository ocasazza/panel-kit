use std::collections::BTreeMap;

use panel_kit_core::theme::{DensityTokens, ThemeColor, ThemeTokens, TypographyTokens};
use panel_kit_tui::{ResolvedTuiTheme, TuiThemeDisposition};

use crate::theme;

use super::parsing::{em, number, parse_css_custom_properties, parse_design_token_region, px, rem};

pub(super) fn assert_generated_consumers_match_core(name: &str, tokens: ThemeTokens) {
    let css_values = parse_css_custom_properties(&theme::css_root_block(&tokens));
    let design_values = parse_design_token_region(&theme::design_token_region(&tokens));
    let tui = ResolvedTuiTheme::from(&tokens);

    assert_color_tokens(name, &tokens, &css_values, &design_values, &tui);
    assert_typography_tokens(name, &tokens.typography, &css_values, &design_values, &tui);
    assert_density_tokens(name, &tokens.density, &css_values, &design_values, &tui);
}

fn assert_color_tokens(
    name: &str,
    tokens: &ThemeTokens,
    css_values: &BTreeMap<String, String>,
    design_values: &BTreeMap<String, String>,
    tui: &ResolvedTuiTheme,
) {
    for slot in ThemeColor::ALL.iter().copied() {
        let expected = tokens.colors.resolve(slot);
        let key = slot.key();

        assert_eq!(
            css_values.get(key),
            Some(&String::from(expected)),
            "{name} CSS variable --{key} drifted from core"
        );
        assert_eq!(
            design_values.get(&format!("colors.{key}")),
            Some(&String::from(expected)),
            "{name} DESIGN.md color {key} drifted from core"
        );
        assert_eq!(
            tui.resolve(slot),
            expected,
            "{name} TUI color {key} drifted from core"
        );
    }
}

fn assert_typography_tokens(
    name: &str,
    typography: &TypographyTokens,
    css_values: &BTreeMap<String, String>,
    design_values: &BTreeMap<String, String>,
    tui: &ResolvedTuiTheme,
) {
    assert_css_typography(typography, css_values);
    assert_design_typography(name, typography, design_values);
    assert_tui_typography(name, tui);
}

fn assert_css_typography(
    typography: &TypographyTokens,
    css_values: &BTreeMap<String, String>,
) {
    assert_eq!(css_values.get("mono"), Some(&typography.family));
    assert_eq!(css_values.get("body-size"), Some(&px(typography.body_size)));
    assert_eq!(css_values.get("body-line-height"), Some(&number(typography.body_line_height)));
    assert_eq!(css_values.get("label-size"), Some(&rem(typography.label_size)));
    assert_eq!(
        css_values.get("label-weight"),
        Some(&typography.label_weight.to_string())
    );
    assert_eq!(
        css_values.get("label-tracking"),
        Some(&em(typography.label_tracking))
    );
}

fn assert_design_typography(
    name: &str,
    typography: &TypographyTokens,
    design_values: &BTreeMap<String, String>,
) {
    for role in ["title", "label", "body", "caption", "micro"] {
        assert_eq!(
            design_values.get(&format!("typography.{role}.fontFamily")),
            Some(&typography.family),
            "{name} DESIGN.md {role} font family drifted from core"
        );
    }

    let body_size = px(typography.body_size);
    let label_weight = typography.label_weight.to_string();
    let line_height = number(typography.body_line_height);
    let label_size = rem(typography.label_size);
    let label_tracking = em(typography.label_tracking);
    for (path, expected) in [
        ("typography.title.lineHeight", line_height.as_str()),
        ("typography.title.letterSpacing", label_tracking.as_str()),
        ("typography.label.fontSize", label_size.as_str()),
        ("typography.label.fontWeight", label_weight.as_str()),
        ("typography.label.lineHeight", line_height.as_str()),
        ("typography.label.letterSpacing", label_tracking.as_str()),
        ("typography.body.fontSize", body_size.as_str()),
        ("typography.body.lineHeight", line_height.as_str()),
        ("typography.caption.fontSize", label_size.as_str()),
        ("typography.micro.lineHeight", line_height.as_str()),
    ] {
        assert_eq!(
            design_values.get(path).map(String::as_str),
            Some(expected),
            "{name} DESIGN.md {path} drifted from core"
        );
    }
}

fn assert_tui_typography(name: &str, tui: &ResolvedTuiTheme) {
    assert_tui_approximates(name, "typography.family", tui.typography.family);
    assert_tui_approximates(name, "typography.body_size", tui.typography.body_size);
    assert_tui_approximates(
        name,
        "typography.body_line_height",
        tui.typography.body_line_height,
    );
    assert_tui_approximates(name, "typography.label_size", tui.typography.label_size);
    assert_tui_approximates(name, "typography.label_weight", tui.typography.label_weight);
    assert_tui_approximates(
        name,
        "typography.label_tracking",
        tui.typography.label_tracking,
    );
}

fn assert_density_tokens(
    name: &str,
    density: &DensityTokens,
    css_values: &BTreeMap<String, String>,
    design_values: &BTreeMap<String, String>,
    tui: &ResolvedTuiTheme,
) {
    assert_css_density(density, css_values);
    assert_design_density(density, design_values);
    assert_tui_density(name, tui);
}

fn assert_css_density(density: &DensityTokens, css_values: &BTreeMap<String, String>) {
    assert_eq!(css_values.get("panel-radius"), Some(&px(density.panel_radius)));
    assert_eq!(css_values.get("badge-radius"), Some(&px(density.badge_radius)));
    assert_eq!(css_values.get("space-xs"), Some(&px(density.spacing_xs)));
    assert_eq!(css_values.get("space-sm"), Some(&px(density.spacing_sm)));
    assert_eq!(css_values.get("space-md"), Some(&px(density.spacing_md)));
}

fn assert_design_density(density: &DensityTokens, design_values: &BTreeMap<String, String>) {
    let chip_radius = px(density.badge_radius.min(3.0));
    assert_eq!(
        design_values.get("rounded.panel"),
        Some(&px(density.panel_radius))
    );
    assert_eq!(design_values.get("rounded.chip"), Some(&chip_radius));
    assert_eq!(
        design_values.get("rounded.pill"),
        Some(&px(density.badge_radius))
    );
    assert_eq!(design_values.get("spacing.xs"), Some(&px(density.spacing_xs)));
    assert_eq!(design_values.get("spacing.sm"), Some(&px(density.spacing_sm)));
    assert_eq!(design_values.get("spacing.md"), Some(&px(density.spacing_md)));
}

fn assert_tui_density(name: &str, tui: &ResolvedTuiTheme) {
    assert_tui_approximates(name, "density.panel_radius", tui.density.panel_radius);
    assert_tui_approximates(name, "density.badge_radius", tui.density.badge_radius);
    assert_tui_approximates(name, "density.spacing_xs", tui.density.spacing_xs);
    assert_tui_approximates(name, "density.spacing_sm", tui.density.spacing_sm);
    assert_tui_approximates(name, "density.spacing_md", tui.density.spacing_md);
}

fn assert_tui_approximates(name: &str, field: &str, disposition: TuiThemeDisposition) {
    match disposition {
        TuiThemeDisposition::Approximated(reason) => assert!(
            !reason.trim().is_empty(),
            "{name} TUI {field} approximation must explain the terminal limitation"
        ),
        TuiThemeDisposition::Applied => {
            panic!("{name} TUI {field} unexpectedly claimed exact application")
        }
    }
}
