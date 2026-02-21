use bstr::ByteSlice;
mod data;
mod types;

use crate::interface::{
    Context, Entry, EntryAction, EntryIcon, FormatStyle, FormattedString, Plugin,
};
use crate::plugins::unicode::data::DATA;

#[derive(Debug)]
pub struct Unicode {}

impl Unicode {
    pub fn new(_: &Context) -> Self {
        Self {}
    }
}

pub fn titlecase(s: &[u8]) -> String {
    let mut last = b' ';
    let mut result = String::new();

    for &c in s {
        result.push(match last {
            b' ' | b'-' | b'<' => c as char,
            _ => (c as char).to_ascii_lowercase(),
        });

        last = c;
    }

    result
}

fn is_unicode_name(c: char) -> bool {
    matches!(c,
        ' ' | '(' | ')' | ',' | '-' | '0'..='9' | '<' | '>' | 'A'..='Z' | 'a'..='z'
    )
}

impl Plugin for Unicode {
    fn name(&self) -> &str {
        "Characters"
    }

    fn icon(&self) -> Option<&str> {
        Some("accessories-character-map")
    }

    fn search(&self, query: &str, _: &Context) -> Vec<Entry> {
        if query.is_empty() {
            // TODO: add recents
        } else if query.chars().count() == 1 {
        } else if query.chars().all(is_unicode_name) {
            let iter1 = u32::from_str_radix(query, 16)
                .into_iter()
                .flat_map(|codepoint| DATA.binary_search_by(|x| x.codepoint.cmp(&codepoint)))
                .map(|x| &DATA[x])
                .map(|x| Entry {
                    name: FormattedString::plain(titlecase(x.name)),
                    tag: Some(FormattedString::plain(x.category.to_string())),
                    description: Some(FormattedString::from_style(
                        format!("{:04X}", x.codepoint),
                        FormatStyle::Highlight,
                    )),
                    icon: EntryIcon::Text(x.representation().to_string()),
                    actions: vec![
                        EntryAction::Copy(x.name.to_str_lossy().into_owned(), None).into(),
                    ],
                    ..Default::default()
                });

            let query = Vec::from(query.to_uppercase());
            let len = query.len();
            let finder = bstr::Finder::new(&query);

            let iter2 = DATA.iter().flat_map(|x| {
                if let Some(i) = finder.find(x.name) {
                    Some(Entry {
                        name: FormattedString {
                            text: titlecase(x.name),
                            ranges: vec![(FormatStyle::Highlight, i..(i + len))],
                        },
                        tag: Some(FormattedString::plain(x.category.to_string())),
                        description: Some(FormattedString::plain(format!("{:04X}", x.codepoint))),
                        icon: EntryIcon::Text(x.representation().to_string()),
                        actions: vec![EntryAction::Copy(x.scalar.to_string(), None).into()],
                        ..Default::default()
                    })
                } else if let Some((alias, i)) = x
                    .aliases
                    .iter()
                    .take_while(|x| !x.is_empty())
                    .flat_map(|x| finder.find(x).map(|i| (x, i)))
                    .next()
                {
                    Some(Entry {
                        name: FormattedString::plain(titlecase(x.name)),
                        tag: Some(FormattedString {
                            text: titlecase(alias),
                            ranges: vec![(FormatStyle::Highlight, i..(i + len))],
                        }),
                        description: Some(FormattedString::plain(format!("{:04X}", x.codepoint))),
                        icon: EntryIcon::Text(x.representation().to_string()),
                        actions: vec![EntryAction::Copy(x.scalar.to_string(), None).into()],
                        ..Default::default()
                    })
                } else {
                    None
                }
            });

            return iter1.chain(iter2).take(128).collect();
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
                        icon: EntryIcon::Text(x.representation().to_string()),
                        actions: vec![
                            EntryAction::Copy(x.name.to_str_lossy().into_owned(), None).into(),
                        ],
                        ..Default::default()
                    })
                    .unwrap_or(Entry {
                        name: FormattedString::plain("<unknown>"),
                        description: Some(FormattedString::plain(format!("{:04X}", c as u32))),
                        icon: EntryIcon::Text(c.to_string()),
                        ..Default::default()
                    })
            })
            .collect()
    }
}
