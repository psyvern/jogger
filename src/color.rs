use serde::Deserialize;
use std::{fmt::Display, num::IntErrorKind, str::FromStr};

#[derive(Clone, Copy, Debug, Default)]
pub struct PangoColor {
    r: u16,
    g: u16,
    b: u16,
}

impl From<PangoColor> for [u16; 3] {
    fn from(value: PangoColor) -> Self {
        [value.r, value.g, value.b]
    }
}

impl Display for PangoColor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "#{:02X}{:02X}{:02X}",
            self.r / 0x101,
            self.g / 0x101,
            self.b / 0x101
        )
    }
}

impl FromStr for PangoColor {
    type Err = IntErrorKind;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.chars().next() {
            Some('#') => {
                let len = match s.len() - 1 {
                    len @ (3 | 6 | 9 | 12) => len / 3,
                    _ => {
                        return Err(IntErrorKind::Zero);
                    }
                };

                let mut r =
                    u16::from_str_radix(&s[1..(1 + len)], 16).map_err(|x| x.kind().clone())?;
                let mut g = u16::from_str_radix(&s[(1 + len)..(1 + 2 * len)], 16)
                    .map_err(|x| x.kind().clone())?;
                let mut b = u16::from_str_radix(&s[(1 + 2 * len)..(1 + 3 * len)], 16)
                    .map_err(|x| x.kind().clone())?;

                let mut bits = len * 4;
                r <<= 16 - bits;
                g <<= 16 - bits;
                b <<= 16 - bits;
                while bits < 16 {
                    r |= r >> bits;
                    g |= g >> bits;
                    b |= b >> bits;
                    bits *= 2;
                }

                Ok(Self { r, g, b })
            }
            // Some(_) => find_color(spec),
            _ => Err(IntErrorKind::Empty),
        }
    }
}

impl<'de> Deserialize<'de> for PangoColor {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        s.parse()
            .map_err(|x| serde::de::Error::custom(format!("{x:?}")))
    }
}
