use std::{
    collections::{HashMap, HashSet, VecDeque},
    path::PathBuf,
};

use mime::Mime;
use xdg::BaseDirectories;
use xdg_mime::SharedMimeInfo;

use crate::plugins::applications::{DesktopEntry, read_desktop_entries};

pub struct XdgAppDatabase {
    pub app_map: HashMap<String, DesktopEntry>,
    pub mime_apps_lists: Vec<MimeAppsListFile>,
    pub mime_db: SharedMimeInfo,
}

impl XdgAppDatabase {
    pub fn new() -> XdgAppDatabase {
        XdgAppDatabase {
            app_map: read_desktop_entries()
                .into_iter()
                .map(|x| (x.id.clone(), x))
                .collect(),
            mime_apps_lists: default_mimeapps_paths()
                .flat_map(MimeAppsListFile::from_path)
                .collect(),
            mime_db: SharedMimeInfo::new(),
        }
    }
}

#[derive(Debug)]
struct MimeAppsListFile {
    apps: Vec<String>,
    default: HashMap<String, HashSet<String>>,
    added: HashMap<String, HashSet<String>>,
    removed: HashMap<String, HashSet<String>>,
}

impl MimeAppsListFile {
    fn from_path(path: PathBuf) -> Result<Self, ()> {
        enum Group {
            Default,
            Added,
            Removed,
        }

        let apps = std::fs::read_dir(&path)
            .map(|x| {
                x.flatten()
                    .flat_map(|file| {
                        file.file_name()
                            .to_string_lossy()
                            .strip_suffix(".desktop")
                            .map(str::to_owned)
                    })
                    .collect()
            })
            .unwrap_or_default();

        let mut result = Self {
            apps,
            default: HashMap::new(),
            added: HashMap::new(),
            removed: HashMap::new(),
        };
        let mut current_group = None;
        if let Ok(s) = std::fs::read_to_string(path.join("mimeapps.list")) {
            for line in s.lines() {
                match line {
                    "[Default Applications]" => current_group = Some(Group::Default),
                    "[Added Associations]" => current_group = Some(Group::Added),
                    "[Removed Associations]" => current_group = Some(Group::Removed),
                    _ => {
                        if line.trim().is_empty() {
                            continue;
                        }
                        let Some((mime, entries)) = line.split_once('=') else {
                            return Err(());
                        };

                        let entries = entries
                            .trim_end_matches(';')
                            .split(';')
                            .map(|x| x.trim_end_matches(".desktop").to_owned());

                        match current_group {
                            None => {
                                return Err(());
                            }
                            Some(Group::Default) => &mut result.default,
                            Some(Group::Added) => &mut result.added,
                            Some(Group::Removed) => &mut result.removed,
                        }
                        .insert(mime.to_owned(), entries.collect());
                    }
                }
            }
        }

        Ok(result)
    }
}

pub fn default_mimeapps_paths() -> impl Iterator<Item = PathBuf> {
    let base_dirs = BaseDirectories::new().unwrap();
    itertools::chain![
        std::iter::once(base_dirs.get_config_home()),
        base_dirs.get_config_dirs(),
        std::iter::once(base_dirs.get_data_home().join("applications")),
        base_dirs
            .get_data_dirs()
            .into_iter()
            .map(|x| x.join("applications")),
    ]
}

impl XdgAppDatabase {
    pub fn find_associations(&self, mime: &Mime) -> Vec<&DesktopEntry> {
        let mut seen: HashSet<&str> = HashSet::new();
        let mut openers = Vec::new();
        let mut mime_stack = VecDeque::new();

        mime_stack.push_back(mime.clone());

        let empty_set = HashSet::new();

        while let Some(mime) = mime_stack.pop_front() {
            for parent in self
                .mime_db
                .get_parents(&mime)
                .or_else(|| self.mime_db.get_parents_aliased(&mime))
                .unwrap_or_default()
            {
                mime_stack.push_back(parent);
            }

            let mut removed = HashSet::new();

            // perform a full file tour to find the default if there is one
            for list in self.mime_apps_lists.iter() {
                for id in list.default.get(mime.essence_str()).unwrap_or(&empty_set) {
                    if let Some(app) = self.app_map.get(id) {
                        seen.insert(id);
                        openers.push(app);
                        break;
                    }
                }
            }

            for list in self.mime_apps_lists.iter() {
                for id in list.added.get(mime.essence_str()).unwrap_or(&empty_set) {
                    if removed.contains(&id) || seen.contains(id.as_str()) {
                        continue;
                    }
                    seen.insert(id);
                    if let Some(app) = self.app_map.get(id) {
                        openers.push(app);
                    }
                }

                for id in list.removed.get(mime.essence_str()).unwrap_or(&empty_set) {
                    removed.insert(id);
                }

                for id in list.apps.iter() {
                    if let Some(app) = self.app_map.get(id) {
                        if removed.contains(id) || seen.contains(id.as_str()) {
                            continue;
                        }
                        if app.mime_types.contains(&mime.essence_str().to_owned()) {
                            seen.insert(id);
                            openers.push(app);
                        }
                    }
                }
            }
        }

        openers
    }
}
