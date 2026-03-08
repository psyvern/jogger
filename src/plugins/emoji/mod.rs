mod data;
mod types;

use std::ops::Range;

use gtk::gdk::Key;
use gtk::gdk::ModifierType;
use itertools::Itertools;

use crate::interface::EntryAction;
use crate::plugins::emoji::data::EMOJIS;
use crate::plugins::emoji::data::GROUPS;

use crate::interface::{Context, Entry, EntryIcon, FormatStyle, FormattedString, Plugin};

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
                            // tag: Some(FormattedString::plain(x.version.to_string())),
                            description: Some(FormattedString::plain(x.attributes.join(", "))),
                            icon: EntryIcon::Text(x.codepoints.to_owned()),
                            actions: vec![EntryAction {
                                icon: "edit-copy".into(),
                                name: "Copy".into(),
                                function: EntryAction::copy(x.codepoints),
                                ..Default::default()
                            }],
                            ..Default::default()
                        })
                        .collect();
                }
            }
        }

        let query = query.to_lowercase();

        let mut emojis = EMOJIS
            .iter()
            .map(|x| {
                (
                    x,
                    Vec::<Range<_>>::new(),
                    Vec::<Range<_>>::new(),
                    Vec::<(usize, Range<_>)>::new(),
                    Vec::<Range<_>>::new(),
                )
            })
            .collect_vec();

        for word in query.split_whitespace() {
            if let Some(word) = word.strip_prefix('#') {
                let len = word.len();

                emojis.retain_mut(|(emoji, _, ranges, _, subgroup_ranges)| {
                    let last = ranges.last().map(|x| x.end).unwrap_or(0);
                    let last2 = subgroup_ranges.last().map(|x| x.end).unwrap_or(0);

                    if let Some(index) = GROUPS[emoji.group].0[last..].to_lowercase().find(word) {
                        ranges.push(last + index..last + index + len);
                        true
                    } else if let Some(index) = GROUPS[emoji.group].1[emoji.subgroup][last2..]
                        .to_lowercase()
                        .find(word)
                    {
                        subgroup_ranges.push(last2 + index..last2 + index + len);
                        true
                    } else {
                        false
                    }
                });
            } else {
                emojis.retain_mut(|(emoji, ranges, _, tag_ranges, _)| {
                    let len = word.len();

                    let last = ranges.last().map(|x| x.end).unwrap_or(0);

                    if let Some(index) = emoji.description[last..].find(word) {
                        ranges.push(last + index..last + index + len);
                        true
                    } else if let Some((index, start)) = emoji
                        .tags
                        .iter()
                        .enumerate()
                        .filter(|(x, _)| !tag_ranges.iter().any(|(y, _)| x == y))
                        .flat_map(|(x, y)| y.find(word).map(|y| (x, y)))
                        .next()
                    {
                        tag_ranges.push((index, start..start + len));
                        true
                    } else {
                        false
                    }
                });
            }
        }

        emojis
            .into_iter()
            .map(|(x, ranges, group_ranges, tag_ranges, subgroup_ranges)| {
                let first = &x.variants[0];
                Entry {
                    name: FormattedString {
                        text: titlecase(x.description),
                        ranges: ranges
                            .into_iter()
                            .map(|x| (FormatStyle::Highlight, x))
                            .collect(),
                    },
                    tag: if tag_ranges.is_empty() {
                        None
                    } else {
                        let (_, text, ranges) = tag_ranges
                            .into_iter()
                            .map(|(index, range)| (x.tags[index], range))
                            .fold(
                                (0, String::new(), Vec::new()),
                                |(mut start, mut buffer, mut ranges), (tag, range)| {
                                    if start != 0 {
                                        start += "  ·  ".len();
                                        buffer.push_str("  ·  ");
                                    }

                                    ranges.push((
                                        FormatStyle::Highlight,
                                        (start + range.start)..(start + range.end),
                                    ));
                                    start += tag.len();
                                    buffer.push_str(tag);

                                    (start, buffer, ranges)
                                },
                            );

                        Some(FormattedString { text, ranges })
                    },
                    description: {
                        let first_part = format!("{}  ·  ", GROUPS[x.group].0);
                        let len = first_part.len();

                        Some(FormattedString {
                            text: format!(
                                "{first_part}{}",
                                titlecase(&GROUPS[x.group].1[x.subgroup].split('-').join(" "))
                            ),
                            ranges: group_ranges
                                .into_iter()
                                .map(|x| (FormatStyle::Highlight, x))
                                .chain(
                                    subgroup_ranges.into_iter().map(|x| {
                                        (FormatStyle::Highlight, x.start + len..x.end + len)
                                    }),
                                )
                                .collect(),
                        })
                    },
                    icon: EntryIcon::Text(first.codepoints.to_owned()),
                    actions: if x.variants.len() > 1 {
                        vec![
                            EntryAction {
                                icon: "edit-copy".into(),
                                name: "Copy".into(),
                                function: EntryAction::copy(first.codepoints),
                                ..Default::default()
                            },
                            EntryAction {
                                icon: "edit-paste-style".into(),
                                name: "Variants...".into(),
                                key: Key::Return,
                                modifier: ModifierType::SHIFT_MASK,
                                function: EntryAction::write(first.codepoints),
                            },
                        ]
                    } else {
                        vec![EntryAction {
                            icon: "edit-copy".into(),
                            name: "Copy".into(),
                            function: EntryAction::copy(first.codepoints),
                            ..Default::default()
                        }]
                    },
                    ..Default::default()
                }
            })
            .take(128)
            .collect()
    }
}
