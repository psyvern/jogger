use std::{
    collections::{HashMap, HashSet, VecDeque},
    fs::File,
    io::Read,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::Command,
};

use mime::Mime;
use xdg::BaseDirectories;
use xdg_mime::SharedMimeInfo;

use crate::{
    plugins::applications::{DesktopEntry, read_desktop_entries},
    utils::CommandExt,
};

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

impl Default for XdgAppDatabase {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
pub struct MimeAppsListFile {
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

pub struct Guess {
    pub mime: Mime,
    pub uncertain: bool,
}

impl XdgAppDatabase {
    pub fn default_for_mime(&self, mime: &Mime) -> Option<&DesktopEntry> {
        let empty = HashSet::new();
        for list in &self.mime_apps_lists {
            for id in list.default.get(mime.essence_str()).unwrap_or(&empty) {
                if let Some(app) = self.app_map.get(id) {
                    return Some(app);
                }
            }
        }

        let openers = self.find_associations(mime);

        openers.into_iter().next()
    }

    pub fn common_ancestor(&self, a: &Mime, b: &Mime) -> Option<Mime> {
        let mut stack = VecDeque::new();
        stack.push_back(a.clone());

        let mut ancestors = Vec::new();
        while let Some(x) = stack.pop_front() {
            if let Some(parents) = self
                .mime_db
                .get_parents(&x)
                .or_else(|| self.mime_db.get_parents_aliased(&x))
            {
                for parent in parents {
                    stack.push_back(parent);
                }
            }

            ancestors.push(x);
        }

        stack.push_back(b.clone());

        while let Some(x) = stack.pop_front() {
            if ancestors.contains(&x) {
                return Some(x);
            }
            if let Some(parents) = self
                .mime_db
                .get_parents(&x)
                .or_else(|| self.mime_db.get_parents_aliased(&x))
            {
                for parent in parents.clone() {
                    stack.push_back(parent);
                }
            }
        }

        None
    }

    pub fn common_ancestor_multiple<'a, I: Iterator<Item = &'a Mime>>(
        &self,
        mut mimes: I,
    ) -> Option<Mime> {
        let mut acc = mimes.next().cloned();

        for item in mimes {
            match acc {
                None => return None,
                Some(x) => acc = self.common_ancestor(&x, item),
            }
        }
        acc
    }

    pub fn guess<P: AsRef<Path>>(&self, path: P) -> Guess {
        // Fill out the metadata
        let metadata = match std::fs::metadata(&path) {
            Ok(m) => Some(m),
            Err(_) => None,
        };

        fn load_data_chunk<P: AsRef<Path>>(path: P, chunk_size: usize) -> Option<Vec<u8>> {
            if chunk_size == 0 {
                return None;
            }

            let mut f = match File::open(&path) {
                Ok(file) => file,
                Err(_) => return None,
            };

            let mut buf = vec![0u8; chunk_size];

            if f.read_exact(&mut buf).is_err() {
                return None;
            }

            Some(buf)
        }

        // Load the minimum amount of data necessary for a match
        // let mut max_data_size = xdg_mime::magic::max_extents(&self.mime_db.magic);
        let mut max_data_size = 1;

        if let Some(metadata) = &metadata {
            let file_size: usize = metadata.len() as usize;
            if file_size < max_data_size {
                max_data_size = file_size;
            }
        }

        let data = load_data_chunk(&path, max_data_size).unwrap_or_default();

        // Set the file name
        let file_name = if let Some(file_name) = path.as_ref().file_name() {
            file_name.to_os_string().into_string().ok()
        } else {
            None
        };

        if let Some(metadata) = &metadata {
            let file_type = metadata.file_type();

            // Special type for directories
            if file_type.is_dir() {
                return Guess {
                    mime: "inode/directory".parse().unwrap(),
                    uncertain: true,
                };
            }

            // Special type for symbolic links
            if file_type.is_symlink() {
                return Guess {
                    mime: "inode/symlink".parse().unwrap(),
                    uncertain: true,
                };
            }

            // Special type for empty files
            if metadata.len() == 0 {
                return Guess {
                    mime: "application/x-zerosize".parse().unwrap(),
                    uncertain: true,
                };
            }
        }

        let mut name_mime_types: Vec<Mime> = match &file_name {
            Some(file_name) => self.mime_db.get_mime_types_from_file_name(file_name),
            None => Vec::new(),
        };

        // File name match, and no conflicts
        if name_mime_types.len() == 1 {
            if name_mime_types[0] == mime::APPLICATION_OCTET_STREAM {
                name_mime_types = vec![];
            } else {
                return Guess {
                    mime: name_mime_types[0].clone(),
                    uncertain: false,
                };
            }
        }

        let sniffed_mime = self.mime_db.get_mime_type_for_data(&data);

        if name_mime_types.is_empty() {
            // No names and no data => unknown MIME type
            if data.is_empty() {
                return Guess {
                    mime: mime::APPLICATION_OCTET_STREAM,
                    uncertain: true,
                };
            }

            if let Some((mime, _)) = sniffed_mime {
                return Guess {
                    mime,
                    uncertain: false,
                };
            }
        } else {
            if let Some((mut mime, priority)) = sniffed_mime {
                // "If no magic rule matches the data (or if the content is not
                // available), use the default type of application/octet-stream
                // for binary data, or text/plain for textual data."
                // -- shared-mime-info, "Recommended checking order"
                // if mime == mime::APPLICATION_OCTET_STREAM && !data.is_empty() && looks_like_text(&data)
                // {
                //     mime = mime::TEXT_PLAIN;
                // }

                // From the content type guessing implementation in GIO:
                //
                // For security reasons we don't ever want to sniff desktop files
                // where we know the filename and it doesn't have a .desktop extension.
                // This is because desktop files allow executing any application and
                // we don't want to make it possible to hide them looking like something
                // else.
                if file_name.is_some() {
                    let x_desktop = "application/x-desktop".parse::<Mime>().unwrap();

                    if mime == x_desktop {
                        mime = mime::TEXT_PLAIN;
                    }
                }

                // We found a match with a high confidence value
                if priority >= 80 {
                    return Guess {
                        mime,
                        uncertain: false,
                    };
                }

                // We have possible conflicts, but the data matches the
                // file name, so let's see if the sniffed MIME type is
                // a subclass of the MIME type associated to the file name,
                // and use that as a tie breaker.
                if name_mime_types
                    .iter()
                    .any(|m| self.mime_db.mime_type_subclass(&mime, m))
                {
                    return Guess {
                        mime,
                        uncertain: false,
                    };
                }
            }

            // If there are conflicts, and the data does not help us,
            // we get the nearest common ancestor from the file name
            if let Some(mime_type) = self.common_ancestor_multiple(name_mime_types.iter()) {
                return Guess {
                    mime: mime_type.clone(),
                    uncertain: true,
                };
            }
        }

        if let Some(metadata) = &metadata {
            // Special type for executable files
            if metadata.permissions().mode() & 0o111 != 0 && path.as_ref().extension().is_none() {
                return Guess {
                    mime: "application/x-executable".parse().unwrap(),
                    uncertain: true,
                };
            }
        }

        // Okay, we give up
        Guess {
            mime: mime::APPLICATION_OCTET_STREAM,
            uncertain: true,
        }
    }

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

