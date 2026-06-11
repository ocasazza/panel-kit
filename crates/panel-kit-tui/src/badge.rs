//! Terminal rendering of the shared badge model
//! ([`panel_kit_core::badge`]): the same kinds, colors, and click actions
//! the Dioxus shell draws as pill chips, as styled spans.

use panel_kit_core::badge::{display_label, BadgeAction, BadgeClickKind, BadgeKind, Rgb};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;

use crate::Theme;

/// One badge: a `(field, value)` pair of clickable metadata, rendered as a
/// `[label]` chip colored by kind (same color mapping as the CSS classes).
#[derive(Clone)]
pub struct Badge {
    /// Kind — selects color and click semantics.
    pub kind: BadgeKind,
    /// Field name, echoed in actions (e.g. `"tag"`).
    pub field: String,
    /// Value, echoed in actions and shown as the label.
    pub value: String,
    /// Active halo: rendered reversed, like the web `.badge.active`.
    pub active: bool,
    /// Community tint override (pair with
    /// [`tag_hue`](panel_kit_core::badge::tag_hue) + [`hue_color`]).
    pub override_color: Option<Rgb>,
}

impl Badge {
    /// A badge with default flags.
    pub fn new(kind: BadgeKind, field: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            kind,
            field: field.into(),
            value: value.into(),
            active: false,
            override_color: None,
        }
    }

    /// The chip color for this badge's kind — the terminal twin of the
    /// `badge-<kind>` CSS classes.
    pub fn color(&self, t: &Theme) -> Color {
        if let Some((r, g, b)) = self.override_color {
            return Color::Rgb(r, g, b);
        }
        match &self.kind {
            BadgeKind::Tag => t.accent,
            BadgeKind::Doctype | BadgeKind::Author => t.badge_info,
            BadgeKind::Folder => t.dim,
            BadgeKind::Entity { .. } | BadgeKind::Date => t.yellow,
            BadgeKind::Wikilink { resolved: true, .. } => t.badge_info,
            BadgeKind::Wikilink { resolved: false, .. } => t.red,
            BadgeKind::Url { .. } | BadgeKind::Status => t.green,
            BadgeKind::Generic => t.line2,
        }
    }

    /// The chip as styled spans: `[label]` in the kind color, reversed when
    /// active. Render with a `Paragraph`/`Line` and record the drawn rect
    /// for click hit-testing.
    pub fn spans(&self, t: &Theme) -> Vec<Span<'static>> {
        let c = self.color(t);
        let mut style = Style::default().fg(c);
        if self.active {
            style = style.add_modifier(Modifier::REVERSED);
        }
        vec![
            Span::styled("[", Style::default().fg(t.line2)),
            Span::styled(display_label(&self.kind, &self.value), style),
            Span::styled("]", Style::default().fg(t.line2)),
        ]
    }

    /// Rendered width in cells (for laying out chip strips / hit rects).
    pub fn width(&self) -> u16 {
        display_label(&self.kind, &self.value).chars().count() as u16 + 2
    }

    /// The action a body click delivers — identical routing to the web
    /// shell: wikilinks navigate, URLs open, everything else toggles (or
    /// raw-clicks, per `click_kind`).
    pub fn click(&self, click_kind: BadgeClickKind) -> BadgeAction {
        match &self.kind {
            BadgeKind::Wikilink { target, .. } => BadgeAction::Navigate {
                target: target.clone(),
            },
            BadgeKind::Url { href, .. } => BadgeAction::OpenUrl { href: href.clone() },
            _ => match click_kind {
                BadgeClickKind::Toggle => BadgeAction::Toggle {
                    field: self.field.clone(),
                    value: self.value.clone(),
                },
                BadgeClickKind::Clicked => BadgeAction::Clicked {
                    field: self.field.clone(),
                    value: self.value.clone(),
                },
            },
        }
    }
}

/// HSL(hue, 0.55, 0.55) → RGB, for tinting tag badges from
/// [`tag_hue`](panel_kit_core::badge::tag_hue).
pub fn hue_color(hue: f32) -> Rgb {
    let (s, l) = (0.55_f32, 0.55_f32);
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let h6 = (hue.rem_euclid(1.0)) * 6.0;
    let x = c * (1.0 - (h6 % 2.0 - 1.0).abs());
    let (r, g, b) = match h6 as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    (
        ((r + m) * 255.0) as u8,
        ((g + m) * 255.0) as u8,
        ((b + m) * 255.0) as u8,
    )
}
