use std::collections::HashMap;
use std::path::PathBuf;

use freedesktop_desktop_entry::{default_paths, get_languages_from_env};
use fuzzy_matcher::FuzzyMatcher;
use itertools::Itertools;
use xdg::BaseDirectories;

use crate::{interface::EntryIcon, Entry, EntryAction, Plugin, SubEntry};

const FIELD_CODE_LIST: [&str; 13] = [
    "%f", "%F", "%u", "%U", "%d", "%D", "%n", "%N", "%i", "%c", "%k", "%v", "%m",
];

#[derive(Clone, Debug)]
struct DesktopEntry {
    name: String,
    description: Option<String>,
    icon: Option<String>,
    path: Option<PathBuf>,
    categories: Vec<String>,
    keywords: Vec<String>,
    actions: HashMap<String, SubEntry>,
    exec: Option<String>,
    terminal: bool,
    frequency: u32,
}

impl DesktopEntry {
    fn new(
        value: freedesktop_desktop_entry::DesktopEntry,
        locales: &[String],
        frequency: &HashMap<String, u32>,
    ) -> Self {
        Self {
            name: value.name(locales).unwrap_or("<none>".into()).to_string(),
            description: value
                .comment(locales)
                .or_else(|| value.generic_name(locales))
                .map(String::from),
            icon: value.icon().map(str::to_owned),
            path: value.desktop_entry("Path").map(PathBuf::from),
            categories: value
                .desktop_entry("Categories")
                .map(|e| e.split(';').map(|e| e.to_owned()).collect())
                .unwrap_or_default(),
            keywords: value
                .keywords(locales)
                .map(|e| {
                    e.into_iter()
                        .filter(|e| !e.is_empty())
                        .map(|e| e.to_string())
                        .collect()
                })
                .unwrap_or_default(),
            actions: value
                .actions()
                .map(|x| {
                    x.iter()
                        .filter_map(|x| {
                            if x.is_empty() {
                                return None;
                            }
                            Some((
                                x.to_string(),
                                SubEntry {
                                    name: value
                                        .action_name(x, locales)
                                        .map(|x| x.to_string())
                                        .unwrap_or("<none>".into()),
                                    action: value
                                        .action_exec(x)
                                        .map(|exec| {
                                            let mut exec = exec.to_string();

                                            for field_code in FIELD_CODE_LIST {
                                                exec = exec.replace(field_code, "");
                                            }
                                            // TODO: add other fields
                                            EntryAction::Shell(exec, None)
                                        })
                                        .unwrap_or(EntryAction::Nothing),
                                },
                            ))
                        })
                        .collect()
                })
                .unwrap_or_default(),
            exec: value.exec().map(|exec| {
                let mut exec = exec.to_string();

                for field_code in FIELD_CODE_LIST {
                    exec = exec.replace(field_code, "");
                }
                exec
            }),
            terminal: value.terminal(),
            frequency: value
                .path
                .file_name()
                .and_then(|name| name.to_str())
                .and_then(|name| frequency.get(name))
                .copied()
                .unwrap_or(0),
        }
    }
}

impl From<&DesktopEntry> for Entry {
    fn from(value: &DesktopEntry) -> Self {
        Entry {
            name: value.name.clone(),
            description: value.description.clone(),
            icon: EntryIcon::from(value.icon.clone()),
            small_icon: EntryIcon::None,
            sub_entries: value.actions.clone(),
            action: match value.exec.clone() {
                Some(exec) => {
                    if value.terminal {
                        let term = std::env::var("TERMINAL_EMULATOR").unwrap_or("xterm".to_owned());
                        EntryAction::Shell(format!("{term} -e {exec}"), value.path.clone())
                    } else {
                        EntryAction::Shell(exec, value.path.clone())
                    }
                }
                None => EntryAction::Nothing,
            },
            id: "".to_owned(),
        }
    }
}

#[derive(Debug)]
pub struct Applications {
    entries: Vec<DesktopEntry>,
}

impl Applications {
    pub async fn new() -> Self {
        let mut plugin = Self {
            entries: Vec::new(),
        };

        plugin.reload();
        plugin
    }
}

impl Plugin for Applications {
    fn reload(&mut self) {
        let base_dirs = BaseDirectories::with_prefix("jogger").unwrap();
        let frequency_path = base_dirs.place_cache_file("frequency.toml").unwrap();
        let frequency = std::fs::read_to_string(frequency_path).unwrap();
        let frequency = toml::from_str::<HashMap<String, u32>>(&frequency).unwrap();

        let locales = get_languages_from_env();
        let entries = freedesktop_desktop_entry::Iter::new(default_paths())
            .entries(Some(&locales))
            .filter(|entry| !entry.no_display())
            .unique_by(|entry| entry.path.clone())
            .map(|entry| DesktopEntry::new(entry, &locales, &frequency))
            .collect_vec();

        self.entries = entries;
    }

    fn search(&self, query: &str) -> Box<dyn Iterator<Item = Entry> + '_> {
        if query.is_empty() {
            Box::new(
                self.entries
                    .iter()
                    .sorted_by_cached_key(|entry| entry.frequency)
                    .rev()
                    .map(Into::into),
            )
        } else {
            let matcher = fuzzy_matcher::skim::SkimMatcherV2::default().smart_case();
            // let mut matcher = Matcher::new(Config::DEFAULT.match_paths());
            // let pattern = Pattern::new(
            //     query,
            //     CaseMatching::Ignore,
            //     Normalization::Smart,
            //     AtomKind::Fuzzy,
            // );
            Box::new(
                self.entries
                    .iter()
                    .filter_map(|entry| {
                        let mut score = 0;

                        score += 4 * matcher.fuzzy_match(&entry.name, query).unwrap_or(0);

                        if let Some(ref description) = entry.description {
                            score += 2 * matcher.fuzzy_match(description, query).unwrap_or(0);
                        }

                        for category in entry.categories.iter() {
                            score += matcher.fuzzy_match(category, query).unwrap_or(0);
                        }

                        for keyword in entry.keywords.iter() {
                            score += matcher.fuzzy_match(keyword, query).unwrap_or(0);
                        }

                        if score == 0 {
                            None
                        } else {
                            Some((score, entry))
                        }
                    })
                    .sorted_by_cached_key(|(x, entry)| (*x, entry.frequency))
                    .rev()
                    .take(16)
                    .map(|(_, x)| x.into()),
            )
        }
    }
}
