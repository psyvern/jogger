use std::fs::File;
use std::io::BufRead;
use std::path::PathBuf;
use std::{cmp::Ordering, collections::HashMap};

use freedesktop_desktop_entry::{default_paths, get_languages_from_env};
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;
use itertools::Itertools;
use xdg::BaseDirectories;

use crate::interface::Context;
use crate::utils::IteratorExt;
use crate::xdg_database::ExecParser;
use crate::{Entry, EntryAction, Plugin, interface::EntryIcon};

const FIELD_CODE_LIST: [&str; 13] = [
    "%f", "%F", "%u", "%U", "%d", "%D", "%n", "%N", "%i", "%c", "%k", "%v", "%m",
];

#[derive(Clone, Debug)]
pub struct DesktopEntry {
    pub id: String,
    name: String,
    description: Option<String>,
    icon: Option<String>,
    path: Option<PathBuf>,
    categories: Vec<String>,
    keywords: Vec<String>,
    actions: HashMap<String, DesktopEntryAction>,
    pub working_directory: Option<PathBuf>,
    exec: Option<String>,
    pub terminal: bool,
    pub terminal_args: TerminalArgs,
    pub(crate) mime_types: Vec<String>,
    frequency: u32,
}

#[derive(Clone, Debug)]
pub struct TerminalArgs {
    pub exec: Option<String>,
    pub app_id: Option<String>,
    pub title: Option<String>,
    pub dir: Option<String>,
    pub hold: Option<String>,
}

#[derive(Clone, Debug)]
struct DesktopEntryAction {
    name: String,
    icon: Option<String>,
    action: EntryAction,
}

enum Kind {
    Name,
    Description,
    Category(usize),
    Keyword(usize),
}

