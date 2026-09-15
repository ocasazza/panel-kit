use super::*;

#[test]
fn badge_spec_covers_web_and_tui_fields() {
    let spec = BadgeSpec {
        kind: BadgeKind::Tag,
        field: "tag".into(),
        value: "alpha".into(),
        active: true,
        with_x: true,
        with_plus: true,
        small: true,
        override_color: Some((10, 20, 30)),
        accent_color: Some((30, 20, 10)),
        click_kind: BadgeClickKind::Clicked,
        emit_hover: true,
    };

    assert_eq!(spec.field, "tag");
    assert_eq!(spec.value, "alpha");
    assert!(spec.active);
    assert!(spec.with_x);
    assert!(spec.with_plus);
    assert!(spec.small);
    assert_eq!(spec.override_color, Some((10, 20, 30)));
    assert_eq!(spec.accent_color, Some((30, 20, 10)));
    assert_eq!(spec.click_kind, BadgeClickKind::Clicked);
    assert!(spec.emit_hover);
}

#[test]
fn badge_actions_match_across_backends() {
    let spec = BadgeSpec {
        with_plus: true,
        with_x: true,
        emit_hover: true,
        ..BadgeSpec::new("tag", "alpha", BadgeKind::Tag)
    };
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
    let url = BadgeSpec::new(
        "url",
        "ratzilla",
        BadgeKind::Url {
            href: "https://github.com/ratatui/ratzilla".into(),
            host: "github.com".into(),
        },
    );

    assert_eq!(
        spec.primary_action(),
        BadgeAction::Toggle {
            field: "tag".into(),
            value: "alpha".into()
        }
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
    assert_eq!(
        url.primary_action(),
        BadgeAction::OpenUrl {
            href: "https://github.com/ratatui/ratzilla".into()
        }
    );
    assert_eq!(
        spec.plus_action(),
        BadgeAction::AddFilter {
            field: "tag".into(),
            value: "alpha".into()
        }
    );
    assert_eq!(
        spec.x_action(),
        BadgeAction::Toggle {
            field: "tag".into(),
            value: "alpha".into()
        }
    );
    assert_eq!(
        spec.hover_action(),
        Some(BadgeAction::Hovered {
            field: "tag".into(),
            value: "alpha".into()
        })
    );
}

#[test]
fn tag_hue_deterministic() {
    assert_eq!(tag_hue("alpha"), tag_hue("alpha"));
    assert!((0.0..1.0).contains(&tag_hue("alpha")));
    assert_ne!(tag_hue("alpha"), tag_hue("beta"));
}

#[test]
fn wikilink_label_prefixed() {
    let k = BadgeKind::Wikilink { resolved: true, target: "Page".into() };

    assert_eq!(display_label(&k, "Page"), "\u{27F6} Page");
}

#[test]
fn url_label_prefers_host() {
    let k = BadgeKind::Url { href: "https://example.com/x".into(), host: "example.com".into() };
    let bare = BadgeKind::Url { href: "https://example.com".into(), host: String::new() };

    assert_eq!(display_label(&k, "https://example.com/x"), "example.com");
    assert_eq!(display_label(&bare, "https://example.com"), "https://example.com");
}
