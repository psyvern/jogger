use std::cmp::Ordering;
use std::collections::HashMap;
use std::collections::HashSet;
use std::env;
use std::fmt::Debug;
use std::fs;
use std::fs::File;
use std::io::{BufRead, BufReader, Result};
use std::num::Wrapping;
use std::path::Path;

use const_gen::*;
use itertools::Itertools;
use serde::Deserialize;

include!("src/plugins/unicode/types.rs");

fn main() -> Result<()> {
    println!("cargo::rerun-if-changed=data/unicode/UnicodeData.txt");
    println!("cargo::rerun-if-changed=data/unicode/NameAliases.txt");
    println!("cargo::rerun-if-changed=data/unicode/emoji-sequences.txt");
    println!("cargo::rerun-if-changed=data/unicode/emoji-test.txt");
    println!("cargo::rerun-if-changed=data/unicode/glyphnames.json");

    println!("cargo::rerun-if-changed=build.rs");

    #[derive(Debug)]
    struct ByteString(String);

    impl CompileConst for ByteString {
        fn const_type() -> String {
            Vec::<u8>::const_type()
        }

        fn const_val(&self) -> String {
            format!("b{}", self.0.const_val())
        }
    }

    #[derive(Debug)]
    struct Char {
        scalar: char,
        codepoint: u32,
        name: ByteString,
        aliases: Vec<ByteString>,
        category: Category,
    }

    impl CompileConst for Char {
        fn const_type() -> String {
            "Char".into()
        }

        fn const_val(&self) -> String {
            format!(
                "{} {{ scalar: {:?}, codepoint: {}, name: {}, aliases: {}, category: Category::{} }}",
                "Char",
                self.scalar,
                self.codepoint.const_val(),
                self.name.const_val(),
                self.aliases.const_val(),
                self.category,
            )
        }
    }

    impl std::hash::Hash for Char {
        fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
            self.scalar.hash(state);
        }
    }

    impl PartialEq for Char {
        fn eq(&self, other: &Self) -> bool {
            self.scalar == other.scalar
        }
    }

    impl Eq for Char {}

    impl PartialOrd for Char {
        fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
            Some(self.cmp(other))
        }
    }

    impl Ord for Char {
        fn cmp(&self, other: &Self) -> Ordering {
            self.scalar.cmp(&other.scalar)
        }
    }

    let mut aliases = HashMap::<u32, Vec<String>>::new();

    let file = File::open("data/unicode/NameAliases.txt").unwrap();
    for line in BufReader::new(file).lines().map_while(Result::ok) {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let mut parts = line.split(';');
        let codepoint =
            u32::from_str_radix(parts.next().expect("NameAliases.txt is malformed"), 16)
                .expect("NameAliases.txt is malformed");
        let alias = parts.next().expect("NameAliases.txt is malformed");

        aliases.entry(codepoint).or_default().push(alias.to_owned());
    }

    let mut vector: Vec<Char> = Vec::new();
    let data_file = File::open("data/unicode/UnicodeData.txt")?;
    for line_result in BufReader::new(data_file).lines() {
        let line = line_result?;
        let line = Box::leak(line.into_boxed_str());
        let mut parts = line.split(';');
        let codepoint = u32::from_str_radix(parts.next().expect("data is corrupt"), 16)
            .expect("data is corrupt");

        let Some(scalar) = char::from_u32(codepoint) else {
            continue;
        };

        let name = parts.next().expect("data is corrupt");

        let category = parts.next().expect("data is corrupt");
        let category = category.parse().expect("Unrecognised category");

        let mut names = Vec::new();
        if category != Category::OtherControl {
            names.push(name.to_owned());
        }

        if let Some(mut other) = aliases.remove(&codepoint) {
            names.append(&mut other);
        }

        let mut names = names.into_iter();

        vector.push(Char {
            scalar,
            codepoint,
            name: ByteString(names.next().unwrap()),
            aliases: names.map(ByteString).collect(),
            category,
        });
    }

    #[derive(serde::Deserialize)]
    struct NerdIconInfo<'a> {
        // char: &'a str,
        code: &'a str,
    }

    #[derive(serde::Deserialize)]
    struct NerdFontMetadata {}

    #[derive(serde::Deserialize)]
    struct NerdFontData<'a> {
        #[serde(rename = "METADATA")]
        _metadata: NerdFontMetadata,
        #[serde(flatten, borrow)]
        icons: HashMap<&'a str, NerdIconInfo<'a>>,
    }

    let nerd_font_data =
        std::fs::read_to_string("data/unicode/glyphnames.json").expect("Couldn't read file");
    let nerd_font_data: NerdFontData =
        serde_json::from_str(&nerd_font_data).expect("JSON was not well-formatted");

    'outer: for (name, data) in nerd_font_data.icons {
        if let Ok(code) = u32::from_str_radix(data.code, 16)
            && let Some((category, name)) = name.split_once('-')
        {
            let name = ByteString(name.to_uppercase().replace('_', " "));

            for character in &mut vector {
                if character.codepoint == code {
                    character.aliases.push(name);
                    continue 'outer;
                }
            }

            vector.push(Char {
                scalar: char::from_u32(code).unwrap(),
                codepoint: code,
                name,
                aliases: Vec::new(),
                category: match category {
                    "cod" => Category::Codicons,
                    "custom" => Category::NfCustom,
                    "dev" => Category::Devicons,
                    "fa" | "fae" => Category::FontAwesome,
                    "iec" => Category::IecPowerSymbols,
                    "linux" => Category::FontLogos,
                    "md" => Category::MaterialDesign,
                    "oct" => Category::Octicons,
                    "pl" | "ple" => Category::PowerlineSymbols,
                    "pom" => Category::Pomicons,
                    "seti" => Category::SetiUI,
                    "weather" => Category::WeatherIcons,
                    "extra" | "indent" | "indentation" => Category::SymbolOther,
                    _ => panic!("Unexpected nerd font category: {category}"),
                },
            })
        }
    }

    vector.sort_by_key(|x| x.codepoint);

    for dupe in &vector.iter().chunk_by(|x| x.codepoint) {
        let vec = dupe.1.collect_vec();
        if vec.len() > 1 {
            println!("cargo::warning={vec:?}");
        }
    }

    vector.sort_by_key(|x| x.scalar);

    let out_dir = env::var_os("OUT_DIR").expect("OUT_DIR variable not specified");
    let dest_path = Path::new(&out_dir).join("unicode_data.rs");

    fs::write(
        &dest_path,
        format!(
            "use crate::plugins::unicode::types::*;\n{}",
            const_declaration!(pub DATA = vector)
        ),
    )?;

    parse_emojis();

    Ok(())
}

