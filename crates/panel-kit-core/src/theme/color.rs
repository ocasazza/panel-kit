use serde::{Deserialize, Serialize};
use std::fmt;

/// A colour as sRGB channels, wire-encoded as one canonical string.
///
/// The only accepted spelling is lowercase `#rrggbb` — the form the CSS
/// custom properties use. Shorthand (`#fff`), uppercase hex, and named
/// colours are rejected rather than tolerated, because a codec that
/// forgives near-misses lets drifted documents parse to near-miss colours.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(try_from = "String", into = "String")]
pub struct Color {
    /// Red channel.
    pub r: u8,
    /// Green channel.
    pub g: u8,
    /// Blue channel.
    pub b: u8,
}

/// Error returned when a string is not a canonical lowercase `#rrggbb`
/// literal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ColorParseError {
    /// The rejected input, kept for the diagnostic.
    input: String,
}

impl ColorParseError {
    /// Build a diagnostic for one rejected input.
    fn new(input: &str) -> Self {
        Self {
            input: input.to_string(),
        }
    }
}

impl fmt::Display for ColorParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "not a canonical lowercase #rrggbb color: `{}`",
            self.input
        )
    }
}

impl std::error::Error for ColorParseError {}

impl Color {
    /// A colour from its sRGB channels.
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Parse a canonical lowercase `#rrggbb` literal.
    pub fn parse(text: &str) -> Result<Self, ColorParseError> {
        let bytes = text.as_bytes();
        if bytes.len() != 7 || bytes[0] != b'#' {
            return Err(ColorParseError::new(text));
        }

        match (channel(bytes, 1), channel(bytes, 3), channel(bytes, 5)) {
            (Some(r), Some(g), Some(b)) => Ok(Self { r, g, b }),
            _ => Err(ColorParseError::new(text)),
        }
    }
}

impl TryFrom<String> for Color {
    type Error = ColorParseError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

impl TryFrom<&str> for Color {
    type Error = ColorParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl From<Color> for String {
    fn from(color: Color) -> Self {
        // `:02x` is lowercase by construction, so the emitted form is the
        // canonical one `parse` accepts — a symmetric codec.
        format!("#{:02x}{:02x}{:02x}", color.r, color.g, color.b)
    }
}

/// Decode one channel from two lowercase hex digits.
fn channel(bytes: &[u8], start: usize) -> Option<u8> {
    match (
        lower_hex_digit(bytes[start]),
        lower_hex_digit(bytes[start + 1]),
    ) {
        (Some(hi), Some(lo)) => Some(hi * 16 + lo),
        _ => None,
    }
}

/// Decode one lowercase hex digit.
///
/// Lowercase hex only, so the canonical-spelling rule lives in the digit
/// table itself: `#5EF38C` fails here rather than being tolerated as a quiet
/// alias of `#5ef38c`.
fn lower_hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}
