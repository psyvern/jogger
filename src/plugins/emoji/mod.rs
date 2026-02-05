mod data;
mod types;

use crate::plugins::emoji::data::GROUPS;
use crate::plugins::emoji::data::get_data;

use crate::{
    interface::{Context, Entry, EntryAction, EntryIcon, FormatStyle, FormattedString, Plugin},
    plugins::emoji::types::Emoji,
};

#[derive(Debug)]
pub struct Emojis {
    data: Vec<Emoji>,
}

impl Emojis {
    pub fn new(_: &mut Context) -> Self {
        Self { data: get_data() }
    }
}

pub fn titlecase(s: &str) -> String {
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

impl Plugin for Emojis {
    fn name(&self) -> &str {
        "Emojis"
    }

    fn icon(&self) -> Option<&str> {
        Some("face-smile-big")
    }

    fn search(&self, query: &str, _: &mut Context) -> Vec<Entry> {
        let query = query.to_lowercase();

        let query = Vec::from(query);
        let len = query.len();
        let finder = bstr::Finder::new(&query);

        self.data
            .iter()
            .flat_map(|x| finder.find(x.description).map(|i| (x, i)))
            .map(|(x, i)| Entry {
                name: FormattedString {
                    text: titlecase(x.description),
                    ranges: vec![(FormatStyle::Highlight, i..(i + len))],
                },
                tag: Some(FormattedString::plain(x.variants[0].version.to_string())),
                description: Some(FormattedString::plain(GROUPS[x.group])),
                icon: EntryIcon::Text(
                    x.variants[0]
                        .codepoints
                        .iter()
                        .flat_map(|x| char::from_u32(*x))
                        .collect(),
                ),
                actions: vec![EntryAction::Write("mogus".to_string()).into()],
                ..Default::default()
            })
            .take(128)
            .collect()
    }

    fn has_entry(&self) -> bool {
        true
    }
}
