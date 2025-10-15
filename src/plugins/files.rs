use std::{fmt::Debug, fs::DirEntry, path::Path};

use gtk::gdk::{Key, ModifierType};
use itertools::Itertools;

use crate::{
    interface::{Context, Entry, EntryAction, EntryIcon},
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
    pub fn new() -> Self {
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
                                name: "..".to_string(),
                                tag: None,
                                description: Some("Go back".to_owned()),
                                icon: EntryIcon::Name("back".to_owned()),
                                small_icon: EntryIcon::None,
                                actions: vec![
                                    EntryAction::Write(if x == "/" { x } else { x + "/" }).into(),
                                    (
                                        EntryAction::Open(
                                            app_database.file_browser.clone().unwrap(),
                                            None,
                                            Some(parent.to_owned()),
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
                            }
                        })
                        .into_iter()
                        .chain(
                            std::fs::read_dir(&path)?
                                .flatten()
                                .flat_map(move |x| self.file_to_entry(app_database, x))
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
                    .filter(move |x| {
                        let name = x.file_name();
                        let name = name.to_string_lossy();
                        name.to_lowercase().contains(&file_query)
                    })
                    .flat_map(|x| self.file_to_entry(app_database, x)),
            ));
        }

        Ok(Box::new(std::iter::empty()))
    }

    fn file_to_entry(&self, database: &XdgAppDatabase, entry: DirEntry) -> Option<Entry> {
        let metadata = entry.metadata().ok()?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let mime = database.guess(entry.path()).mime;

        let app = database.default_for_mime(mime);
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

        Some(Entry {
            name: name.to_string(),
            tag: None,
            description: Some(desc),
            icon: EntryIcon::Name(icon),
            small_icon: EntryIcon::from(small_icon),
            actions: if metadata.is_dir() || mime.as_str() == "inode/directory" {
                vec![
                    EntryAction::Write(reduce_tilde(&entry.path(), &self.home_dir) + "/").into(),
                    (
                        EntryAction::Open(
                            database.file_browser.clone().unwrap(),
                            None,
                            Some(entry.path()),
                        ),
                        Key::Return,
                        ModifierType::SHIFT_MASK,
                    ),
                    (
                        EntryAction::LaunchTerminal {
                            program: None,
                            arguments: vec![],
                            working_directory: Some(entry.path()),
                        },
                        Key::t,
                        ModifierType::CONTROL_MASK,
                    ),
                ]
            } else if let Some(app) = app {
                vec![
                    EntryAction::Open(app.id.clone(), None, Some(entry.path())).into(),
                    (
                        EntryAction::Open(
                            database.file_browser.clone().unwrap(),
                            None,
                            Some(entry.path()),
                        ),
                        Key::Return,
                        ModifierType::SHIFT_MASK,
                    ),
                    (
                        EntryAction::LaunchTerminal {
                            program: None,
                            arguments: vec![],
                            working_directory: entry.path().parent().map(|x| x.to_owned()),
                        },
                        Key::t,
                        ModifierType::CONTROL_MASK,
                    ),
                ]
            } else {
                vec![]
            },
            id: "".to_owned(),
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
