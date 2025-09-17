use std::fs::File;
use std::io::BufRead;
use std::path::PathBuf;
use std::{cmp::Ordering, collections::HashMap};

use freedesktop_desktop_entry::{default_paths, get_languages_from_env};
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;
use itertools::Itertools;
use xdg::BaseDirectories;

use crate::{Entry, EntryAction, Plugin, SubEntry, interface::EntryIcon};

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
        ignored: &[(String, String)],
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
                            if ignored.contains(&(value.id().to_string(), x.to_string())) {
                                return None;
                            }
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
            frequency: frequency.get(value.id()).copied().unwrap_or(0),
        }
    }

    fn get_score(&self, query: &str, matcher: &SkimMatcherV2) -> Option<(u8, i64, Entry)> {
        if let Some((score, chars)) = matcher.fuzzy_indices(&self.name, query) {
            let mut buffer = String::new();

            for (i, c) in self.name.chars().enumerate() {
                if chars.contains(&i) {
                    buffer.push_str(&format!("<span color=\"#A2C9FE\">{c}</span>",));
                } else {
                    buffer.push(c);
                }
            }

            return Some((
                255,
                score,
                Entry::from(&DesktopEntry {
                    name: buffer,
                    ..self.clone()
                }),
            ));
        }

        if let Some(ref description) = self.description {
            if let Some(score) = matcher.fuzzy_match(description, query) {
                return Some((254, score, self.into()));
            }
        }

        for category in self.categories.iter() {
            let mut score = 0;
            score += matcher.fuzzy_match(category, query).unwrap_or(0);

            if score > 0 {
                return Some((253, score, self.into()));
            }
        }

        for keyword in self.keywords.iter() {
            let mut score = 0;
            score += matcher.fuzzy_match(keyword, query).unwrap_or(0);

            if score > 0 {
                return Some((252, score, self.into()));
            }
        }

        None
    }
}

impl From<&DesktopEntry> for Entry {
    fn from(value: &DesktopEntry) -> Self {
        Entry {
            name: value.name.clone(),
            description: value.description.clone(),
            icon: EntryIcon::from(value.icon.clone()),
            small_icon: EntryIcon::None,
            // sub_entries: value.actions.clone(),
            sub_entries: HashMap::new(),
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
        let base_dirs = BaseDirectories::with_prefix("jogger").unwrap();

        let ignored = base_dirs.place_config_file("ignored.conf").unwrap();
        let ignored = if std::fs::exists(&ignored).unwrap() {
            let ignored = File::open(ignored).unwrap();
            std::io::BufReader::new(ignored)
                .lines()
                .map_while(Result::ok)
                .flat_map(|x| {
                    let mut parts = x.splitn(2, '/');
                    match (parts.next(), parts.next()) {
                        (Some(a), Some(b)) => Some((a.to_string(), b.to_string())),
                        _ => None,
                    }
                })
                .collect()
        } else {
            std::fs::File::create(ignored).unwrap();
            vec![]
        };

        let frequency = base_dirs.place_config_file("frequency.toml").unwrap();
        let frequency = if std::fs::exists(&frequency).unwrap() {
            let frequency = std::fs::read_to_string(frequency).unwrap();
            toml::from_str::<HashMap<String, u32>>(&frequency).unwrap()
        } else {
            std::fs::File::create(frequency).unwrap();
            HashMap::new()
        };

        let locales = get_languages_from_env();
        let entries = freedesktop_desktop_entry::Iter::new(default_paths())
            .entries(Some(&locales))
            .filter(|entry| !entry.no_display())
            .unique_by(|entry| entry.path.clone())
            .unique_by(|entry| (entry.id().to_owned(), entry.exec().map(str::to_owned)))
            .map(|entry| DesktopEntry::new(entry, &locales, &frequency, &ignored))
            .collect_vec();

        Self { entries }
    }
}

impl Plugin for Applications {
    fn search(&self, query: &str) -> Box<dyn Iterator<Item = Entry> + '_> {
        if query.is_empty() {
            Box::new(
                self.entries
                    .iter()
                    .sorted_by(|a, b| match b.frequency.cmp(&a.frequency) {
                        Ordering::Equal => a.name.cmp(&b.name),
                        x => x,
                    })
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
                    .flat_map(|entry| {
                        entry
                            .actions
                            .iter()
                            .flat_map(|action| {
                                let mut score = 0;

                                score +=
                                    4 * matcher.fuzzy_match(&action.1.name, query).unwrap_or(0);
                                score += 4 * matcher.fuzzy_match(&entry.name, query).unwrap_or(0);

                                for category in entry.categories.iter() {
                                    score += matcher.fuzzy_match(category, query).unwrap_or(0);
                                }

                                for keyword in entry.keywords.iter() {
                                    score += matcher.fuzzy_match(keyword, query).unwrap_or(0);
                                }

                                if score > 0 {
                                    Some((
                                        0,
                                        score,
                                        Entry {
                                            name: action.1.name.clone(),
                                            description: Some(entry.name.clone()),
                                            icon: EntryIcon::from(entry.icon.clone()),
                                            small_icon: EntryIcon::Name("emblem-added".into()),
                                            sub_entries: HashMap::new(),
                                            action: action.1.action.clone(),
                                            id: "".to_owned(),
                                        },
                                    ))
                                } else {
                                    None
                                }
                            })
                            .chain(entry.get_score(query, &matcher))
                    })
                    .sorted_by_cached_key(|(a, b, _)| (*a, *b))
                    .rev()
                    .take(20)
                    .map(|(_, _, x)| x),
            )
        }
    }
}