impl DesktopEntry {
    fn new(
        value: freedesktop_desktop_entry::DesktopEntry,
        locales: &[String],
        frequency: &[String],
        ignored: &[(String, String)],
    ) -> Self {
        Self {
            id: value.id().to_owned(),
            name: value.name(locales).unwrap_or("<none>".into()).to_string(),
            description: value
                .comment(locales)
                .or_else(|| value.generic_name(locales))
                .map(String::from),
            icon: value.icon().map(str::to_owned),
            path: value.desktop_entry("Path").map(PathBuf::from),
            categories: value
                .desktop_entry("Categories")
                .map(|e| {
                    e.trim_end_matches(';')
                        .split(';')
                        .map(|e| e.to_owned())
                        .collect()
                })
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
                                DesktopEntryAction {
                                    name: value
                                        .action_name(x, locales)
                                        .map(|x| x.to_string())
                                        .unwrap_or("<none>".into()),
                                    icon: value.action_entry(x, "Icon").map(str::to_string),
                                    action: value
                                        .action_exec(x)
                                        .map(|exec| {
                                            let mut exec = exec.to_string();

                                            for field_code in FIELD_CODE_LIST {
                                                exec = exec.replace(field_code, "");
                                            }
                                            // TODO: add other fields
                                            EntryAction::Shell(exec)
                                        })
                                        .unwrap_or(EntryAction::Shell(String::new())),
                                },
                            ))
                        })
                        .collect()
                })
                .unwrap_or_default(),
            working_directory: value.path().map(PathBuf::from),
            exec: value.exec().map(str::to_owned),
            terminal: value.terminal(),
            terminal_args: TerminalArgs {
                exec: value.desktop_entry("X-TerminalArgExec").map(str::to_owned),
                app_id: value.desktop_entry("X-TerminalArgAppId").map(str::to_owned),
                title: value.desktop_entry("X-TerminalArgTitle").map(str::to_owned),
                dir: value.desktop_entry("X-TerminalArgDir").map(str::to_owned),
                hold: value.desktop_entry("X-TerminalArgHold").map(str::to_owned),
            },
            mime_types: value
                .mime_type()
                .unwrap_or_default()
                .iter()
                .filter(|x| !x.is_empty())
                .map(|x| x.to_string())
                .collect(),
            frequency: frequency
                .iter()
                .position(|x| x == value.id())
                .map(|x| (frequency.len() - x) as u32)
                .unwrap_or(0),
        }
    }

    pub fn is_terminal_emulator(&self) -> bool {
        self.categories.contains(&"TerminalEmulator".to_owned())
    }

    fn get_score(&self, query: &str, matcher: &SkimMatcherV2) -> Option<(u8, i64, Entry)> {
        let name = matcher
            .fuzzy_indices(&self.name, query)
            .map(|x| (Kind::Name, x.0 * 4 / 3, x.1));
        let description = self.description.as_ref().and_then(|x| {
            matcher
                .fuzzy_indices(x, query)
                .map(|x| (Kind::Description, x.0 * 5 / 4, x.1))
        });
        let categories = self.categories.iter().enumerate().flat_map(|(i, x)| {
            matcher
                .fuzzy_indices(x, query)
                .map(|x| (Kind::Category(i), x.0, x.1))
        });
        let keywords = self.keywords.iter().enumerate().flat_map(|(i, x)| {
            matcher
                .fuzzy_indices(x, query)
                .map(|x| (Kind::Keyword(i), x.0, x.1))
        });

        itertools::chain!(name, description, categories, keywords)
            .max_by_key(|x| x.1)
            .map(|(kind, score, indices)| match kind {
                Kind::Name => (
                    7,
                    score,
                    Entry::from(&DesktopEntry {
                        name: color_fuzzy_match(&self.name, indices),
                        ..self.clone()
                    }),
                ),
                Kind::Description => (
                    6,
                    score,
                    Entry::from(&DesktopEntry {
                        name: self.name.clone(),
                        description: self
                            .description
                            .as_ref()
                            .map(|x| color_fuzzy_match(x, indices)),
                        ..self.clone()
                    }),
                ),
                Kind::Keyword(i) => (
                    5,
                    score,
                    Entry::from((
                        self,
                        format!("#{}", color_fuzzy_match(&self.keywords[i], indices)),
                    )),
                ),
                Kind::Category(i) => (
                    4,
                    score,
                    Entry::from((
                        self,
                        format!("@{}", color_fuzzy_match(&self.categories[i], indices)),
                    )),
                ),
            })
    }

    fn get_action_score(
        &self,
        action: &DesktopEntryAction,
        query: &str,
        matcher: &SkimMatcherV2,
    ) -> Option<(u8, i64, Entry)> {
        let name = matcher
            .fuzzy_indices(&action.name, query)
            .map(|x| (Kind::Name, x.0 * 4 / 3, x.1));
        let entry_name = matcher
            .fuzzy_indices(&self.name, query)
            .map(|x| (Kind::Description, x.0 * 5 / 4, x.1));
        let categories = self.categories.iter().enumerate().flat_map(|(i, x)| {
            matcher
                .fuzzy_indices(x, query)
                .map(|x| (Kind::Category(i), x.0, x.1))
        });
        let keywords = self.keywords.iter().enumerate().flat_map(|(i, x)| {
            matcher
                .fuzzy_indices(x, query)
                .map(|x| (Kind::Keyword(i), x.0, x.1))
        });

        itertools::chain!(name, entry_name, categories, keywords)
            .max_by_key(|x| x.1)
            .map(|(kind, score, indices)| match kind {
                Kind::Name => (
                    3,
                    score,
                    Entry {
                        name: color_fuzzy_match(&action.name, indices),
                        tag: None,
                        description: Some(self.name.clone()),
                        icon: EntryIcon::from(self.icon.clone()),
                        small_icon: EntryIcon::Name(
                            action.icon.clone().unwrap_or("emblem-added".into()),
                        ),
                        actions: vec![action.action.clone().into()],
                        id: "".to_owned(),
                    },
                ),
                Kind::Description => (
                    2,
                    score,
                    Entry {
                        name: action.name.clone(),
                        tag: None,
                        description: Some(color_fuzzy_match(&self.name, indices)),
                        icon: EntryIcon::from(self.icon.clone()),
                        small_icon: EntryIcon::Name(
                            action.icon.clone().unwrap_or("emblem-added".into()),
                        ),
                        actions: vec![action.action.clone().into()],
                        id: "".to_owned(),
                    },
                ),
                Kind::Keyword(i) => (
                    1,
                    score,
                    Entry {
                        name: action.name.clone(),
                        tag: Some(color_fuzzy_match(&self.keywords[i], indices)),
                        description: Some(self.name.clone()),
                        icon: EntryIcon::from(self.icon.clone()),
                        small_icon: EntryIcon::Name(
                            action.icon.clone().unwrap_or("emblem-added".into()),
                        ),
                        actions: vec![action.action.clone().into()],
                        id: "".to_owned(),
                    },
                ),
                Kind::Category(i) => (
                    0,
                    score,
                    Entry {
                        name: action.name.clone(),
                        tag: Some(format!(
                            "@{}",
                            color_fuzzy_match(&self.categories[i], indices)
                        )),
                        description: Some(self.name.clone()),
                        icon: EntryIcon::from(self.icon.clone()),
                        small_icon: EntryIcon::Name(
                            action.icon.clone().unwrap_or("emblem-added".into()),
                        ),
                        actions: vec![action.action.clone().into()],
                        id: "".to_owned(),
                    },
                ),
            })
    }

    pub fn parse_exec(&self, uris: &[String], force_append: bool) -> Vec<String> {
        let Some(exec) = &self.exec else {
            return vec![];
        };

        let parser = ExecParser {
            name: &self.name,
            icon: self.icon.as_deref(),
            force_append,
        };

        parser.parse(exec, uris)
    }

    pub fn program(&self) -> String {
        let ss = self.parse_exec(&[], false);
        ss.into_iter().next().unwrap_or_default()
    }
}