    fn terminal_emulator(&self) -> Option<&DesktopEntry> {
        if let Some(emulator) = self.default_for_mime(&"x-scheme-handler/terminal".parse().unwrap())
        {
            return Some(emulator);
        }

        println!(
            "No default terminal emulator could be found, will fallback on the first terminal emulator we find. To learn how to set one for vicinae to use: https://docs.vicinae.com/default-terminal"
        );

        self.app_map.values().find(|x| x.is_terminal_emulator())
    }

    pub fn launch(&self, app: &DesktopEntry, args: &[String]) -> bool {
        let exec = app.parse_exec(args, false);

        if exec.is_empty() {
            println!("No program to start the app");
            return false;
        }

        let mut command = if app.terminal {
            if let Some(emulator) = self.terminal_emulator() {
                let program = emulator.program();
                let mut command = Command::new(&program);

                // because yes, gnome-terminal does not support -e properly
                if program == "gnome-terminal" {
                    command.arg("--");
                } else {
                    command.arg("-e");
                }

                for part in exec {
                    command.arg(part);
                }

                command
            } else {
                return false;
            }
        } else {
            let mut command = Command::new(&exec[0]);
            for arg in &exec[1..] {
                command.arg(arg);
            }

            command
        };

        if let Some(working_directory) = &app.working_directory {
            command.current_dir(working_directory);
        }

        if let Err(error) = command.spawn_detached() {
            println!("Failed to start app {:?} {:?}", command.get_args(), error);
            return false;
        }

        true
    }
}

pub struct ExecParser<'a> {
    pub name: &'a str,
    pub icon: Option<&'a str>,
    pub force_append: bool,
}

impl ExecParser<'_> {
    pub fn parse(&self, data: &str, uris: &[String]) -> Vec<String> {
        let mut args = Vec::new();
        enum State {
            Reset,
            FieldCode,
            Escaped,
            Quote,
            QuotedEscaped,
        }
        let mut state = State::Reset;
        let mut part = String::new();
        let mut uri_expanded = false;
        let mut quote_char = 0 as char; // the current quotation char

        for ch in data.chars() {
            match state {
                State::Reset => match ch {
                    '"' | '\'' => {
                        state = State::Quote;
                        quote_char = ch;
                    }
                    '%' => {
                        state = State::FieldCode;
                    }
                    '\\' => {
                        state = State::Escaped;
                    }
                    ch if ch.is_whitespace() => {
                        if !part.is_empty() {
                            args.push(part.clone());
                            part.clear();
                        }
                    }
                    ch => {
                        part.push(ch);
                    }
                },
                State::FieldCode => {
                    match ch {
                        '%' => {
                            part.push('%');
                        }
                        'f' | 'u' => {
                            uri_expanded = true;
                            if let Some(uri) = uris.first() {
                                args.push(uri.clone());
                            }
                        }
                        'F' | 'U' => {
                            uri_expanded = true;
                            args.extend_from_slice(uris);
                        }
                        'i' => {
                            if let Some(m_icon) = self.icon {
                                args.push("--icon".to_owned());
                                args.push(m_icon.to_owned());
                            }
                        }
                        'c' => {
                            args.push(self.name.to_owned());
                        }
                        _ => {}
                    }

                    state = State::Reset;
                }

                State::Escaped => {
                    part.push(ch);
                    state = State::Reset;
                }
                State::Quote => {
                    if ch == '\\' {
                        state = State::QuotedEscaped;
                    } else if ch == quote_char {
                        state = State::Reset;
                    } else {
                        part.push(ch);
                    }
                }
                State::QuotedEscaped => {
                    part.push(ch);
                    state = State::Quote;
                }
            }
        }

        if !part.is_empty() {
            args.push(part);
        }
        if !uri_expanded && self.force_append {
            args.extend_from_slice(uris);
        }

        args
    }
}
