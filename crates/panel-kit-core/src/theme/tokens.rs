use super::Color;

/// One palette entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Token {
    pub name: &'static str,
    pub hex: &'static str,
    pub rgb: (u8, u8, u8),
}

impl Token {
    pub fn check(&self) -> bool {
        match Color::parse(self.hex) {
            Ok(color) => color == self.color(),
            Err(_) => false,
        }
    }
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

token!(BACKGROUND, "color-background", "#0a0a0a", 0x0a, 0x0a, 0x0a, "Page background.");
token!(SURFACE, "color-surface", "#0d0d0d", 0x0d, 0x0d, 0x0d, "Raised surfaces.");
token!(TEXT, "color-text", "#ededed", 0xed, 0xed, 0xed, "Body and title text.");
token!(TEXT_MUTED, "color-text-muted", "#7a7a7a", 0x7a, 0x7a, 0x7a, "De-emphasized text.");
token!(DIVIDER, "color-divider", "#262626", 0x26, 0x26, 0x26, "Structural hairline dividing regions.");
token!(BORDER, "color-border", "#5f5f5f", 0x5f, 0x5f, 0x5f, "Object outline.");
token!(INVERSE_BACKGROUND, "color-inverse-background", "#ededed", 0xed, 0xed, 0xed, "Inverted background, used by selection.");
token!(INVERSE_TEXT, "color-inverse-text", "#0a0a0a", 0x0a, 0x0a, 0x0a, "Inverted foreground, used by selection.");
token!(FOCUS_RING, "color-focus-ring", "#ededed", 0xed, 0xed, 0xed, "Keyboard-focus indicator.");
token!(PRIMARY, "color-primary", "#3b9bff", 0x3b, 0x9b, 0xff, "Floating/tiling mode light.");
token!(SECONDARY, "color-secondary", "#ff5fc3", 0xff, 0x5f, 0xc3, "Maximize/restore light.");
token!(TERTIARY, "color-tertiary", "#ffbd2e", 0xff, 0xbd, 0x2e, "Minimize light.");
token!(HIGHLIGHT, "color-highlight", "#5ef38c", 0x5e, 0xf3, 0x8c, "Live-system signal.");
token!(SUCCESS, "color-success", "#27c93f", 0x27, 0xc9, 0x3f, "Verdict: resolved, valid, reachable.");
token!(FAILURE, "color-failure", "#ff5f56", 0xff, 0x5f, 0x56, "Failed checks and verdicts.");
token!(ERROR, "color-error", "#ff5f56", 0xff, 0x5f, 0x56, "Errors and unresolved references.");
token!(WARNING, "color-warning", "#ffbd2e", 0xff, 0xbd, 0x2e, "Warnings and thresholds.");
token!(INFO, "color-info", "#83b7cc", 0x83, 0xb7, 0xcc, "Informational metadata badges.");

pub const MONO: &str =
    "ui-monospace, \"SF Mono\", \"JetBrains Mono\", \"Menlo\", \"Consolas\", monospace";

pub const DARK: &[Token] = &[
    BACKGROUND, SURFACE, TEXT, TEXT_MUTED, DIVIDER, BORDER, INVERSE_BACKGROUND, INVERSE_TEXT,
    FOCUS_RING, PRIMARY, SECONDARY, TERTIARY, HIGHLIGHT, SUCCESS, FAILURE, ERROR, WARNING, INFO,
];

pub fn by_name(name: &str) -> Option<Token> {
    // Boot CSS placeholders use role names ("background"); token names use the
    // full CSS var name ("color-background"). Match either form.
    let full = if name.starts_with("color-") {
        name.to_string()
    } else {
        format!("color-{name}")
    };
    DARK.iter().copied().find(|t| t.name == full || t.name == name)
}