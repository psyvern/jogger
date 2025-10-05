use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::fs::File;
use std::io::{BufRead, BufReader, Result};
use std::path::Path;

use itertools::Itertools;

// defines Char struct
include!("src/plugins/unicode/char.rs");

fn main() -> Result<()> {
    #[derive(Debug)]
    struct Char {
        scalar: char,
        codepoint: u32,
        name: String,
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

    let mut vector: Vec<Char> = Vec::new();
    let data_path = env::var_os("DATA_PATH").unwrap_or_else(|| "UnicodeData.txt".into());

    let data_file = File::open(data_path)?;
    for line_result in BufReader::new(data_file).lines() {
        let line = line_result?;
        let line = Box::leak(line.into_boxed_str());
        let mut csv_iter = line.split(';');
        let codepoint = u32::from_str_radix(csv_iter.next().expect("data is corrupt"), 16)
            .expect("data is corrupt");

        const CONTROL_VALUE: &str = "<control>";
        let name = csv_iter.next().expect("data is corrupt");

        let category = csv_iter.next().expect("data is corrupt");
        let category = category.parse().expect("Unrecognised category");

        let name = {
            let mut name_in_file = name;
            if name == CONTROL_VALUE {
                let mut i = 0;
                while i < 7 {
                    csv_iter.next();
                    i += 1;
                }
                name_in_file = csv_iter.next().expect("data is corrupt");
            }
            if name_in_file.is_empty() {
                if codepoint == 0x80 {
                    "PADDING CHARACTER"
                } else if codepoint == 0x81 {
                    "HIGH OCTET PRESET"
                } else if codepoint == 0x84 {
                    "INDEX"
                } else if codepoint == 0x99 {
                    "SINGLE GRAPHIC CHARACTER INTRODUCER"
                } else {
                    CONTROL_VALUE
                }
            } else {
                name_in_file
            }
        };

        if let Some(scalar) = char::from_u32(codepoint) {
            vector.push(Char {
                scalar,
                codepoint,
                name: name.to_owned(),
                category,
            });
        }
    }

    #[derive(serde::Deserialize)]
    struct NerdIconInfo<'a> {
        char: &'a str,
        code: &'a str,
    }

    #[derive(serde::Deserialize)]
    struct NerdFontMetadata {
        website: String,
        #[serde(rename = "development-website")]
        development_website: String,
        version: String,
        date: String,
    }

    #[derive(serde::Deserialize)]
    struct NerdFontData<'a> {
        #[serde(rename = "METADATA")]
        _metadata: NerdFontMetadata,
        #[serde(flatten, borrow)]
        icons: HashMap<&'a str, NerdIconInfo<'a>>,
    }

    let data_path = env::var_os("GLYPH_PATH").unwrap_or_else(|| "glyphnames.json".into());
    let nerd_font_data = std::fs::read_to_string(data_path).expect("Couldn't read file");
    let nerd_font_data: NerdFontData =
        serde_json::from_str(&nerd_font_data).expect("JSON was not well-formatted");

    for x in nerd_font_data
        .icons
        .keys()
        .map(|x| x.split_once('-').map(|x| x.0))
        .collect::<std::collections::HashSet<_>>()
    {
        println!("{x:?}",);
    }

    for (name, data) in nerd_font_data.icons {
        if let Ok(code) = u32::from_str_radix(data.code, 16) {
            if let Some((category, name)) = name.split_once('-') {
                vector.push(Char {
                    scalar: char::from_u32(code).unwrap(),
                    codepoint: code,
                    name: name.to_uppercase().replace('_', " "),
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

    let data = format!(
        "
        use crate::plugins::unicode::char::Category;
        static DATA: [Char; {}] = [{}];
        ",
        set.len(),
        set.into_iter()
            .map(|x| {
                format!(
                    "Char {{ scalar: {:?}, codepoint: {}, name: {:?}, category: Category::{} }}",
                    x.scalar, x.codepoint, x.name, x.category
                )
            })
            .join(", "),
    );

    let out_dir = env::var_os("OUT_DIR").expect("OUT_DIR variable not specified");
    let dest_path = Path::new(&out_dir).join("data.rs");
    fs::write(&dest_path, data)?;
    println!("cargo:rerun-if-changed=build.rs");

    Ok(())
}
