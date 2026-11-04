//! Gate: the panel chrome geometry in `panel-kit.css` is the geometry core's
//! `PanelPaint` hands to native pixel backends.

use panel_kit_core::frame::ChromeProjectionInput;
use panel_kit_core::theme::{
    PanelPaint, ThemeTokens, GRIP_PX, LIGHT_GAP_PX, LIGHT_PX, PANEL_BORDER_PX,
};
use panel_kit_core::ChromeMetrics;

use super::parsing::px;

use crate::CSS;

/// Declarations of the first rule whose selector line is exactly `selector`.
fn rule(selector: &str) -> String {
    let start = CSS
        .find(&format!("\n{selector} {{"))
        .unwrap_or_else(|| panic!("panel-kit.css has no `{selector}` rule"));
    let body = &CSS[start..];
    let open = body.find('{').expect("rule opens") + 1;
    let close = body.find('}').expect("rule closes");
    body[open..close].split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The value of `property` in `declarations`.
fn decl(declarations: &str, property: &str) -> String {
    declarations
        .split(';')
        .filter_map(|decl| decl.split_once(':'))
        .find(|(name, _)| name.trim() == property)
        .map(|(_, value)| value.trim().to_owned())
        .unwrap_or_else(|| panic!("no `{property}` in `{declarations}`"))
}

#[test]
fn css_chrome_literals_are_core_panel_paint() {
    assert_eq!(decl(&rule(".light"), "width"), px(LIGHT_PX));
    assert_eq!(decl(&rule(".light"), "height"), px(LIGHT_PX));
    assert_eq!(decl(&rule(".lights"), "gap"), px(LIGHT_GAP_PX));
    assert_eq!(decl(&rule(".resize::after"), "width"), px(GRIP_PX));
    assert_eq!(decl(&rule(".resize::after"), "height"), px(GRIP_PX));
    assert!(decl(&rule(".panel"), "border").starts_with(&px(PANEL_BORDER_PX)));
    let head_h = CSS
        .split_once("--panel-head-h:")
        .and_then(|(_, rest)| rest.split(';').next())
        .expect("panel-kit.css declares --panel-head-h");
    assert_eq!(
        head_h.trim(),
        px(ChromeProjectionInput::full(ChromeMetrics::WEB).panel_header_h)
    );
}

/// The header, title gap and body padding are the token expressions
/// `PanelPaint::css_px` evaluates.
#[test]
fn css_chrome_spacing_is_the_panel_paint_expression() {
    let head = rule(".panel-head");
    assert_eq!(decl(&head, "left"), "calc(var(--space-md) - var(--space-xs))");
    assert_eq!(decl(&head, "right"), "calc(var(--space-md) - var(--space-xs))");
    assert_eq!(decl(&head, "top"), format!("-{}", px(PANEL_BORDER_PX)));
    assert_eq!(decl(&head, "height"), "var(--panel-head-h)");
    assert_eq!(decl(&head, "padding"), "0 var(--space-xs)");
    assert_eq!(decl(&head, "gap"), ".5rem");
    assert_eq!(
        decl(&rule(".panel-body"), "padding"),
        "calc(var(--panel-head-h) + var(--space-sm)) var(--space-md) var(--space-md)"
    );

    let tokens = ThemeTokens::dark();
    let paint = PanelPaint::css_px(&tokens);
    let density = tokens.density;
    assert_eq!(paint.head_inset, density.spacing_md - density.spacing_xs);
    assert_eq!(paint.head_pad, density.spacing_xs);
    assert_eq!(paint.title_gap, 0.5 * tokens.typography.body_size);
    assert_eq!(paint.body_pad_top, paint.head_h + density.spacing_sm);
    assert_eq!(paint.body_pad_x, density.spacing_md);
}
