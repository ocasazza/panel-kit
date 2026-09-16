use super::*;

// === Seed invariants (moved from tokens.rs unchanged) ===

#[test]
fn hex_and_rgb_agree_for_every_token() {
    for t in DARK {
        assert!(
            t.check(),
            "{}: {} does not equal {:?}",
            t.name,
            t.hex,
            t.rgb
        );
    }
}

#[test]
fn token_check_uses_the_canonical_color_codec() {
    let canonical = Token {
        name: "accent",
        hex: "#5ef38c",
        rgb: (0x5e, 0xf3, 0x8c),
    };
    let uppercase = Token {
        hex: "#5EF38C",
        ..canonical
    };
    let mismatch = Token {
        rgb: (0x5e, 0xf3, 0x8d),
        ..canonical
    };

    assert!(canonical.check());
    assert!(!uppercase.check());
    assert!(!mismatch.check());
}

#[test]
fn line2_clears_the_non_text_contrast_threshold_against_panel() {
    fn channel(c: u8) -> f64 {
        let c = c as f64 / 255.0;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    }

    fn luminance(t: Token) -> f64 {
        let (r, g, b) = t.rgb;
        0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b)
    }

    let ratio = (luminance(LINE2) + 0.05) / (luminance(PANEL) + 0.05);
    assert!(ratio >= 3.0, "line2 on panel is {ratio:.4}, below 3:1");
}

#[test]
fn token_names_are_unique() {
    for (i, a) in DARK.iter().enumerate() {
        for b in &DARK[i + 1..] {
            assert_ne!(a.name, b.name, "duplicate token name {}", a.name);
        }
    }
}

// === Slice 9 core half: the structured token set ===

/// HR-11 core half. The web `:root` block is pinned to the `DARK` seed by the
/// web crate's stylesheet test, and the terminal `Theme::DARK` is built from
/// the same constants — so the keys both consumers derive from are exactly the
/// seed slice, and the structured preset must carry every one of them plus
/// `focus_ring`.
#[test]
fn dark_theme_emits_every_web_and_tui_token() {
    let dark = ThemeTokens::dark().colors;

    for token in DARK {
        let slot = ThemeColor::ALL
            .iter()
            .copied()
            .find(|slot| slot.key() == token.name)
            .unwrap_or_else(|| panic!("no ThemeColor slot carries key `{}`", token.name));
        assert_eq!(
            dark.resolve(slot),
            token.color(),
            "dark preset drifts from the `{}` seed",
            token.name,
        );
    }

    // `focus_ring` must exist as a slot even though no generated consumer uses
    // it yet; in the dark preset it aliases the foreground, matching
    // `--focus-ring: var(--fg)`.
    assert_eq!(dark.focus_ring, dark.fg);
    assert_eq!(dark.resolve(ThemeColor::FocusRing), dark.fg);

    let mut keys: Vec<_> = ThemeColor::ALL.iter().map(|slot| slot.key()).collect();
    keys.sort_unstable();
    keys.dedup();
    assert_eq!(
        keys.len(),
        ThemeColor::ALL.len(),
        "duplicate ThemeColor keys"
    );
}

/// The non-colour tokens are seeded from the documented canonical values
/// (DESIGN.md hierarchy and spacing ramps), so a preset cannot quietly retype
/// the system or drift from the web sheet.
#[test]
fn presets_carry_the_canonical_typography_and_density() {
    for theme in [ThemeTokens::dark(), ThemeTokens::paper()] {
        assert_eq!(theme.typography.family, MONO);
        assert_eq!(theme.typography.body_size, 13.0);
        assert_eq!(theme.typography.body_line_height, 1.5);
        assert_eq!(theme.typography.label_size, 0.72);
        assert_eq!(theme.typography.label_weight, 700);
        assert_eq!(theme.typography.label_tracking, 0.06);
        assert_eq!(theme.density.panel_radius, 4.0);
        assert_eq!(theme.density.badge_radius, 999.0);
        assert_eq!(theme.density.spacing_xs, 4.0);
        assert_eq!(theme.density.spacing_sm, 6.0);
        assert_eq!(theme.density.spacing_md, 8.0);
    }
}

/// The codec accepts and emits exactly one spelling.
#[test]
fn color_codec_accepts_only_canonical_lowercase_hex() {
    let accent = Color::try_from("#5ef38c").expect("canonical literal must parse");
    assert_eq!(accent, Color::new(0x5e, 0xf3, 0x8c));
    assert_eq!(String::from(accent), "#5ef38c");

    for bad in [
        "#5EF38C",
        "#5Ef38c",
        "5ef38c",
        "#5ef38c;",
        " #5ef38c",
        "fff",
        "#fff",
        "#ffff",
        "#5ef38",
        "#5ef38cc",
        "#gggggg",
        "red",
        "rebeccapurple",
        "",
    ] {
        assert!(
            Color::try_from(bad).is_err(),
            "`{bad}` must not parse as a color",
        );
    }
}

/// The serde codec is the same strict one, so a drifted document fails loudly
/// instead of parsing to a near-miss colour.
#[test]
fn color_serde_round_trips_the_canonical_string_form() {
    let color = Color::new(0x0a, 0x0a, 0x0a);
    assert_eq!(serde_json::to_string(&color).unwrap(), "\"#0a0a0a\"");
    assert_eq!(serde_json::from_str::<Color>("\"#0a0a0a\"").unwrap(), color);
    assert!(serde_json::from_str::<Color>("\"#0A0A0A\"").is_err());
    assert!(serde_json::from_str::<Color>("\"red\"").is_err());
}

/// Design §8.1 wire shape: adjacent tagging with snake_case spellings — the
/// strict spec depends on these exact forms.
#[test]
fn color_ref_serializes_as_tagged_source_and_value() {
    assert_eq!(
        serde_json::to_string(&ColorRef::Literal(Color::new(0x5e, 0xf3, 0x8c))).unwrap(),
        r##"{"source":"literal","value":"#5ef38c"}"##,
    );

    for (slot, spelling) in [
        (ThemeColor::Accent, "accent"),
        (ThemeColor::Line2, "line2"),
        (ThemeColor::InverseBg, "inverse_bg"),
        (ThemeColor::InverseFg, "inverse_fg"),
        (ThemeColor::BadgeInfo, "badge_info"),
        (ThemeColor::FocusRing, "focus_ring"),
    ] {
        assert_eq!(
            serde_json::to_string(&ColorRef::Theme(slot)).unwrap(),
            format!(r#"{{"source":"theme","value":"{spelling}"}}"#),
        );
    }
}
