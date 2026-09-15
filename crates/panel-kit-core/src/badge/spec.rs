//! Complete badge specification shared by backend painters.

use serde::{Deserialize, Serialize};

use super::{BadgeAction, BadgeClickKind, BadgeKind, Rgb};

/// Complete renderer-neutral badge model consumed by every backend painter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct BadgeSpec {
    /// Visual and behavioral badge kind.
    pub kind: BadgeKind,
    /// Attribute name half of the badge pair.
    pub field: String,
    /// Attribute value half of the badge pair.
    pub value: String,
    /// Whether the badge is currently active.
    pub active: bool,
    /// Whether to expose the trailing toggle affordance.
    pub with_x: bool,
    /// Whether to expose the add-filter affordance.
    pub with_plus: bool,
    /// Whether to use compact badge geometry.
    pub small: bool,
    /// Community tint override shared by web and terminal.
    pub override_color: Option<Rgb>,
    /// Portable accent color; arbitrary CSS strings stay web-only.
    pub accent_color: Option<Rgb>,
    /// Body-click semantics for non-link badge kinds.
    pub click_kind: BadgeClickKind,
    /// Whether pointer hover should emit a hover action.
    pub emit_hover: bool,
}

impl BadgeSpec {
    /// Build a badge spec with the default rendering flags.
    pub fn new(field: impl Into<String>, value: impl Into<String>, kind: BadgeKind) -> Self {
        Self {
            kind,
            field: field.into(),
            value: value.into(),
            active: false,
            with_x: false,
            with_plus: false,
            small: false,
            override_color: None,
            accent_color: None,
            click_kind: BadgeClickKind::Toggle,
            emit_hover: false,
        }
    }

    /// Action delivered by a primary body click.
    pub fn primary_action(&self) -> BadgeAction {
        match &self.kind {
            BadgeKind::Wikilink { target, .. } => BadgeAction::Navigate { target: target.clone() },
            BadgeKind::Url { href, .. } => BadgeAction::OpenUrl { href: href.clone() },
            _ => self.field_value_action(self.click_kind),
        }
    }

    /// Action delivered by the optional `+` affordance.
    pub fn plus_action(&self) -> BadgeAction {
        BadgeAction::AddFilter { field: self.field.clone(), value: self.value.clone() }
    }

    /// Action delivered by the optional `×` affordance.
    pub fn x_action(&self) -> BadgeAction {
        BadgeAction::Toggle { field: self.field.clone(), value: self.value.clone() }
    }

    /// Action delivered by pointer hover when `emit_hover` is enabled.
    pub fn hover_action(&self) -> Option<BadgeAction> {
        self.emit_hover.then(|| BadgeAction::Hovered { field: self.field.clone(), value: self.value.clone() })
    }

    fn field_value_action(&self, click_kind: BadgeClickKind) -> BadgeAction {
        match click_kind {
            BadgeClickKind::Toggle => BadgeAction::Toggle { field: self.field.clone(), value: self.value.clone() },
            BadgeClickKind::Clicked => BadgeAction::Clicked { field: self.field.clone(), value: self.value.clone() },
        }
    }
}
