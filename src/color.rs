use core::fmt;
use core::str::FromStr;

/// An 8-bit sRGB color with alpha.
///
/// The custom picker edits RGB and can optionally edit alpha. When alpha
/// editing is disabled, the incoming alpha component is preserved.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const BLACK: Self = Self::rgb(0, 0, 0);
    pub const WHITE: Self = Self::rgb(255, 255, 255);
    pub const TRANSPARENT: Self = Self::rgba(0, 0, 0, 0);

    #[must_use]
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    #[must_use]
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Returns `#RRGGBB`.
    #[must_use]
    pub fn to_hex_rgb(self) -> String {
        format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
    }

    /// Returns `#RRGGBBAA`.
    #[must_use]
    pub fn to_hex_rgba(self) -> String {
        format!("#{:02X}{:02X}{:02X}{:02X}", self.r, self.g, self.b, self.a)
    }

    /// Converts to Win32 `COLORREF` layout (`0x00BBGGRR`).
    #[must_use]
    pub const fn to_colorref(self) -> u32 {
        self.r as u32 | ((self.g as u32) << 8) | ((self.b as u32) << 16)
    }

    /// Converts a Win32 `COLORREF` (`0x00BBGGRR`) to an opaque color.
    #[must_use]
    pub const fn from_colorref(value: u32) -> Self {
        Self::rgb(
            (value & 0xFF) as u8,
            ((value >> 8) & 0xFF) as u8,
            ((value >> 16) & 0xFF) as u8,
        )
    }

    /// Converts a Win32 `COLORREF` while retaining a caller-supplied alpha.
    #[must_use]
    pub const fn from_colorref_with_alpha(value: u32, alpha: u8) -> Self {
        Self::rgba(
            (value & 0xFF) as u8,
            ((value >> 8) & 0xFF) as u8,
            ((value >> 16) & 0xFF) as u8,
            alpha,
        )
    }

    pub fn parse_hex(value: &str) -> Result<Self, ParseColorError> {
        let hex = value.strip_prefix('#').unwrap_or(value);

        match hex.len() {
            3 => Ok(Self::rgb(
                expand_nibble(parse_nibble(&hex[0..1])?),
                expand_nibble(parse_nibble(&hex[1..2])?),
                expand_nibble(parse_nibble(&hex[2..3])?),
            )),
            4 => Ok(Self::rgba(
                expand_nibble(parse_nibble(&hex[0..1])?),
                expand_nibble(parse_nibble(&hex[1..2])?),
                expand_nibble(parse_nibble(&hex[2..3])?),
                expand_nibble(parse_nibble(&hex[3..4])?),
            )),
            6 => Ok(Self::rgb(
                parse_byte(&hex[0..2])?,
                parse_byte(&hex[2..4])?,
                parse_byte(&hex[4..6])?,
            )),
            8 => Ok(Self::rgba(
                parse_byte(&hex[0..2])?,
                parse_byte(&hex[2..4])?,
                parse_byte(&hex[4..6])?,
                parse_byte(&hex[6..8])?,
            )),
            _ => Err(ParseColorError),
        }
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.a == 255 {
            write!(f, "#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
        } else {
            write!(
                f,
                "#{:02X}{:02X}{:02X}{:02X}",
                self.r, self.g, self.b, self.a
            )
        }
    }
}

impl FromStr for Color {
    type Err = ParseColorError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse_hex(s)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParseColorError;

impl fmt::Display for ParseColorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("invalid hex color; expected RGB, RGBA, RRGGBB, or RRGGBBAA")
    }
}

impl std::error::Error for ParseColorError {}

fn parse_nibble(value: &str) -> Result<u8, ParseColorError> {
    u8::from_str_radix(value, 16).map_err(|_| ParseColorError)
}

fn expand_nibble(value: u8) -> u8 {
    (value << 4) | value
}

fn parse_byte(value: &str) -> Result<u8, ParseColorError> {
    u8::from_str_radix(value, 16).map_err(|_| ParseColorError)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_supported_hex_forms() {
        assert_eq!(
            Color::parse_hex("#123").unwrap(),
            Color::rgb(0x11, 0x22, 0x33)
        );
        assert_eq!(
            Color::parse_hex("1234").unwrap(),
            Color::rgba(0x11, 0x22, 0x33, 0x44)
        );
        assert_eq!(
            Color::parse_hex("#123456").unwrap(),
            Color::rgb(0x12, 0x34, 0x56)
        );
        assert_eq!(
            Color::parse_hex("12345678").unwrap(),
            Color::rgba(0x12, 0x34, 0x56, 0x78)
        );
    }

    #[test]
    fn rejects_invalid_hex() {
        assert!(Color::parse_hex("#12").is_err());
        assert!(Color::parse_hex("#GG0000").is_err());
    }

    #[test]
    fn colorref_round_trip() {
        let color = Color::rgb(12, 34, 56);
        assert_eq!(Color::from_colorref(color.to_colorref()), color);
    }

    #[test]
    fn display_omits_opaque_alpha() {
        assert_eq!(Color::rgb(1, 2, 3).to_string(), "#010203");
        assert_eq!(Color::rgba(1, 2, 3, 4).to_string(), "#01020304");
    }
}
