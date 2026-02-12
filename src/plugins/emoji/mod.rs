mod data;
mod types;

use std::ops::Range;

use gtk::gdk::Key;
use gtk::gdk::ModifierType;
use itertools::Itertools;

use crate::plugins::emoji::data::EMOJIS;
use crate::plugins::emoji::data::GROUPS;

use crate::interface::{
    Context, Entry, EntryAction, EntryIcon, FormatStyle, FormattedString, Plugin,
};

#[derive(Debug)]
pub struct Emojis {}

impl Emojis {
    pub fn new(_: &Context) -> Self {
        Self {}
    }
}

pub fn titlecase(s: &str) -> String {
    let mut last = ' ';
    let mut result = String::new();

    for c in s.chars() {
        result.push(match last {
            ' ' | '-' | '<' => c.to_ascii_uppercase(),
            _ => c,
        });

        last = c;
    }

    result
}

impl Plugin for Emojis {
    fn name(&self) -> &str {
        "Emojis"
    }

    fn icon(&self) -> Option<&str> {
        Some("face-smile-big")
    }

    fn search(&self, query: &str, _: &Context) -> Vec<Entry> {
        if query.chars().next().is_some_and(|x| x >= '\x7F') {
            for emoji in EMOJIS {
                if emoji.variants.iter().any(|x| x.codepoints == query) {
                    return emoji
                        .variants
                        .iter()
                        .map(|x| Entry {
                            name: FormattedString::plain(titlecase(emoji.description)),
                            tag: Some(FormattedString::plain(x.version.to_string())),
                            description: Some(FormattedString::plain(x.attributes.join(", "))),
                            icon: EntryIcon::Text(x.codepoints.to_owned()),
                            actions: vec![EntryAction::Copy(x.codepoints.to_owned(), None).into()],
                            ..Default::default()
                        })
                        .collect();
                }
            }
        }

        let query = query.to_lowercase();

        let mut emojis = EMOJIS
            .iter()
            .map(|x| (x, Vec::<Range<_>>::new(), Vec::<Range<_>>::new()))
            .collect_vec();

        for word in query.split_whitespace() {
            if let Some(word) = word.strip_prefix('#') {
                let len = word.len();

                emojis.retain_mut(|(emoji, _, ranges)| {
                    let last = ranges.last().map(|x| x.end).unwrap_or(0);

                    if let Some(index) = GROUPS[emoji.group].0.to_lowercase()[last..].find(word) {
                        ranges.push(last + index..last + index + len);
                        true
                    } else {
                        false
                    }
                });
            } else {
                emojis.retain_mut(|(emoji, ranges, _)| {
                    let len = word.len();

                    let last = ranges.last().map(|x| x.end).unwrap_or(0);

                    if let Some(index) = emoji.description[last..].find(word) {
                        ranges.push(last + index..last + index + len);
                        true
                    } else {
                        false
                    }
                });
            }
        }

        emojis
            .into_iter()
            .map(|(x, ranges, group_ranges)| {
                let first = &x.variants[0];
                Entry {
                    name: FormattedString {
                        text: titlecase(x.description),
                        ranges: ranges
                            .into_iter()
                            .map(|x| (FormatStyle::Highlight, x))
                            .collect(),
                    },
                    tag: if x.variants.len() > 1 {
                        Some(FormattedString::plain(format!(
                            "{} variants",
                            x.variants.len()
                        )))
                    } else {
                        None
                    },
                    description: Some(FormattedString {
                        text: GROUPS[x.group].0.to_owned(),
                        ranges: group_ranges
                            .into_iter()
                            .map(|x| (FormatStyle::Highlight, x))
                            .collect(),
                    }),
                    icon: EntryIcon::Text(first.codepoints.to_owned()),
                    actions: if x.variants.len() > 1 {
                        vec![
                            EntryAction::Copy(first.codepoints.to_owned(), None).into(),
                            (
                                EntryAction::Write {
                                    text: first.codepoints.to_owned(),
                                    description: "Variants...".into(),
                                    icon: "edit-paste-style".into(),
                                },
                                Key::Return,
                                ModifierType::SHIFT_MASK,
                            ),
                        ]
                    } else {
                        vec![EntryAction::Copy(first.codepoints.to_owned(), None).into()]
                    },
                    ..Default::default()
                }
            })
            .take(128)
            .collect()
    }

    fn has_entry(&self) -> bool {
        true
    }
}
