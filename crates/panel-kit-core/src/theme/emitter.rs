use super::{ThemeColor, ThemeTokens};

/// Begin marker for generated theme variables in the committed stylesheet.
pub const CSS_THEME_REGION_BEGIN: &str = "/* BEGIN GENERATED THEME TOKENS */";
/// End marker for generated theme variables in the committed stylesheet.
pub const CSS_THEME_REGION_END: &str = "/* END GENERATED THEME TOKENS */";
/// Begin marker for generated theme values in the design-system frontmatter.
pub const DESIGN_THEME_REGION_BEGIN: &str = "# BEGIN GENERATED THEME TOKENS";
/// End marker for generated theme values in the design-system frontmatter.
pub const DESIGN_THEME_REGION_END: &str = "# END GENERATED THEME TOKENS";

/// Render the web stylesheet's default `:root` theme variables.
pub fn css_root_block(theme: &ThemeTokens) -> String {
    let mut css = String::from(":root {\n");
    for slot in ThemeColor::ALL.iter().copied() {
        css.push_str(&format!(
            "  --{}: {};\n",
            slot.key(),
            String::from(theme.colors.resolve(slot))
        ));
    }

    css.push_str(&format!("  --mono: {};\n", theme.typography.family));
    css.push_str(&format!(
        "  --body-size: {};\n",
        px(theme.typography.body_size)
    ));
    css.push_str(&format!(
        "  --body-line-height: {};\n",
        number(theme.typography.body_line_height)
    ));
    css.push_str(&format!(
        "  --label-size: {}rem;\n",
        number(theme.typography.label_size)
    ));
    css.push_str(&format!(
        "  --label-weight: {};\n",
        theme.typography.label_weight
    ));
    css.push_str(&format!(
        "  --label-tracking: {}em;\n",
        number(theme.typography.label_tracking)
    ));
    css.push_str(&format!(
        "  --panel-radius: {};\n",
        px(theme.density.panel_radius)
    ));
    css.push_str(&format!(
        "  --badge-radius: {};\n",
        px(theme.density.badge_radius)
    ));
    css.push_str(&format!(
        "  --space-xs: {};\n",
        px(theme.density.spacing_xs)
    ));
    css.push_str(&format!(
        "  --space-sm: {};\n",
        px(theme.density.spacing_sm)
    ));
    css.push_str(&format!(
        "  --space-md: {};\n",
        px(theme.density.spacing_md)
    ));
    css.push('}');
    css
}

/// Render the generated value block inside `DESIGN.md` frontmatter.
pub fn design_token_region(theme: &ThemeTokens) -> String {
    format!(
        "colors:\n{}typography:\n{}rounded:\n{}spacing:\n{}",
        design_colors(theme),
        design_typography(theme),
        design_rounded(theme),
        design_spacing(theme),
    )
}

/// Replace a marker-delimited generated region with freshly rendered content.
pub fn replace_generated_region(document: &str, begin: &str, end: &str, generated: &str) -> String {
    let start = document
        .find(begin)
        .unwrap_or_else(|| panic!("missing generated-region begin marker {begin:?}"));
    let after_begin = start + begin.len();
    let end_offset = document[after_begin..]
        .find(end)
        .unwrap_or_else(|| panic!("missing generated-region end marker {end:?}"));
    let end_start = after_begin + end_offset;

    format!(
        "{}\n{}\n{}",
        &document[..after_begin],
        generated.trim_matches('\n'),
        document[end_start..].trim_start_matches('\n')
    )
}

fn design_colors(theme: &ThemeTokens) -> String {
    let mut colors = String::new();
    for slot in ThemeColor::ALL.iter().copied() {
        colors.push_str(&format!(
            "  {}: {:?}\n",
            slot.key(),
            String::from(theme.colors.resolve(slot))
        ));
    }
    colors
}

fn design_typography(theme: &ThemeTokens) -> String {
    let family = yaml_string(&theme.typography.family);
    format!(
        "  title:\n    fontFamily: {family}\n    fontSize: \"0.8rem\"\n    fontWeight: 600\n    lineHeight: {}\n    letterSpacing: \"{}em\"\n  label:\n    fontFamily: {family}\n    fontSize: \"{}rem\"\n    fontWeight: {}\n    lineHeight: {}\n    letterSpacing: \"{}em\"\n  body:\n    fontFamily: {family}\n    fontSize: {:?}\n    fontWeight: 400\n    lineHeight: {}\n    letterSpacing: \"normal\"\n  caption:\n    fontFamily: {family}\n    fontSize: \"{}rem\"\n    fontWeight: 400\n    lineHeight: 1.3\n    letterSpacing: \"0.02em\"\n  micro:\n    fontFamily: {family}\n    fontSize: \"0.65rem\"\n    fontWeight: 400\n    lineHeight: {}\n    letterSpacing: \"normal\"\n",
        number(theme.typography.body_line_height),
        number(theme.typography.label_tracking),
        number(theme.typography.label_size),
        theme.typography.label_weight,
        number(theme.typography.body_line_height),
        number(theme.typography.label_tracking),
        px(theme.typography.body_size),
        number(theme.typography.body_line_height),
        number(theme.typography.label_size),
        number(theme.typography.body_line_height),
    )
}

fn design_rounded(theme: &ThemeTokens) -> String {
    format!(
        "  chip: \"{}\"\n  panel: \"{}\"\n  overlay: \"5px\"\n  pill: \"{}\"\n  dot: \"50%\"\n",
        px(theme.density.badge_radius.min(3.0)),
        px(theme.density.panel_radius),
        px(theme.density.badge_radius),
    )
}

fn design_spacing(theme: &ThemeTokens) -> String {
    format!(
        "  hair: \"2px\"\n  xs: \"{}\"\n  sm: \"{}\"\n  md: \"{}\"\n  lg: \"12px\"\n  xl: \"16px\"",
        px(theme.density.spacing_xs),
        px(theme.density.spacing_sm),
        px(theme.density.spacing_md),
    )
}

fn yaml_string(value: &str) -> String {
    format!("{value:?}")
}

fn px(value: f64) -> String {
    format!("{}px", number(value))
}

fn number(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{value:.0}")
    } else {
        let mut text = format!("{value:.4}");
        while text.ends_with('0') {
            text.pop();
        }
        text
    }
}