fn color_fuzzy_match(string: &str, indices: Vec<usize>) -> String {
    let mut buffer = String::new();

    let mut last = 0;
    for range in indices.into_iter().ranges() {
        buffer.push_str(&string[last..range.start]);
        last = range.end;
        buffer.push_str(&format!(
            "<span color=\"#A2C9FE\">{}</span>",
            &string[range]
        ));
    }
    buffer.push_str(&string[last..]);

    buffer
}

impl From<&DesktopEntry> for Entry {
    fn from(value: &DesktopEntry) -> Self {
        Entry {
            name: value.name.clone(),
            tag: None,
            description: value.description.clone(),
            icon: EntryIcon::from(value.icon.clone()),
            small_icon: EntryIcon::None,
            actions: vec![EntryAction::Open(value.id.clone(), None).into()],
            id: "".to_owned(),
        }
    }
}

impl From<(&DesktopEntry, String)> for Entry {
    fn from(value: (&DesktopEntry, String)) -> Self {
        let (value, tag) = value;
        Entry {
            name: value.name.clone(),
            tag: Some(tag),
            description: value.description.clone(),
            icon: EntryIcon::from(value.icon.clone()),
            small_icon: EntryIcon::None,
            actions: vec![EntryAction::Open(value.id.clone(), None).into()],
            id: "".to_owned(),
        }
    }
}

#[derive(Debug)]
pub struct Applications {}

pub fn read_desktop_entries() -> Vec<DesktopEntry> {
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
    let frequency = if std::fs::exists(&frequency).unwrap_or(false) {
        std::fs::read_to_string(frequency)
            .unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .collect_vec()
    } else {
        std::fs::File::create(frequency).unwrap();
        vec![]
    };

    let locales = get_languages_from_env();
    freedesktop_desktop_entry::Iter::new(default_paths())
        .entries(Some(&locales))
        .filter(|entry| !entry.no_display())
        .unique_by(|entry| entry.path.clone())
        .unique_by(|entry| (entry.id().to_owned(), entry.exec().map(str::to_owned)))
        .map(|entry| DesktopEntry::new(entry, &locales, &frequency, &ignored))
        .collect()
}

impl Applications {
    pub fn new() -> Self {
        Self {}
    }
}

impl Plugin for Applications {
    fn name(&self) -> &str {
        "Applications"
    }

    fn search(&self, query: &str, context: &mut Context) -> Vec<Entry> {
        if query.is_empty() {
            context
                .apps
                .app_map
                .values()
                .sorted_by(|a, b| match b.frequency.cmp(&a.frequency) {
                    Ordering::Equal => a.name.cmp(&b.name),
                    x => x,
                })
                .map(Into::into)
                .collect()
        } else {
            let matcher = fuzzy_matcher::skim::SkimMatcherV2::default().ignore_case();
            // let mut matcher = Matcher::new(Config::DEFAULT.match_paths());
            // let pattern = Pattern::new(
            //     query,
            //     CaseMatching::Ignore,
            //     Normalization::Smart,
            //     AtomKind::Fuzzy,
            // );
            context
                .apps
                .app_map
                .values()
                .flat_map(|entry| {
                    entry
                        .actions
                        .iter()
                        .flat_map(|(_, action)| entry.get_action_score(action, query, &matcher))
                        .chain(entry.get_score(query, &matcher))
                })
                .sorted_by_cached_key(|(a, b, _)| (*b, *a))
                .rev()
                .take(20)
                .map(|(_, _, x)| x)
                .collect()
        }
    }
}
