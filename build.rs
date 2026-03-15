use std::cmp::Ordering;
use std::collections::HashMap;
use std::env;
use std::fmt::Debug;
use std::fs;
use std::fs::File;
use std::io::{BufRead, BufReader, Result};
use std::path::Path;

use const_gen::*;
use itertools::Itertools;
use serde::Deserialize;

include!("src/plugins/unicode/types.rs");

fn main() -> Result<()> {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=data");

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
    #[derive(Deserialize)]
    struct Messages {
        groups: Vec<Message>,
        subgroups: Vec<Message>,
    }

    #[derive(Deserialize)]
    struct Message {
        message: String,
    }

    let messages_raw: Messages =
        serde_json::from_reader(File::open("data/emojibase/messages.raw.json").unwrap()).unwrap();

    let groups = messages_raw
        .groups
        .into_iter()
        .map(|x| x.message)
        .collect_vec();
    let subgroups = messages_raw
        .subgroups
        .into_iter()
        .map(|x| x.message)
        .collect_vec();

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
        serde_json::from_reader(File::open("data/emojibase/data.raw.json").unwrap()).unwrap();

    let emojis = emojis_from_json
        .into_iter()
        .flat_map(|emoji| {
            emoji.order?;
            let group = emoji.group?;
            let subgroup = emoji.subgroup?;

            let mut variants = vec![EmojiVariant {
                codepoints: emoji.emoji,
                attributes: vec![],
            }];

            variants.extend(emoji.skins.into_iter().map(|x| {
                let (_, attributes) = x.label.split_once(": ").unwrap_or((&x.label, ""));

                EmojiVariant {
                    codepoints: x.emoji,
                    attributes: attributes.split(", ").map(str::to_owned).collect(),
                }
            }));

            Some(Emoji {
                group: group as usize,
                subgroup: subgroup as usize,
                description: emoji.label,
                tags: emoji.tags,
                version: emoji.version.into(),
                variants,
            })
        })
        .collect_vec();

    let out_dir = env::var_os("OUT_DIR").expect("OUT_DIR variable not specified");
    let dest_path = Path::new(&out_dir).join("unicode_emojis.rs");

    let const_declarations = [
        const_declaration!(pub GROUPS = groups),
        const_declaration!(pub SUBGROUPS = subgroups),
        const_declaration!(pub EMOJIS = emojis),
    ]
    .join("\n");

    fs::write(
        &dest_path,
        format!("use crate::plugins::emoji::types::*;\n{const_declarations}",),
    )
    .unwrap();
}
