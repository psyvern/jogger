use crate::Plugin;
use crate::interface::{Context, EntryIcon, FormatStyle, FormattedString};

use crate::{Entry, EntryAction};
use std::env;

#[derive(Debug)]
pub struct Commands {
    shell: Option<String>,
}

impl Commands {
    pub fn new(_: &Context) -> Self {
        Self {
            shell: env::var("SHELL").ok(),
        }
    }
}

impl Plugin for Commands {
    fn name(&self) -> &str {
        "Terminal"
    }

    fn icon(&self) -> Option<&str> {
        Some("terminal")
    }

    fn prefix(&self) -> Option<&str> {
        Some(">")
    }

    fn search(&self, query: &str, _: &Context) -> Vec<Entry> {
        vec![Entry {
            name: FormattedString::from_style(query.trim(), FormatStyle::Monospace),
            tag: None,
            description: self
                .shell
                .as_ref()
                .map(|x| FormattedString::from_style(x, FormatStyle::Monospace)),
            icon: EntryIcon::Name("terminal".to_owned()),
            small_icon: EntryIcon::None,
            actions: vec![EntryAction::Shell(query.trim().into()).into()],
            id: "".to_owned(),
            ..Default::default()
        }]
    }
}
