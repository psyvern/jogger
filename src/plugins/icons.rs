use crate::interface::{Context, Entry, EntryIcon, FormatStyle, FormattedString, Plugin};

#[derive(Debug)]
pub struct Icons {}

impl Icons {
    pub fn new(_: &Context) -> Self {
        Self {}
    }
}

impl Plugin for Icons {
    fn name(&self) -> &str {
        "Icons"
    }

    fn icon(&self) -> Option<&str> {
        Some("iconthemes")
    }

    fn search(&self, query: &str, context: &Context) -> Vec<Entry> {
        let len = query.len();

        if len < 2 {
            return vec![];
        }

        let query = query.to_lowercase();

        context
            .icons
            .iter()
            .filter_map(|(x, y)| x.find(&query).map(|pos| (x, y, pos)))
            .map(|(x, path, pos)| Entry {
                name: FormattedString {
                    text: x.into(),
                    ranges: vec![(FormatStyle::Highlight, pos..pos + len)],
                },
                description: Some(path.into()),
                icon: EntryIcon::Name(x.into()),
                ..Default::default()
            })
            .collect()
    }
}
