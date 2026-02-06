use std::{fmt::Display, str::FromStr};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Category {
    // Normative Categories
    LetterUppercase,
    LetterLowercase,
    LetterTitlecase,
    MarkNonSpacing,
    MarkSpacingCombining,
    MarkEnclosing,
    NumberDecimalDigit,
    NumberLetter,
    NumberOther,
    SeparatorSpace,
    SeparatorLine,
    SeparatorParagraph,
    OtherControl,
    OtherFormat,
    OtherSurrogate,
    OtherPrivateUse,
    OtherNotAssigned,

    // Informative Categories
    LetterModifier,
    LetterOther,
    PunctuationConnector,
    PunctuationDash,
    PunctuationOpen,
    PunctuationClose,
    PunctuationInitialQuote,
    PunctuationFinalQuote,
    PunctuationOther,
    SymbolMath,
    SymbolCurrency,
    SymbolModifier,
    SymbolOther,

    // NerdFonts Categories
    PowerlineSymbols,
    FontAwesome,
    Devicons,
    WeatherIcons,
    SetiUI,
    NfCustom,
    Octicons,
    FontLogos,
    IecPowerSymbols,
    Pomicons,
    MaterialDesign,
    Codicons,
}

impl FromStr for Category {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Category, std::string::String> {
        match s {
            "Lu" => Ok(Category::LetterUppercase),
            "Ll" => Ok(Category::LetterLowercase),
            "Lt" => Ok(Category::LetterTitlecase),
            "Mn" => Ok(Category::MarkNonSpacing),
            "Mc" => Ok(Category::MarkSpacingCombining),
            "Me" => Ok(Category::MarkEnclosing),
            "Nd" => Ok(Category::NumberDecimalDigit),
            "Nl" => Ok(Category::NumberLetter),
            "No" => Ok(Category::NumberOther),
            "Zs" => Ok(Category::SeparatorSpace),
            "Zl" => Ok(Category::SeparatorLine),
            "Zp" => Ok(Category::SeparatorParagraph),
            "Cc" => Ok(Category::OtherControl),
            "Cf" => Ok(Category::OtherFormat),
            "Cs" => Ok(Category::OtherSurrogate),
            "Co" => Ok(Category::OtherPrivateUse),
            "Cn" => Ok(Category::OtherNotAssigned),
            "Lm" => Ok(Category::LetterModifier),
            "Lo" => Ok(Category::LetterOther),
            "Pc" => Ok(Category::PunctuationConnector),
            "Pd" => Ok(Category::PunctuationDash),
            "Ps" => Ok(Category::PunctuationOpen),
            "Pe" => Ok(Category::PunctuationClose),
            "Pi" => Ok(Category::PunctuationInitialQuote),
            "Pf" => Ok(Category::PunctuationFinalQuote),
            "Po" => Ok(Category::PunctuationOther),
            "Sm" => Ok(Category::SymbolMath),
            "Sc" => Ok(Category::SymbolCurrency),
            "Sk" => Ok(Category::SymbolModifier),
            "So" => Ok(Category::SymbolOther),
            _ => Err(s.to_owned()),
        }
    }
}

impl Display for Category {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

#[derive(Clone, Debug)]
pub struct Char<'a> {
    pub scalar: char,
    pub codepoint: u32,
    pub name: &'a [u8],
    pub aliases: &'a [&'a [u8]],
    pub category: Category,
}

impl Char<'_> {
    pub fn representation(&self) -> char {
        let c = self.scalar;

        match c {
            ' ' => '⎵',
            '\0'..'\u{20}' => char::from_u32(c as u32 + 0x2400).unwrap_or(c),
            c if c.is_whitespace() || c.is_control() => '�',
            c => c,
        }
    }
}
