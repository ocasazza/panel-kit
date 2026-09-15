//! Terminal rendering of the shared badge model
//! ([`panel_kit_core::badge`]): the same kinds, colors, and click actions
//! the Dioxus shell draws as pill chips, as styled spans.

use panel_kit_core::badge::{display_label, BadgeKind, BadgeSpec};
pub use panel_kit_core::widgets::badge::hue_color;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;

use crate::ResolvedTuiTheme;

/// The chip color for a shared badge spec — the terminal twin of the
/// `badge-<kind>` CSS classes.
pub fn color(spec: &BadgeSpec, t: &ResolvedTuiTheme) -> Color {
    if let Some((r, g, b)) = spec.override_color {
        return Color::Rgb(r, g, b);
    }

    match &spec.kind {
        BadgeKind::Tag => t.badge_info,
        BadgeKind::Doctype | BadgeKind::Author => t.badge_info,
        BadgeKind::Folder => t.dim,
        BadgeKind::Entity { .. } | BadgeKind::Date => t.yellow,
        BadgeKind::Wikilink { resolved: true, .. } => t.badge_info,
        BadgeKind::Wikilink {
            resolved: false, ..
        } => t.red,
        BadgeKind::Url { .. } | BadgeKind::Status => t.green,
        BadgeKind::Generic => t.line2,
    }
}

/// The chip as styled spans: `[label]` in the kind color, reversed when
/// active. Render with a `Paragraph`/`Line` and record the drawn rect for
/// click hit-testing.
pub fn spans(spec: &BadgeSpec, t: &ResolvedTuiTheme) -> Vec<Span<'static>> {
    let badge_color = color(spec, t);
    let mut style = Style::default().fg(badge_color);
    if spec.active {
        style = style.add_modifier(Modifier::REVERSED);
    }

    vec![
        Span::styled("[", Style::default().fg(t.line2)),
        Span::styled(display_label(&spec.kind, &spec.value), style),
        Span::styled("]", Style::default().fg(t.line2)),
    ]
}

/// Rendered width in cells for laying out chip strips / hit rects.
pub fn width(spec: &BadgeSpec) -> u16 {
    display_label(&spec.kind, &spec.value).chars().count() as u16 + 2
}

#[cfg(test)]
mod tests {
    use panel_kit_core::badge::{BadgeAction, BadgeClickKind};
    use panel_kit_core::theme::ThemeTokens;
    use ratatui::style::{Color, Modifier};

    use super::*;

    #[test]
    fn tui_badge_paints_core_badge_spec_without_local_fields() {
        let theme = ResolvedTuiTheme::from(&ThemeTokens::dark());
        let spec = BadgeSpec {
            active: true,
            override_color: Some((10, 20, 30)),
            ..BadgeSpec::new("tag", "browser-tui", BadgeKind::Tag)
        };

        let rendered = spans(&spec, &theme);

        assert_eq!(color(&spec, &theme), Color::Rgb(10, 20, 30));
        assert_eq!(width(&spec), "[browser-tui]".chars().count() as u16);
        assert_eq!(rendered[1].content.as_ref(), "browser-tui");
        assert!(rendered[1].style.add_modifier.contains(Modifier::REVERSED));
    }

    #[test]
    fn tui_badge_body_actions_use_core_badge_spec_helpers() {
        let clicked = BadgeSpec {
            click_kind: BadgeClickKind::Clicked,
            ..BadgeSpec::new("tag", "alpha", BadgeKind::Tag)
        };
        let link = BadgeSpec::new(
            "link",
            "PanelCatalog",
            BadgeKind::Wikilink {
                resolved: true,
                target: "PanelCatalog".into(),
            },
        );

        assert_eq!(
            clicked.primary_action(),
            BadgeAction::Clicked {
                field: "tag".into(),
                value: "alpha".into()
            }
        );
        assert_eq!(
            link.primary_action(),
            BadgeAction::Navigate {
                target: "PanelCatalog".into()
            }
        );
    }
}

