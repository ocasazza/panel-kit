use super::Color;

/// One palette entry.
///
/// Both representations are carried because different backends need different
/// ones: CSS wants the hex string, ratatui and native toolkits want the channel
/// triple. [`Token::check`] asserts they agree.
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
        match Color::parse(self.hex) {
            Ok(color) => color == self.color(),
            Err(_) => false,
        }
    }

    /// The token's colour as a [`Color`] — the palette-building view of the
    /// same value the hex string and channel triple carry.
    pub const fn color(self) -> Color {
        Color::new(self.rgb.0, self.rgb.1, self.rgb.2)
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
