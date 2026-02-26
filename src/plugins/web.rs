use std::path::PathBuf;

use crate::interface::{Context, Entry, EntryAction, FormatStyle, FormattedString, Plugin};

#[derive(Debug)]
pub struct Web {
    handler: String,
}

impl Web {
    pub fn new(context: &Context) -> Self {
        Self {
            handler: context
                .apps
                .default_for_mime(&"x-scheme-handler/https".parse().unwrap())
                .unwrap()
                .id
                .clone(),
        }
    }
}

impl Plugin for Web {
    fn name(&self) -> &str {
        "Web"
    }

    fn icon(&self) -> Option<&str> {
        Some("search")
    }

    fn search(&self, query: &str, _: &Context) -> Vec<Entry> {
        let len = query.len();

        let Ok(request) =
            reqwest::blocking::get(format!("https://www.startpage.com/osuggestions?q={query}"))
        else {
            return vec![];
        };

        let Ok(results) = request.json::<(String, Vec<String>)>() else {
            return vec![];
        };

        results
            .1
            .into_iter()
            .map(|x| {
                let path = PathBuf::from(format!("https://www.startpage.com/sp/search?query={x}"));
                Entry {
                    name: FormattedString {
                        text: x,
                        ranges: vec![(FormatStyle::Highlight, 0..len)],
                    },
                    description: Some("Web search".into()),
                    icon: Some("search".to_owned()).into(),
                    actions: vec![EntryAction {
                        icon: "search".into(),
                        name: "Search".into(),
                        function: EntryAction::open(self.handler.clone(), None, Some(path)),
                        ..Default::default()
                    }],
                    score: 1000,
                    ..Default::default()
                }
            })
            .collect()
    }
}