#[derive(Debug, CompileConst, Clone, Copy, PartialEq, PartialOrd, Default)]
struct UnicodeVersion {
    major: u16,
    minor: u16,
}

impl Display for UnicodeVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

impl From<f64> for UnicodeVersion {
    fn from(value: f64) -> Self {
        let major = value.round();
        let minor = (value - major) * 10.0;

        let major = major as u16;
        let minor = minor as u16;

        Self { major, minor }
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

#[derive(CompileConst, Debug)]
struct EmojiVariant {
    codepoints: String,
    attributes: Vec<String>,
}

enum SkinTone {
    Light,
    MediumLight,
    Medium,
    Dark,
    MediumDark,
}

#[derive(Debug, CompileConst)]
enum VariantType {
    Simple(String),
    Skin {
        base: String,
        rest: String,
    },
    SkinHair {
        base: char,
        extra: char,
    },
    DoubleSkin {
        single: String,
        left: String,
        middle: Vec<String>,
        right: String,
    },
}

impl VariantType {
    fn to_variants(&self) -> Vec<EmojiVariant> {
        match self {
            VariantType::Simple(text) => vec![EmojiVariant {
                codepoints: text.clone(),
                attributes: vec![],
            }],
            VariantType::Skin { base, rest } => {
                let mut variants = Vec::new();
                let (add_selector, rest) = match rest.strip_prefix('\u{FE0F}') {
                    Some(rest) => (true, rest),
                    None => (false, rest.as_str()),
                };

                variants.push(EmojiVariant {
                    codepoints: if add_selector {
                        format!("{base}\u{FE0F}{rest}")
                    } else {
                        format!("{base}{rest}")
                    },
                    attributes: vec![],
                });

                for a in '\u{1F3FB}'..='\u{1F3FF}' {
                    variants.push(EmojiVariant {
                        codepoints: format!("{base}{a}{rest}"),
                        attributes: vec![a.to_string()],
                    })
                }

                variants
            }
            VariantType::SkinHair { base, extra } => {
                let mut variants = Vec::new();

                variants.push(EmojiVariant {
                    codepoints: base.to_string(),
                    attributes: vec![],
                });

                for tone in '\u{1F3FB}'..='\u{1F3FF}' {
                    variants.push(EmojiVariant {
                        codepoints: format!("{base}{tone}"),
                        attributes: vec![tone.to_string()],
                    })
                }

                for hair in '\u{1F9B0}'..='\u{1F9B3}' {
                    variants.push(EmojiVariant {
                        codepoints: format!("{base}\u{200D}{hair}"),
                        attributes: vec![hair.to_string()],
                    });

                    for tone in '\u{1F3FB}'..='\u{1F3FF}' {
                        variants.push(EmojiVariant {
                            codepoints: format!("{base}{tone}\u{200D}{hair}"),
                            attributes: vec![hair.to_string(), tone.to_string()],
                        })
                    }
                }

                for hair in ['\u{1F471}', '\u{1F9D4}'] {
                    // 1F9D4 200D 2642 FE0F
                    variants.push(EmojiVariant {
                        codepoints: if *extra == '\0' {
                            hair.to_string()
                        } else {
                            format!("{hair}\u{200D}{extra}\u{FE0F}")
                        },
                        attributes: vec![hair.to_string()],
                    });

                    for tone in '\u{1F3FB}'..='\u{1F3FF}' {
                        variants.push(EmojiVariant {
                            codepoints: if *extra == '\0' {
                                format!("{hair}{tone}")
                            } else {
                                format!("{hair}{tone}\u{200D}{extra}\u{FE0F}")
                            },
                            attributes: vec![hair.to_string(), tone.to_string()],
                        })
                    }
                }

                variants
            }
            VariantType::DoubleSkin {
                single,
                left,
                middle,
                right,
            } => {
                let mut variants = Vec::new();

                variants.push(EmojiVariant {
                    codepoints: single.clone(),
                    attributes: vec![],
                });

                for a in '\u{1F3FB}'..='\u{1F3FF}' {
                    for b in '\u{1F3FB}'..='\u{1F3FF}' {
                        variants.push(EmojiVariant {
                            codepoints: if a == b {
                                format!("{single}{a}")
                            } else {
                                let mut parts = Vec::new();
                                parts.push(format!("{left}{a}"));
                                parts.extend(middle.iter().map(String::to_owned));
                                parts.push(format!("{right}{b}"));
                                parts.join("\u{200D}")
                            },
                            attributes: vec![a.to_string(), b.to_string()],
                        })
                    }
                }

                variants
            }
        }
    }
}

#[derive(CompileConst)]
struct Emoji2 {
    group: usize,
    subgroup: usize,
    description: String,
    variant_type: VariantType,
}

#[derive(Default, CompileConst)]
struct Emoji {
    group: usize,
    subgroup: usize,
    description: String,
    tags: Vec<String>,
    version: UnicodeVersion,
    variants: Vec<EmojiVariant>,
}

fn parse_emojis() {
    let mut groups = Vec::new();
    let mut subgroups = Vec::new();

    let mut current_group_real = String::new();
    let mut current_group = Wrapping(usize::MAX);
    let mut current_subgroup = Wrapping(usize::MAX);

    // struct EmojiVariant {
    //     codepoints: String,
    //     version: UnicodeVersion,
    //     attributes: Vec<String>,
    // }

    // #[derive(Default)]
    // struct Emoji {
    //     group: usize,
    //     subgroup: usize,
    //     description: String,
    //     variants: Vec<EmojiVariant>,
    // }

    let mut emojis = Vec::<Emoji>::new();

    let mut found_attributes = HashSet::new();

    let file = File::open("data/unicode/emoji-test.txt").unwrap();
    for line in BufReader::new(file).lines().map_while(Result::ok) {
        if line.is_empty() {
            continue;
        }

        if let Some(line) = line.strip_prefix('#') {
            match line.split_once(": ") {
                Some((" group", group)) => {
                    if current_subgroup.0 != usize::MAX {
                        groups.push((current_group_real, std::mem::take(&mut subgroups)));
                        current_subgroup = Wrapping(usize::MAX);
                    }

                    current_group += 1;
                    current_group_real = group.to_owned();
                }
                Some((" subgroup", subgroup)) => {
                    subgroups.push(subgroup.to_owned());
                    current_subgroup += 1;
                }
                _ => {}
            }

            continue;
        }

        let (codepoints, line) = line.split_once(';').unwrap();
        let (qualification, line) = line.split_once("# ").unwrap();

        if qualification.trim() != "fully-qualified" {
            continue;
        }

        let codepoints: Vec<_> = codepoints
            .trim()
            .split(' ')
            .map(|x| u32::from_str_radix(x, 16).unwrap())
            .map(|x| char::from_u32(x).unwrap())
            .collect();

        let codepoints = codepoints.into_iter().collect();

        let (_, line) = line.split_once(' ').unwrap();
        let (version, line) = line.split_once(' ').unwrap();
        let (description, mut attributes) = match line.split_once(": ") {
            Some((a, b)) => (a, b.split(", ").map(str::to_owned).collect()),
            None => (line, vec![]),
        };

        let description = description.to_lowercase();
        let description = if description == "flag" || description == "keycap" {
            let attribute = attributes.pop().unwrap();
            format!("{attribute} {description}")
        } else {
            description
        };

        for attribute in &attributes {
            found_attributes.insert(attribute.to_owned());
        }

        let variant = EmojiVariant {
            codepoints,
            attributes,
        };
        let version = version.parse().unwrap();

        if let Some(emoji) = emojis
            .iter_mut()
            .find(|x| x.description == description && x.group == current_group.0)
        {
            emoji.variants.push(variant);
            if version < emoji.version {
                emoji.version = version;
            }
        } else {
            emojis.push(Emoji {
                group: current_group.0,
                subgroup: current_subgroup.0,
                description,
                tags: vec![],
                version,
                variants: vec![variant],
            });
        }
    }

    if current_subgroup.0 != usize::MAX {
        groups.push((current_group_real, subgroups));
    }

    let mut real_emojis = Vec::new();

    fn is_skin_tone(c: char) -> bool {
        ('\u{1F3FB}'..='\u{1F3FF}').contains(&c)
    }

    #[derive(Deserialize)]
    struct EmojiFromJson {
        label: String,
        emoji: String,
        order: Option<u64>,
        group: Option<u8>,
        subgroup: Option<u8>,
        version: f64,
        #[serde(default)]
        tags: Vec<String>,
        #[serde(default)]
        skins: Vec<EmojiSkin>,
    }

    #[derive(Deserialize)]
    struct EmojiSkin {
        label: String,
        emoji: String,
        tone: EmojiTone,
    }

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum EmojiTone {
        Single(u8),
        Double(u8, u8),
    }

    let emojis_from_json: Vec<EmojiFromJson> =
        serde_json::from_reader(File::open("data/unicode/emojibase.json").unwrap()).unwrap();

    emojis.clear();
    for emoji in emojis_from_json {
        let Some(order) = emoji.order else {
            continue;
        };

        let Some(group) = emoji.group else {
            continue;
        };

        let Some(subgroup) = emoji.subgroup else {
            continue;
        };

        let (label, attributes) = emoji.label.split_once(": ").unwrap_or((&emoji.label, ""));
        let attributes = attributes.split(", ").collect_vec();

        let mut variants = vec![EmojiVariant {
            codepoints: emoji.emoji,
            attributes: vec![],
        }];

        variants.extend(emoji.skins.into_iter().map(|x| EmojiVariant {
            codepoints: x.emoji,
            attributes: vec![],
        }));

        emojis.push(Emoji {
            group: group as usize,
            subgroup: subgroup as usize,
            description: emoji.label,
            tags: emoji.tags,
            version: emoji.version.into(),
            variants,
        })
    }

    for mut emoji in emojis {
        // let tags = emojis_from_json
        //     .iter()
        //     .find(|x| x.label == emoji.description)
        //     .map(|x| x.tags.clone())
        //     .unwrap_or_default();

        let tags = Vec::new();

        let base = &emoji.variants[0].codepoints;

        let variant_type = if emoji.variants.len() == 26
            && let Some((char1, middle, char2)) = emoji
                .variants
                .iter()
                .map(|x| x.codepoints.split('\u{200D}').collect_vec())
                .filter(|x| x.len() > 1)
                .flat_map(|mut x| {
                    let a = x.remove(0);
                    let b = x.pop().unwrap();
                    if let Some(a) = a.strip_suffix(is_skin_tone)
                        && let Some(b) = b.strip_suffix(is_skin_tone)
                    {
                        return Some((a, x, b));
                    }

                    None
                })
                .next()
        {
            VariantType::DoubleSkin {
                single: base.to_owned(),
                left: char1.to_owned(),
                middle: middle.into_iter().map(|x| x.to_owned()).collect(),
                right: char2.to_owned(),
            }
        } else if emoji.variants.len() == 42 {
            let extra = emoji
                .variants
                .iter()
                .flat_map(|x| x.codepoints.chars().nth(2))
                .find(|x| !('\u{1F9B0}'..='\u{1F9B3}').contains(x) && *x != '\u{200D}')
                .unwrap_or_default();

            VariantType::SkinHair {
                base: base.chars().next().unwrap(),
                extra,
            }
        } else if emoji.variants.len() == 6
            && emoji.variants.iter().skip(1).all(|x| {
                let mut chars = x.codepoints.chars();
                let first = chars.next().unwrap();
                let Some(tone) = chars.next() else {
                    return false;
                };
                if !is_skin_tone(tone) {
                    return false;
                }

                let rest = chars.collect::<String>();

                match base.chars().nth(1) {
                    Some('\u{FE0F}') => {
                        let mut chars = base.chars();
                        let base_first = chars.next().unwrap();
                        chars.next();
                        let base_rest = chars.collect::<String>();

                        if first == base_first && rest == base_rest {
                            return true;
                        }
                    }
                    _ => {
                        if base == &format!("{first}{rest}") {
                            return true;
                        }
                    }
                }

                false
            })
        {
            VariantType::Skin {
                base: base.chars().take(1).collect(),
                rest: base.chars().skip(1).collect(),
            }
        } else if emoji.variants.len() == 1 {
            VariantType::Simple(base.clone())
        } else {
            println!(
                "cargo::warning={} ({}) - {base}",
                emoji.description,
                emoji.variants.len()
            );
            continue;
        };

        let variants = variant_type.to_variants();

        if !variants.is_empty() {
            real_emojis.push(Emoji {
                variants,
                tags,
                ..emoji
            });
        }
    }

    let out_dir = env::var_os("OUT_DIR").expect("OUT_DIR variable not specified");
    let dest_path = Path::new(&out_dir).join("unicode_emojis.rs");

    let const_declarations = [
        const_declaration!(pub GROUPS = groups),
        const_declaration!(pub EMOJIS = real_emojis),
    ]
    .join("\n");

    fs::write(
        &dest_path,
        format!("use crate::plugins::emoji::types::*;\n{const_declarations}",),
    )
    .unwrap();
}
