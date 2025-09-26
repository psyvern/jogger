use std::{collections::HashMap, fmt::Debug, fs::Metadata, os::unix::fs::MetadataExt};

use xdg_mime::SharedMimeInfo;

use crate::interface::{Entry, EntryAction, EntryIcon, Plugin};

pub struct Files {
    mime_db: SharedMimeInfo,
}

impl Debug for Files {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Files {}")
    }
}

impl Files {
    pub fn new() -> Self {
        let mut mime_db = SharedMimeInfo::new();
        mime_db.reload();

        Self { mime_db }
    }

    fn search_inner(&self, query: &str) -> std::io::Result<Box<dyn Iterator<Item = Entry> + '_>> {
        let path = expanduser::expanduser(query)?;
        if std::fs::exists(&path)? {
            let file = std::fs::File::open(&path)?;
            let metadata = file.metadata()?;
            if metadata.is_dir() {
                let query = query.to_owned();
                let parent = query.trim_end_matches('/');
                let parent = parent.rfind('/').map(|x| &parent[..x]).unwrap_or(parent);
                return Ok(Box::new(
                    std::iter::once(Entry {
                        name: "..".to_string(),
                        tag: None,
                        description: Some("Go back".to_owned()),
                        icon: EntryIcon::Name("back".to_owned()),
                        small_icon: EntryIcon::None,
                        action: EntryAction::Write(parent.to_owned()),
                        sub_entries: HashMap::new(),
                        id: "".to_owned(),
                    })
                    .chain(std::fs::read_dir(&path)?.map(move |x| match x {
                        Ok(x) => {
                            let metadata = x.metadata().unwrap();
                            let name = x.file_name();
                            let name = name.to_string_lossy();
                            let (icon, desc) = get_file_info(&self.mime_db, &name, &metadata);

                            Entry {
                                name: name.to_string(),
                                tag: None,
                                description: Some(desc),
                                icon: EntryIcon::Name(icon),
                                small_icon: EntryIcon::None,
                                action: if metadata.is_dir() {
                                    EntryAction::Write(format!(
                                        "{}/{}/",
                                        query.rfind('/').map(|x| &query[..x]).unwrap_or(&query),
                                        name,
                                    ))
                                } else {
                                    EntryAction::Open(x.path())
                                },
                                sub_entries: HashMap::new(),
                                id: "".to_owned(),
                            }
                        }
                        Err(_) => Entry {
                            name: "Nothing".into(),
                            tag: None,
                            description: Some("Nothing".to_owned()),
                            icon: EntryIcon::Name("error".to_owned()),
                            small_icon: EntryIcon::None,
                            action: EntryAction::Nothing,
                            sub_entries: HashMap::new(),
                            id: "".to_owned(),
                        },
                    })),
                ));
            }
            Ok(Box::new(std::iter::empty()))
        } else {
            if let (Some(path), Some(file_query)) = (path.parent(), path.file_name()) {
                let file_query = file_query.to_string_lossy().to_string();
                let path = path.to_path_buf();
                let query = query.to_owned();
                return Ok(Box::new(std::fs::read_dir(&path)?.flat_map(move |x| {
                    let Ok(x) = x else {
                        return None;
                    };

                    let name = x.file_name();
                    let name = name.to_string_lossy();
                    if name.contains(&file_query) {
                        let metadata = x.metadata().unwrap();
                        let (icon, desc) = get_file_info(&self.mime_db, &name, &metadata);

                        return Some(Entry {
                            name: name.to_string(),
                            tag: None,
                            description: Some(desc),
                            icon: EntryIcon::Name(icon),
                            small_icon: EntryIcon::None,
                            action: if metadata.is_dir() {
                                EntryAction::Write(format!(
                                    "{}/{}/",
                                    query.rfind('/').map(|x| &query[..x]).unwrap_or(&query),
                                    name,
                                ))
                            } else {
                                EntryAction::Open(x.path())
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
}

fn get_file_info(database: &SharedMimeInfo, name: &str, metadata: &Metadata) -> (String, String) {
    if metadata.file_type().is_dir() {
        ("folder".to_owned(), "Folder".to_owned())
    } else {
        let guess = database
            .guess_mime_type()
            .zero_size(false)
            .file_name(name)
            .guess();
        let mime = guess.mime_type();

        let names = database.lookup_icon_names(mime);
        (
            names
                .iter()
                .find(|x| *x != "application-x-zerosize")
                .map_or(
                    if metadata.size() == 0 {
                        "application-x-zerosize"
                    } else {
                        "application-x-generic"
                    },
                    |x| x,
                )
                .to_owned(),
            mime.essence_str().to_owned(),
        )
    }
}

impl Plugin for Files {
    fn name(&self) -> &str {
        "Files"
    }

    fn icon(&self) -> Option<&str> {
        Some("system-file-manager")
    }

    fn search(&self, query: &str) -> Box<dyn Iterator<Item = Entry> + '_> {
        self.search_inner(query)
            .unwrap_or(Box::new(std::iter::empty()))
    }
}
