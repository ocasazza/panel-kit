//! The canonical palette: one source of truth for every renderer.
//!
//! panel-kit's colours previously existed as three hand-maintained copies —
//! the `:root` block in `assets/panel-kit.css`, the `Theme` struct in
//! `panel-kit-tui`, and the pre-WASM boot stylesheet. They were byte-identical
//! by discipline alone, and the discipline had already failed: the boot shell
//! carried seven near-miss values (`#09090a` for `--bg`'s `#0a0a0a`, `#efeff0`
//! for `#ededed`, and so on) plus its own font stack.
//!
//! Near-miss drift is the worst kind, because nothing looks broken. A copy
//! that is obviously wrong gets fixed; a copy that is one hex digit off just
//! quietly stops being the same product.
//!
//! So the values live here, in the crate both renderers already depend on:
//!
//! - `assets/panel-kit.css` declares them as `:root` custom properties, and a
//!   test in the web crate asserts every declaration matches this module.
//! - `panel-kit-tui`'s [`Theme::DARK`](../../panel_kit_tui/theme/struct.Theme.html)
//!   is built from these constants rather than transcribing them.
//! - The boot stylesheet is *generated* from these constants at build time,
//!   because it paints before WASM exists and so cannot read the injected
//!   stylesheet at all.
//!
//! A new backend — iPad, Android, native, embedded — consumes this module and
//! is correct by construction.

/// One palette entry.
///
/// Both representations are carried because different backends need
/// different ones: CSS wants the hex string, ratatui and native toolkits want
/// the channel triple. [`Token::check`] asserts they agree.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Token {
    /// CSS custom-property name, without the leading `--`.
    pub name: &'static str,
    /// Lowercase `#rrggbb`.
    pub hex: &'static str,
    /// The same colour as sRGB channels.
    pub rgb: (u8, u8, u8),
}

impl Token {
    /// Whether [`hex`](Token::hex) and [`rgb`](Token::rgb) describe the same
    /// colour. Used by the token test; a mismatch is a typo, not a choice.
    pub fn check(&self) -> bool {
        let b = self.hex.as_bytes();
        if b.len() != 7 || b[0] != b'#' {
            return false;
        }
        let nib = |c: u8| match c {
            b'0'..=b'9' => Some(c - b'0'),
            b'a'..=b'f' => Some(c - b'a' + 10),
            _ => None,
        };
        let byte = |i: usize| match (nib(b[i]), nib(b[i + 1])) {
            (Some(hi), Some(lo)) => Some(hi * 16 + lo),
            _ => None,
        };
        byte(1) == Some(self.rgb.0) && byte(3) == Some(self.rgb.1) && byte(5) == Some(self.rgb.2)
    }
}

macro_rules! token {
    ($konst:ident, $name:literal, $hex:literal, $r:literal, $g:literal, $b:literal, $doc:literal) => {
        #[doc = $doc]
        pub const $konst: Token = Token {
            name: $name,
            hex: $hex,
            rgb: ($r, $g, $b),
        };
    };
}

token!(
    BG,
    "bg",
    "#0a0a0a",
    0x0a,
    0x0a,
    0x0a,
    "Page background, and the recessed field behind chips and badges."
);
token!(
    PANEL,
    "panel",
    "#0d0d0d",
    0x0d,
    0x0d,
    0x0d,
    "Every raised surface: panels, topbar, dock, tooltip."
);
token!(
    FG,
    "fg",
    "#ededed",
    0xed,
    0xed,
    0xed,
    "Body and title text."
);
token!(
    DIM,
    "dim",
    "#7a7a7a",
    0x7a,
    0x7a,
    0x7a,
    "De-emphasized text. The only grey between `FG` and the lines."
);
token!(
    LINE,
    "line",
    "#262626",
    0x26,
    0x26,
    0x26,
    "Structural hairline dividing regions. Never a text colour."
);
token!(
    LINE2,
    "line2",
    "#5f5f5f",
    0x5f,
    0x5f,
    0x5f,
    "Object outline. Reaches 3:1 against `PANEL`; never a text colour."
);
token!(
    INV_BG,
    "inv-bg",
    "#ededed",
    0xed,
    0xed,
    0xed,
    "Inverted background, used by selection."
);
token!(
    INV_FG,
    "inv-fg",
    "#0a0a0a",
    0x0a,
    0x0a,
    0x0a,
    "Inverted foreground, used by selection."
);
token!(
    ACCENT,
    "accent",
    "#5ef38c",
    0x5e,
    0xf3,
    0x8c,
    "Live-system signal only: activity, loading, selection, drag-in-flight."
);
token!(
    RED,
    "red",
    "#ff5f56",
    0xff,
    0x5f,
    0x56,
    "Errors and unresolved references."
);
token!(
    YELLOW,
    "yellow",
    "#ffbd2e",
    0xff,
    0xbd,
    0x2e,
    "Minimize light."
);
token!(
    GREEN,
    "green",
    "#27c93f",
    0x27,
    0xc9,
    0x3f,
    "Verdict: resolved, valid, reachable."
);
token!(
    BLUE,
    "blue",
    "#3b9bff",
    0x3b,
    0x9b,
    0xff,
    "Floating/tiling mode light."
);
token!(
    PINK,
    "pink",
    "#ff5fc3",
    0xff,
    0x5f,
    0xc3,
    "Maximize/restore light."
);
token!(
    BADGE_INFO,
    "badge-info",
    "#83b7cc",
    0x83,
    0xb7,
    0xcc,
    "Informational metadata badges, and the resting tint for tags."
);

/// The monospace stack, as a CSS `font-family` value.
///
/// One family across every role is a design rule, not a default, so the stack
/// is a token like any colour — the boot stylesheet previously shipped a
/// different one.
pub const MONO: &str =
    "ui-monospace, \"SF Mono\", \"JetBrains Mono\", \"Menlo\", \"Consolas\", monospace";

/// Every colour token, in declaration order.
///
/// Consumers that generate a stylesheet or a native palette should iterate
/// this rather than naming constants, so a token added here reaches every
/// backend without an edit.
pub const DARK: &[Token] = &[
    BG, PANEL, FG, DIM, LINE, LINE2, INV_BG, INV_FG, ACCENT, RED, YELLOW, GREEN, BLUE, PINK,
    BADGE_INFO,
];

/// Look a token up by its CSS name.
pub fn by_name(name: &str) -> Option<Token> {
    DARK.iter().copied().find(|t| t.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn line2_clears_the_non_text_contrast_threshold_against_panel() {
        // The reason --line2 moved off #3a3a3a: panel borders and the resize
        // grip are UI boundaries, and the grip is drawn *only* in this colour,
        // so failing 3:1 made a control invisible rather than merely subtle.
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
}
