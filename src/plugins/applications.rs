use std::fs::File;
use std::io::BufRead;
use std::path::PathBuf;
use std::{cmp::Ordering, collections::HashMap};

use freedesktop_desktop_entry::{default_paths, get_languages_from_env};
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;
use gtk::gdk::{Key, ModifierType};
use itertools::Itertools;
use xdg::BaseDirectories;

use crate::interface::{Context, FormatStyle, FormattedString};
use crate::utils::IteratorExt;
use crate::xdg_database::ExecParser;
use crate::{Entry, EntryAction, Plugin, interface::EntryIcon};

#[derive(Clone, Debug)]
pub struct DesktopEntry {
    pub id: String,
    name: String,
    description: Option<String>,
    pub icon: Option<String>,
    file_path: PathBuf,
    categories: Vec<String>,
    keywords: Vec<String>,
    pub actions: HashMap<String, DesktopEntryAction>,
    pub working_directory: Option<PathBuf>,
    exec: Option<String>,
    pub terminal: bool,
    pub terminal_args: TerminalArgs,
    pub(crate) mime_types: Vec<String>,
    pub display: bool,
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
pub struct DesktopEntryAction {
    name: String,
    pub icon: Option<String>,
    pub exec: Option<String>,
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
            file_path: value.path.clone(),
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
                                    exec: value.action_exec(x).map(str::to_owned),
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
            display: !value.no_display(),
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

    fn get_score(
        &self,
        query: &str,
        matcher: &SkimMatcherV2,
        desktop_file_opener: &str,
    ) -> Option<(u8, i64, Entry)> {
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
            .map(|(kind, score, indices)| {
                let actions = self.get_actions(desktop_file_opener);

                match kind {
                    Kind::Name => (
                        7,
                        score,
                        Entry {
                            name: format_indices(&self.name, indices),
                            tag: None,
                            description: self.description.as_ref().map(FormattedString::plain),
                            icon: EntryIcon::from(self.icon.clone()),
                            small_icon: EntryIcon::None,
                            actions,
                            id: "".to_owned(),
                            ..Default::default()
                        },
                    ),
                    Kind::Description => (
                        6,
                        score,
                        Entry {
                            name: FormattedString::plain(&self.name),
                            tag: None,
                            description: self
                                .description
                                .as_ref()
                                .map(|x| format_indices(x, indices)),
                            icon: EntryIcon::from(self.icon.clone()),
                            small_icon: EntryIcon::None,
                            actions,
                            id: "".to_owned(),
                            ..Default::default()
                        },
                    ),
                    Kind::Keyword(i) => (
                        5,
                        score,
                        Entry {
                            name: FormattedString::plain(&self.name),
                            tag: Some(format_indices_with_prefix(&self.keywords[i], '#', indices)),
                            description: self.description.as_ref().map(FormattedString::plain),
                            icon: EntryIcon::from(self.icon.clone()),
                            small_icon: EntryIcon::None,
                            actions,
                            id: "".to_owned(),
                            ..Default::default()
                        },
                    ),
                    Kind::Category(i) => (
                        4,
                        score,
                        Entry {
                            name: FormattedString::plain(&self.name),
                            tag: Some(format_indices_with_prefix(
                                &self.categories[i],
                                '@',
                                indices,
                            )),
                            description: self.description.as_ref().map(FormattedString::plain),
                            icon: EntryIcon::from(self.icon.clone()),
                            small_icon: EntryIcon::None,
                            actions,
                            id: "".to_owned(),
                            ..Default::default()
                        },
                    ),
                }
            })
    }

    fn get_actions(&self, desktop_file_opener: &str) -> Vec<(EntryAction, Key, ModifierType)> {
        let mut vec = vec![
            EntryAction::Open(self.id.clone(), None, None).into(),
            (
                EntryAction::Open(
                    desktop_file_opener.to_owned(),
                    None,
                    Some(self.file_path.clone()),
                ),
                Key::e,
                ModifierType::CONTROL_MASK,
            ),
        ];
        vec.extend(self.actions.keys().enumerate().flat_map(|(i, x)| {
            let key = match i {
                0 => Key::_1,
                1 => Key::_2,
                2 => Key::_3,
                3 => Key::_4,
                4 => Key::_5,
                5 => Key::_6,
                6 => Key::_7,
                7 => Key::_8,
                8 => Key::_9,
                9 => Key::_0,
                _ => return None,
            };
            Some((
                EntryAction::Open(self.id.clone(), Some(x.to_owned()), None),
                key,
                ModifierType::CONTROL_MASK,
            ))
        }));
        vec
    }

