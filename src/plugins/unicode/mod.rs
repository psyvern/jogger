mod char;
use crate::plugins::unicode::char::Char;

use crate::interface::{Context, Entry, EntryAction, EntryIcon, Plugin};
use itertools::Itertools;
use titlecase::Titlecase;

include!(concat!(env!("OUT_DIR"), "/data.rs"));

#[derive(Debug)]
pub struct Unicode {}

impl Unicode {
    pub fn new() -> Self {
        Self {}
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
        if let Ok(query) = query.chars().exactly_one() {
            DATA.iter()
                .find(|x| x.scalar == query)
                .map(|x| Entry {
                    name: x.name.titlecase(),
                    tag: None,
                    description: Some(x.codepoint.to_owned()),
                    icon: EntryIcon::Character(
                        if x.scalar.is_whitespace() || x.scalar.is_control() {
                            '�'
                        } else {
                            x.scalar
                        },
                    ),
                    small_icon: EntryIcon::None,
                    actions: vec![EntryAction::Copy(x.scalar.to_string())],
                    id: "".to_owned(),
                })
                .into_iter()
                .collect()
        } else {
            let query = query.to_uppercase();
            DATA.iter()
                .filter(|x| x.name.contains(&query))
                .take(50)
                .map(|x| Entry {
                    name: x.name.titlecase(),
                    tag: None,
                    description: Some(x.codepoint.to_owned()),
                    icon: EntryIcon::Character(
                        if x.scalar.is_whitespace() || x.scalar.is_control() {
                            '�'
                        } else {
                            x.scalar
                        },
                    ),
                    small_icon: EntryIcon::None,
                    actions: vec![EntryAction::Copy(x.scalar.to_string())],
                    id: "".to_owned(),
                })
                .collect()
        }
    }

    fn has_entry(&self) -> bool {
        true
    }
}
