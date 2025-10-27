mod char;

use itertools::Itertools;

use crate::plugins::unicode::char::Char;

use crate::interface::{
    Context, Entry, EntryAction, EntryIcon, FormatStyle, FormattedString, Plugin,
};

include!(concat!(env!("OUT_DIR"), "/data.rs"));

#[derive(Debug)]
pub struct Unicode {}

impl Unicode {
    pub fn new(_: &mut Context) -> Self {
        Self {}
    }
}

fn titlecase(s: &str) -> String {
    let mut last = ' ';
    let mut result = String::new();

    for c in s.chars() {
        result.push(match last {
            ' ' | '-' | '<' => c,
            _ => c.to_ascii_lowercase(),
        });

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
                    name: FormattedString::plain(titlecase(x.name)),
                    tag: Some(FormattedString::plain(x.category.to_string())),
                    description: Some(FormattedString::from_style(
                        format!("{:04X}", x.codepoint),
                        FormatStyle::Highlight,
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
                    name: FormattedString {
                        text: titlecase(x.name),
                        ranges: vec![(FormatStyle::Highlight, i..(i + query.len()))],
                    },
                    tag: Some(FormattedString::plain(x.category.to_string())),
                    description: Some(FormattedString::plain(format!("{:04X}", x.codepoint))),
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
                        name: FormattedString::plain(titlecase(x.name)),
                        tag: Some(FormattedString::plain(x.category.to_string())),
                        description: Some(FormattedString::plain(format!("{:04X}", x.codepoint))),
                        icon: EntryIcon::Character(char_representation(x.scalar)),
                        small_icon: EntryIcon::None,
                        actions: vec![EntryAction::Copy(x.name.to_owned()).into()],
                        id: "".to_owned(),
                    })
                    .unwrap_or(Entry {
                        name: FormattedString::plain("<unknown>"),
                        tag: Some(FormattedString::plain("??")),
                        description: Some(FormattedString::plain(format!("{:04X}", c as u32))),
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