    fn get_action_score(
        &self,
        action_id: &str,
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
                        name: format_indices(&action.name, indices),
                        tag: None,
                        description: Some(FormattedString::plain(&self.name)),
                        icon: EntryIcon::from(self.icon.clone()),
                        small_icon: EntryIcon::Name(
                            action.icon.clone().unwrap_or("emblem-added".into()),
                        ),
                        actions: vec![
                            EntryAction::Open(self.id.clone(), Some(action_id.to_owned()), None)
                                .into(),
                        ],
                        id: "".to_owned(),
                        ..Default::default()
                    },
                ),
                Kind::Description => (
                    2,
                    score,
                    Entry {
                        name: FormattedString::plain(&action.name),
                        tag: None,
                        description: Some(format_indices(&self.name, indices)),
                        icon: EntryIcon::from(self.icon.clone()),
                        small_icon: EntryIcon::Name(
                            action.icon.clone().unwrap_or("emblem-added".into()),
                        ),
                        actions: vec![
                            EntryAction::Open(self.id.clone(), Some(action_id.to_owned()), None)
                                .into(),
                        ],
                        id: "".to_owned(),
                        ..Default::default()
                    },
                ),
                Kind::Keyword(i) => (
                    1,
                    score,
                    Entry {
                        name: FormattedString::plain(&action.name),
                        tag: Some(format_indices_with_prefix(&self.keywords[i], '#', indices)),
                        description: Some(FormattedString::plain(&self.name)),
                        icon: EntryIcon::from(self.icon.clone()),
                        small_icon: EntryIcon::Name(
                            action.icon.clone().unwrap_or("emblem-added".into()),
                        ),
                        actions: vec![
                            EntryAction::Open(self.id.clone(), Some(action_id.to_owned()), None)
                                .into(),
                        ],
                        id: "".to_owned(),
                        ..Default::default()
                    },
                ),
                Kind::Category(i) => (
                    0,
                    score,
                    Entry {
                        name: FormattedString::plain(&action.name),
                        tag: Some(format_indices_with_prefix(
                            &self.categories[i],
                            '@',
                            indices,
                        )),
                        description: Some(FormattedString::plain(&self.name)),
                        icon: EntryIcon::from(self.icon.clone()),
                        small_icon: EntryIcon::Name(
                            action.icon.clone().unwrap_or("emblem-added".into()),
                        ),
                        actions: vec![
                            EntryAction::Open(self.id.clone(), Some(action_id.to_owned()), None)
                                .into(),
                        ],
                        id: "".to_owned(),
                        ..Default::default()
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

    pub fn parse_str(&self, string: &str, uris: &[String], force_append: bool) -> Vec<String> {
        let parser = ExecParser {
            name: &self.name,
            icon: self.icon.as_deref(),
            force_append,
        };

        parser.parse(string, uris)
    }

    pub fn program(&self) -> String {
        let ss = self.parse_exec(&[], false);
        ss.into_iter().next().unwrap_or_default()
    }
}

fn format_indices(string: &str, indices: Vec<usize>) -> FormattedString {
    FormattedString {
        text: string.to_owned(),
        ranges: indices
            .into_iter()
            .ranges()
            .map(|x| (FormatStyle::Highlight, x))
            .collect(),
    }
}

fn format_indices_with_prefix(string: &str, prefix: char, indices: Vec<usize>) -> FormattedString {
    let offset = prefix.len_utf8();
    FormattedString {
        text: format!("{prefix}{string}"),
        ranges: indices
            .into_iter()
            .ranges()
            .map(|x| (FormatStyle::Highlight, (x.start + offset)..(x.end + offset)))
            .collect(),
    }
}

#[derive(Debug)]
pub struct Applications {
    desktop_file_opener: String,
}

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
        .unique_by(|entry| entry.path.clone())
        .unique_by(|entry| (entry.id().to_owned(), entry.exec().map(str::to_owned)))
        .map(|entry| DesktopEntry::new(entry, &locales, &frequency, &ignored))
        .collect()
}

impl Applications {
    pub fn new(context: &mut Context) -> Self {
        let opener = context
            .apps
            .default_for_mime(&"application/x-desktop".parse().unwrap())
            .or_else(|| {
                context
                    .apps
                    .default_for_mime(&"text/plain".parse().unwrap())
            })
            .expect("Couldn't find app to open .desktop files");

        Self {
            desktop_file_opener: opener.id.clone(),
        }
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
                .filter(|x| x.display)
                .sorted_by(|a, b| match b.frequency.cmp(&a.frequency) {
                    Ordering::Equal => a.name.cmp(&b.name),
                    x => x,
                })
                .map(|x| Entry {
                    name: FormattedString::plain(&x.name),
                    tag: None,
                    description: x.description.as_ref().map(FormattedString::plain),
                    icon: EntryIcon::from(x.icon.clone()),
                    small_icon: EntryIcon::None,
                    actions: x.get_actions(&self.desktop_file_opener),
                    id: "".to_owned(),
                    ..Default::default()
                })
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
                .filter(|x| x.display)
                .flat_map(|entry| {
                    entry
                        .actions
                        .iter()
                        .flat_map(|(id, action)| {
                            entry.get_action_score(id, action, query, &matcher)
                        })
                        .chain(entry.get_score(query, &matcher, &self.desktop_file_opener))
                })
                .sorted_by_cached_key(|(a, b, _)| (*b, *a))
                .rev()
                .take(20)
                .map(|(_, _, x)| x)
                .collect()
        }
    }
}
