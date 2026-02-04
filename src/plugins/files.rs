use std::{fmt::Debug, fs::DirEntry, ops::Range, os::unix::fs::MetadataExt, path::Path};

use gtk::gdk::{Key, ModifierType};
use itertools::Itertools;

use crate::{
    interface::{Context, Entry, EntryAction, EntryIcon, FormatStyle, FormattedString},
    xdg_database::XdgAppDatabase,
};

#[derive(Debug)]
pub struct Files {
    home_dir: String,
}

fn reduce_tilde(path: &Path, home_dir: &str) -> String {
    let path = path.to_string_lossy();
    match path.strip_prefix(home_dir) {
        Some(x) => format!("~{x}"),
        None => path.into_owned(),
    }
}

impl Files {
    pub fn new(_: &mut Context) -> Self {
        Self {
            home_dir: std::env::var("HOME").unwrap(),
        }
    }

    fn search_inner<'a>(
        &'a self,
        query: &str,
        app_database: &'a XdgAppDatabase,
    ) -> std::io::Result<Box<dyn Iterator<Item = Entry> + 'a>> {
        let query = if query.starts_with('~') && !query.starts_with("~/") {
            &("~/".to_owned() + &query[1..])
        } else {
            query
        };
        let path = expanduser::expanduser(query)?;
        if query.ends_with('/') {
            let metadata = std::fs::metadata(&path)?;
            let path = path.canonicalize()?;
            if metadata.is_dir() {
                return Ok(Box::new(
                    path.parent()
                        .map(|parent| {
                            let x = reduce_tilde(parent, &self.home_dir);
                            Entry {
                                name: FormattedString::plain(".."),
                                tag: None,
                                description: Some(FormattedString::plain("Go back")),
                                icon: EntryIcon::Name("back".to_owned()),
                                small_icon: EntryIcon::None,
                                actions: vec![
                                    EntryAction::Write(if x == "/" { x } else { x + "/" }).into(),
                                    (
                                        EntryAction::Open(
                                            app_database.file_browser.clone().unwrap(),
                                            None,
                                            Some(parent.to_owned()),
                                            Some("Open in folder".into()),
                                        ),
                                        Key::Return,
                                        ModifierType::CONTROL_MASK,
                                    ),
                                    (
                                        EntryAction::LaunchTerminal {
                                            program: None,
                                            arguments: vec![],
                                            working_directory: Some(path.clone()),
                                        },
                                        Key::t,
                                        ModifierType::CONTROL_MASK,
                                    ),
                                ],
                                id: "".to_owned(),
                                ..Default::default()
                            }
                        })
                        .into_iter()
                        .chain(
                            std::fs::read_dir(&path)?
                                .flatten()
                                .flat_map(|x| self.file_to_entry(app_database, x, None))
                                .take(255)
                                .sorted_by_cached_key(|x| x.name.clone()),
                        ),
                ));
            }
            return Ok(Box::new(std::iter::empty()));
        }

        let (path, file_query) = if query.ends_with("/.") {
            (Some(path.as_path()), Some(".".to_owned()))
        } else {
            (
                path.parent(),
                path.file_name().map(|x| x.to_string_lossy().to_lowercase()),
            )
        };
        if let (Some(path), Some(file_query)) = (path, file_query) {
            return Ok(Box::new(
                std::fs::read_dir(path)?
                    .flatten()
                    .filter_map(move |x| {
                        let name = x.file_name();
                        let name = name.to_string_lossy();
                        name.to_lowercase()
                            .find(&file_query)
                            .map(|pos| (x, pos..pos + file_query.len()))
                    })
                    .flat_map(|(x, range)| self.file_to_entry(app_database, x, Some(range)))
                    .take(255),
            ));
        }

        Ok(Box::new(std::iter::empty()))
    }

    fn file_to_entry(
        &self,
        database: &XdgAppDatabase,
        entry: DirEntry,
        range: Option<Range<usize>>,
    ) -> Option<Entry> {
        let path = entry.path();

        let metadata = entry.metadata().ok()?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let mime = database.guess(&path).mime;

        let is_dir = metadata.is_dir() || mime.as_str() == "inode/directory";
        let mut apps = database.find_associations(mime).into_iter();
        let icon = database.mime_db.lookup_icon_name(mime);
        let desc = database
            .mime_db
            .get_comment(mime)
            .cloned()
            .unwrap_or_default();
        let small_icon = if metadata.file_type().is_symlink() {
            Some("emblem-link".to_owned())
        } else {
            None
        };

        let size = if is_dir {
            let count = std::fs::read_dir(&path).map(|x| x.count()).unwrap_or(0);
            if count == 0 {
                "Empty".to_owned()
            } else {
                format!("{count} items")
            }
        } else {
            let mut size = metadata.size();
            let mut decimal = 0;
            let mut power = 0;

            while size > 1024 {
                decimal = size % 1024;
                size /= 1024;
                power += 1;
            }

            format!(
                "{size}{} {}B",
                if decimal == 0 {
                    "".to_owned()
                } else {
                    format!(".{:02}", (decimal as f64 / 10.24).round())
                },
                match power {
                    0 => "",
                    1 => "k",
                    2 => "M",
                    3 => "G",
                    4 => "T",
                    5 => "P",
                    6 => "E",
                    7 => "Z",
                    8 => "Y",
                    9 => "R",
                    10 => "Q",
                    _ => "?",
                }
            )
        };

        Some(Entry {
            name: match range {
                None => FormattedString::plain(name),
                Some(range) => FormattedString {
                    text: name.to_string(),
                    ranges: vec![(FormatStyle::Highlight, range)],
                },
            },
            tag: Some(FormattedString::plain(size)),
            description: Some(FormattedString::plain(desc)),
            icon: EntryIcon::Name(icon.clone()),
            small_icon: EntryIcon::from(small_icon),
            actions: if is_dir {
                vec![
                    EntryAction::Write(reduce_tilde(&path, &self.home_dir) + "/").into(),
                    (
                        EntryAction::Open(
                            database.file_browser.clone().unwrap(),
                            None,
                            Some(path.clone()),
                            None,
                        ),
                        Key::Return,
                        ModifierType::SHIFT_MASK,
                    ),
                    (
                        EntryAction::LaunchTerminal {
                            program: None,
                            arguments: vec![],
                            working_directory: Some(path.clone()),
                        },
                        Key::t,
                        ModifierType::CONTROL_MASK,
                    ),
                ]
            } else {
                let mut vec = Vec::new();
                if let Some(app) = apps.next() {
                    vec.push(
                        EntryAction::Open(app.id.clone(), None, Some(path.clone()), None).into(),
                    );
                }

                if let Some(file_browser) = &database.file_browser {
                    vec.push((
                        EntryAction::Open(
                            file_browser.to_owned(),
                            None,
                            Some(path.clone()),
                            Some("Open in folder".into()),
                        ),
                        Key::Return,
                        ModifierType::SHIFT_MASK,
                    ));
                }

                vec.push((
                    EntryAction::LaunchTerminal {
                        program: None,
                        arguments: vec![],
                        working_directory: path.parent().map(|x| x.to_owned()),
                    },
                    Key::t,
                    ModifierType::CONTROL_MASK,
                ));

                vec.extend(apps.map(|x| {
                    (
                        EntryAction::Open(
                            x.id.clone(),
                            None,
                            Some(path.clone()),
                            Some(format!("Open with {}", x.name)),
                        ),
                        Key::Escape,
                        ModifierType::NO_MODIFIER_MASK,
                    )
                }));

                vec.push((
                    EntryAction::Open(
                        "".to_owned(),
                        None,
                        Some(path.clone()),
                        Some("Open with...".to_owned()),
                    ),
                    Key::Escape,
                    ModifierType::NO_MODIFIER_MASK,
                ));

                vec
            },
            id: "".to_owned(),
            drag_file: Some(path),
        })
    }
}

impl Files {
    fn name(&self) -> &str {
        "Files"
    }

    fn icon(&self) -> Option<&str> {
        Some("system-file-manager")
    }

    pub fn search<'a>(
        &'a self,
        query: &str,
        context: &'a mut Context,
    ) -> Box<dyn Iterator<Item = Entry> + 'a> {
        self.search_inner(query, &context.apps)
            .unwrap_or(Box::new(std::iter::empty()))
    }
}
