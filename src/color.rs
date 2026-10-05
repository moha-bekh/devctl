//! Colors as the user writes them: six hex digits, `RRGGBB`, with an
//! optional leading `#`.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::ParseError;

/// One 24-bit color.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    pub const BLACK: Rgb = Rgb(0, 0, 0);
    pub const WHITE: Rgb = Rgb(0xFF, 0xFF, 0xFF);

    pub fn is_black(self) -> bool {
        self == Rgb::BLACK
    }

    /// The same hue, `factor` times as bright, each channel clamped to 255.
    pub fn scale(self, factor: f32) -> Rgb {
        let channel = |c: u8| (c as f32 * factor).round().clamp(0.0, 255.0) as u8;
        Rgb(channel(self.0), channel(self.1), channel(self.2))
    }
}

impl FromStr for Rgb {
    type Err = ParseError;

    fn from_str(text: &str) -> Result<Rgb, ParseError> {
        let hex = text.strip_prefix('#').unwrap_or(text);
        let bad = || {
            ParseError(format!(
                "`{text}` is not a color (expected RRGGBB, e.g. 8855FF)"
            ))
        };
        if hex.len() != 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(bad());
        }
        let channel = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).map_err(|_| bad());
        Ok(Rgb(channel(0)?, channel(2)?, channel(4)?))
    }
}

impl fmt::Display for Rgb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:02X}{:02X}{:02X}", self.0, self.1, self.2)
    }
}

impl Serialize for Rgb {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Rgb {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Rgb, D::Error> {
        String::deserialize(deserializer)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_color_reads_with_or_without_a_hash() {
        assert_eq!("8855FF".parse::<Rgb>().unwrap(), Rgb(0x88, 0x55, 0xFF));
        assert_eq!("#00ffff".parse::<Rgb>().unwrap(), Rgb(0, 0xFF, 0xFF));
    }

    #[test]
    fn a_color_is_refused_unless_it_is_six_hex_digits() {
        for text in ["", "FFF", "GG0000", "8855FF0", "+12345"] {
            assert!(text.parse::<Rgb>().is_err(), "{text}");
        }
    }

    #[test]
    fn scaling_keeps_the_hue_and_clamps() {
        assert_eq!(Rgb(0x88, 0x55, 0xFF).scale(0.5), Rgb(0x44, 0x2B, 0x80));
        assert_eq!(Rgb(200, 0, 100).scale(2.0), Rgb(255, 0, 200));
    }
}
