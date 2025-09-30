use std::{collections::HashMap, fmt::Debug, fs::Metadata, path::Path};

use itertools::Itertools;

use crate::{
    interface::{Entry, EntryAction, EntryIcon},
    plugins::applications::DesktopEntry,
    xdg_database::XdgAppDatabase,
};

pub struct Files {
    home_dir: String,
}

impl Debug for Files {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Files {}")
    }
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
                        .map(|x| {
                            let x = reduce_tilde(x, &self.home_dir);
                            Entry {
                                name: "..".to_string(),
                                tag: None,
                                description: Some("Go back".to_owned()),
                                icon: EntryIcon::Name("back".to_owned()),
                                small_icon: EntryIcon::None,
                                action: EntryAction::Write(if x == "/" { x } else { x + "/" }),
                                sub_entries: HashMap::new(),
                                id: "".to_owned(),
                            }
                        })
                        .into_iter()
                        .chain(
                            std::fs::read_dir(&path)?
                                .flatten()
                                .flat_map(move |x| {
                                    let metadata = x.metadata().ok()?;
                                    let name = x.file_name();
                                    let name = name.to_string_lossy();
                                    let (icon, desc, small_icon, app) =
                                        get_file_info(app_database, &x.path(), &metadata);

                                    Some(Entry {
                                        name: name.to_string(),
                                        tag: None,
                                        description: Some(desc),
                                        icon: EntryIcon::Name(icon),
                                        small_icon: EntryIcon::from(small_icon),
                                        action: if metadata.is_dir() {
                                            EntryAction::Write(
                                                reduce_tilde(&x.path(), &self.home_dir) + "/",
                                            )
                                        } else if let Some(app) = app {
                                            EntryAction::Open(app.id.clone(), Some(x.path()))
                                        } else {
                                            EntryAction::Nothing
                                        },
                                        sub_entries: HashMap::new(),
                                        id: "".to_owned(),
                                    })
                                })
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
            return Ok(Box::new(std::fs::read_dir(path)?.flat_map(move |x| {
                let Ok(x) = x else {
                    return None;
                };

                let name = x.file_name();
                let name = name.to_string_lossy();
                if name.to_lowercase().contains(&file_query) {
                    let metadata = x.metadata().unwrap();
                    let (icon, desc, small_icon, app) =
                        get_file_info(app_database, &x.path(), &metadata);

                    return Some(Entry {
                        name: name.to_string(),
                        tag: None,
                        description: Some(desc),
                        icon: EntryIcon::Name(icon),
                        small_icon: EntryIcon::from(small_icon),
                        action: if metadata.is_dir() {
                            EntryAction::Write(reduce_tilde(&x.path(), &self.home_dir) + "/")
                        } else if let Some(app) = app {
                            EntryAction::Open(app.id.clone(), Some(x.path()))
                        } else {
                            EntryAction::Nothing
                        },
                        sub_entries: HashMap::new(),
                        id: "".to_owned(),
                    });
                }

                None
            })));
        }

        Ok(Box::new(std::iter::empty()))
    }
}

fn get_file_info<'a>(
    database: &'a XdgAppDatabase,
    path: &Path,
    metadata: &Metadata,
) -> (String, String, Option<String>, Option<&'a DesktopEntry>) {
    let guess = database.guess(path);
    let mime = guess.mime;
    let app = database.default_for_mime(&mime);

    let names = database.mime_db.lookup_icon_names(&mime);
    (
        names
            .into_iter()
            .next()
            .unwrap_or("application-x-generic".to_owned()),
        database
            .mime_db
            .get_comment(&mime)
            .cloned()
            .unwrap_or(String::new()),
        if metadata.file_type().is_symlink() {
            Some("emblem-link".to_owned())
        } else {
            None
        },
        app,
    )
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
        app_database: &'a XdgAppDatabase,
    ) -> Box<dyn Iterator<Item = Entry> + 'a> {
        self.search_inner(query, app_database)
            .unwrap_or(Box::new(std::iter::empty()))
    }
}
