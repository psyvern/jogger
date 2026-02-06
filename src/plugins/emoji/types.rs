use std::{fmt::Display, str::FromStr};

#[derive(Debug)]
pub struct UnicodeVersion {
    pub major: u16,
    pub minor: u16,
}

impl Display for UnicodeVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

impl FromStr for UnicodeVersion {
    type Err = bool;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let Some(s) = s.strip_prefix('E') else {
            return Err(false);
        };

        let Some((major, minor)) = s.split_once('.') else {
            return Err(false);
        };

        let Ok(major) = major.parse() else {
            return Err(true);
        };

        let Ok(minor) = minor.parse() else {
            return Err(true);
        };

        Ok(Self { major, minor })
    }
}

#[derive(Debug)]
pub struct EmojiVariant {
    pub codepoints: &'static str,
    pub version: UnicodeVersion,
    pub attributes: &'static [&'static str],
}

#[derive(Debug, Default)]
pub struct Emoji {
    pub group: usize,
    pub subgroup: usize,
    pub description: &'static str,
    pub variants: &'static [EmojiVariant],
}
