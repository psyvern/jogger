use crate::Plugin;
use crate::interface::{Context, EntryAction, EntryIcon, FormatStyle, FormattedString};

use crate::Entry;
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

    fn search(&self, query: &str, _: &Context) -> Vec<Entry> {
        vec![Entry {
            name: FormattedString::from_style(query.trim(), FormatStyle::Monospace),
            description: self
                .shell
                .as_ref()
                .map(|x| FormattedString::from_style(x, FormatStyle::Monospace)),
            icon: EntryIcon::Name("terminal".into()),
            actions: vec![EntryAction {
                icon: "terminal".into(),
                name: "Run".into(),
                function: EntryAction::command(
                    "sh".into(),
                    vec!["-c".into(), query.trim().into()],
                    None,
                ),
                ..Default::default()
            }],
            ..Default::default()
        }]
    }
}
