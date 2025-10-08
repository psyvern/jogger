mod char;

use std::ops::Range;

use itertools::Itertools;

use crate::plugins::unicode::char::Char;

use crate::interface::{Context, Entry, EntryAction, EntryIcon, Plugin};

include!(concat!(env!("OUT_DIR"), "/data.rs"));

#[derive(Debug)]
pub struct Unicode {}

impl Unicode {
    pub fn new() -> Self {
        Self {}
    }
}

fn titlecase(s: &str) -> String {
    let mut last = ' ';
    let mut result = String::new();

    for c in s.chars() {
        match c {
            '<' => result.push_str("&lt;"),
            '>' => result.push_str("&gt;"),
            _ => result.push(match last {
                ' ' | '-' | '<' => c,
                _ => c.to_ascii_lowercase(),
            }),
        }

        last = c;
    }

    result
}

fn titlecase2(s: &str, color: Range<usize>) -> String {
    let mut last = ' ';
    let mut result = String::new();

    for (i, c) in s.chars().enumerate() {
        if i == color.start && i != color.end {
            result.push_str("<span color=\"#A2C9FE\">");
        }

        match c {
            '<' => result.push_str("&lt;"),
            '>' => result.push_str("&gt;"),
            _ => result.push(match last {
                ' ' | '-' | '<' => c,
                _ => c.to_ascii_lowercase(),
            }),
        }

        if i + 1 == color.end {
            result.push_str("</span>");
        }

        last = c;
    }

    result
}

fn char_representation(c: char) -> char {
    match c {
        ' ' => '⎵',
        '\0'..'\u{20}' => char::from_u32(c as u32 + 0x2400).unwrap_or(c),
        c if c.is_whitespace() || c.is_control() => '�',
        c => c,
    }
}

impl Plugin for Unicode {
    fn name(&self) -> &str {
        "Characters"
    }

    fn icon(&self) -> Option<&str> {
        Some("accessories-character-map")
    }

    fn search(&self, query: &str, _: &mut Context) -> Vec<Entry> {
        if query.chars().exactly_one().is_ok() {
        } else if let Ok(codepoint) = u32::from_str_radix(query, 16) {
            return DATA
                .binary_search_by(|x| x.codepoint.cmp(&codepoint))
                .map(|x| &DATA[x])
                .map(|x| Entry {
                    name: titlecase(x.name),
                    tag: Some(x.category.to_string()),
                    description: Some(format!(
                        "<span color=\"#A2C9FE\">{:04X}</span>",
                        x.codepoint
                    )),
                    icon: EntryIcon::Character(char_representation(x.scalar)),
                    small_icon: EntryIcon::None,
                    actions: vec![EntryAction::Copy(x.name.to_owned()).into()],
                    id: "".to_owned(),
                })
                .into_iter()
                .collect();
        } else if query.chars().all(|c| {
            matches!(c,
                ' ' | '(' | ')' | ',' | '-' | '0'..='9' | '<' | '>' | 'A'..='Z' | 'a'..='z'
            )
        }) {
            let query = query.to_uppercase();
            return DATA
                .iter()
                .flat_map(|x| x.name.find(&query).map(|i| (i, x)))
                .take(64)
                .map(|(i, x)| Entry {
                    name: titlecase2(x.name, i..(i + query.len())),
                    tag: Some(x.category.to_string()),
                    description: Some(format!("{:04X}", x.codepoint)),
                    icon: EntryIcon::Character(char_representation(x.scalar)),
                    small_icon: EntryIcon::None,
                    actions: vec![EntryAction::Copy(x.scalar.to_string()).into()],
                    id: "".to_owned(),
                })
                .collect();
        }

        query
            .chars()
            .map(|c| {
                DATA.binary_search_by(|x| x.scalar.cmp(&c))
                    .map(|x| &DATA[x])
                    .map(|x| Entry {
                        name: titlecase(x.name),
                        tag: Some(x.category.to_string()),
                        description: Some(format!("{:04X}", x.codepoint)),
                        icon: EntryIcon::Character(char_representation(x.scalar)),
                        small_icon: EntryIcon::None,
                        actions: vec![EntryAction::Copy(x.name.to_owned()).into()],
                        id: "".to_owned(),
                    })
                    .unwrap_or(Entry {
                        name: "&lt;unknown&gt;".to_owned(),
                        tag: Some("??".to_owned()),
                        description: Some(format!("{:04X}", c as u32)),
                        icon: EntryIcon::Character(c),
                        small_icon: EntryIcon::None,
                        actions: vec![],
                        id: "".to_owned(),
                    })
            })
            .collect()
    }

    fn has_entry(&self) -> bool {
        true
    }
}
