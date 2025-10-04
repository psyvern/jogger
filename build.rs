use std::env;
use std::fs;
use std::fs::File;
use std::io::{BufRead, BufReader, Result};
use std::path::Path;

// defines Char struct
include!("src/plugins/unicode/char.rs");

fn main() -> Result<()> {
    let mut vector: Vec<Char> = Vec::new();
    let data_path = env::var_os("DATA_PATH").unwrap_or_else(|| "UnicodeData.txt".into());

    let data_file = File::open(data_path)?;
    for line_result in BufReader::new(data_file).lines() {
        let line = line_result?;
        let line = Box::leak(line.into_boxed_str());
        let mut csv_iter = line.split(';');
        let codepoint = csv_iter.next().expect("data is corrupt");
        let name = {
            const CONTROL_VALUE: &str = "<control>";
            let mut name_in_file = csv_iter.next().expect("data is corrupt");
            if name_in_file == CONTROL_VALUE {
                let mut i = 0;
                while i < 8 {
                    csv_iter.next();
                    i += 1;
                }
                name_in_file = csv_iter.next().expect("data is corrupt");
            }
            if name_in_file.is_empty() {
                if codepoint == "0080" {
                    "PADDING CHARACTER"
                } else if codepoint == "0081" {
                    "HIGH OCTET PRESET;"
                } else if codepoint == "0084" {
                    "INDEX"
                } else if codepoint == "0099" {
                    "SINGLE GRAPHIC CHARACTER INTRODUCER"
                } else {
                    CONTROL_VALUE
                }
            } else {
                name_in_file
            }
        };
        let hex_codepoint = u32::from_str_radix(codepoint, 16).expect("number is corrupt");
        if let Some(scalar) = std::char::from_u32(hex_codepoint) {
            vector.push(Char {
                scalar,
                codepoint,
                name,
            });
        }
    }

    let data = format!("static DATA: [Char; {}] = {:?};", vector.len(), vector);

    let out_dir = env::var_os("OUT_DIR").expect("OUT_DIR variable not specified");
    let dest_path = Path::new(&out_dir).join("data.rs");
    fs::write(&dest_path, data)?;
    println!("cargo:rerun-if-changed=build.rs");

    Ok(())
}
