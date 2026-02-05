use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::env;
use std::fmt::Debug;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::io::{BufRead, BufReader, Result};
use std::num::Wrapping;
use std::path::Path;

use itertools::Itertools;

// defines Char struct
include!("src/plugins/unicode/char.rs");

fn main() -> Result<()> {
    println!("cargo:rerun-if-changed=data/unicode/UnicodeData.txt");
    println!("cargo:rerun-if-changed=data/unicode/NameAliases.txt");
    println!("cargo:rerun-if-changed=data/unicode/emoji-sequences.txt");
    println!("cargo:rerun-if-changed=data/unicode/emoji-test.txt");
    println!("cargo:rerun-if-changed=data/unicode/glyphnames.json");

    println!("cargo:rerun-if-changed=build.rs");

    #[derive(Debug)]
    struct Char {
        scalar: char,
        codepoint: u32,
        name: String,
        aliases: [String; 5],
        category: Category,
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

    impl Display for Char {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(
                f,
                r#"Char {{ scalar: {:?}, codepoint: {}, name: b"{}", aliases: [{}], category: Category::{} }}"#,
                self.scalar,
                self.codepoint,
                self.name,
                self.aliases.iter().map(|x| format!("b\"{x}\"")).join(", "),
                self.category
            )
        }
    }

    let mut aliases_map = HashMap::<u32, Vec<String>>::new();

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

        aliases_map
            .entry(codepoint)
            .or_default()
            .push(alias.to_owned());
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

        let mut aliases = Vec::new();
        if category != Category::OtherControl {
            aliases.push(name.to_owned());
        }

        if let Some(mut other) = aliases_map.remove(&codepoint) {
            aliases.append(&mut other);
        }

        let mut aliases = aliases.into_iter();

        vector.push(Char {
            scalar,
            codepoint,
            name: aliases.next().unwrap(),
            aliases: aliases
                .chain(std::iter::repeat(String::new()))
                .next_array()
                .unwrap(),
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

    for (name, data) in nerd_font_data.icons {
        if let Ok(code) = u32::from_str_radix(data.code, 16) {
            if let Some((category, name)) = name.split_once('-') {
                vector.push(Char {
                    scalar: char::from_u32(code).unwrap(),
                    codepoint: code,
                    name: name.to_uppercase().replace('_', " "),
                    aliases: Default::default(),
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
    }

    vector.sort_by_key(|x| x.scalar);
    let mut set = BTreeSet::new();
    for c in vector {
        set.insert(c);
    }

    let out_dir = env::var_os("OUT_DIR").expect("OUT_DIR variable not specified");
    let dest_path = Path::new(&out_dir).join("unicode_data.rs");

    fs::write(
        &dest_path,
        format!(
            "use crate::plugins::unicode::char::Category;\nstatic DATA: [Char; {}] = [{}];",
            set.len(),
            set.into_iter().map(|x| x.to_string()).join(", "),
        ),
    )?;

    parse_emojis();

    Ok(())
}

#[derive(Debug)]
struct UnicodeVersion {
    major: u16,
    minor: u16,
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

struct EmojiVariant {
    codepoints: Vec<u32>,
    version: UnicodeVersion,
    attributes: Vec<String>,
}

impl Debug for EmojiVariant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "EmojiVariant {{ codepoints: vec!{:?}, version: {:?}, attributes: vec!{:?} }}",
            self.codepoints, self.version, self.attributes
        )
    }
}

#[derive(Default)]
struct Emoji {
    group: usize,
    subgroup: usize,
    description: String,
    variants: Vec<EmojiVariant>,
}

impl Debug for Emoji {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Emoji {{ group: {:?}, subgroup: {:?}, description: {:?}, variants: vec!{:?} }}",
            self.group, self.subgroup, self.description, self.variants
        )
    }
}

fn parse_emojis() {
    let mut groups = Vec::new();
    let mut subgroups = Vec::new();

    let mut current_group_real = String::new();
    let mut current_group = Wrapping(usize::MAX);
    let mut current_subgroup = Wrapping(usize::MAX);

    let mut emojis = Vec::new();

    let mut current_emoji = Emoji::default();

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

        let codepoints = codepoints
            .trim()
            .split(' ')
            .map(|x| u32::from_str_radix(x, 16))
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap();

        let (_, line) = line.split_once(' ').unwrap();
        let (version, line) = line.split_once(' ').unwrap();
        let (description, attributes) = match line.split_once(": ") {
            Some((a, b)) => (a, b.split(", ").map(str::to_owned).collect()),
            None => (line, vec![]),
        };

        let variant = EmojiVariant {
            codepoints,
            version: version.parse().unwrap(),
            attributes,
        };

        if current_emoji.description == description {
            current_emoji.variants.push(variant);
        } else {
            if !current_emoji.description.is_empty() {
                emojis.push(current_emoji);
            }

            current_emoji = Emoji {
                group: current_group.0,
                subgroup: current_subgroup.0,
                description: description.to_owned(),
                variants: vec![variant],
            }
        }
    }

    if !current_emoji.description.is_empty() {
        emojis.push(current_emoji);
    }

    if current_subgroup.0 != usize::MAX {
        groups.push((current_group_real, subgroups));
    }

    let out_dir = env::var_os("OUT_DIR").expect("OUT_DIR variable not specified");
    let dest_path = Path::new(&out_dir).join("unicode_emojis.rs");

    fs::write(
        &dest_path,
        format!(
            "\
use crate::plugins::emoji::types::*;

pub const GROUPS: [&'static str; {}] = {:?};

pub(crate) fn get_data() -> Vec<Emoji> {{
    vec!{emojis:?}
}}
",
            groups.len(),
            groups.into_iter().map(|x| x.0).collect_vec()
        ),
    )
    .unwrap();
}
