use std::{collections::HashMap, fmt::Debug, os::unix::fs::MetadataExt};

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
                return Ok(Box::new(std::fs::read_dir(&path)?.map(|x| match x {
                    Ok(x) => {
                        let metadata = x.metadata().unwrap();
                        // let boi = gtk::gio::File::for_path(x.path());
                        // let info = boi
                        //     .query_info(
                        //         "standard::icon",
                        //         FileQueryInfoFlags::empty(),
                        //         None::<&Cancellable>,
                        //     )
                        //     .unwrap();
                        // let icon = info.icon().unwrap();
                        // let display = gtk::gdk::Display::default().unwrap();
                        // println!("{}", display.name());
                        // let icon_theme = &self.theme;
                        // let icon_filename = icon_theme.lookup_icon(
                        //     icon.to_string().unwrap().as_ref(),
                        //     &[],
                        //     48,
                        //     1,
                        //     gtk::TextDirection::None,
                        //     IconLookupFlags::empty(),
                        // );
                        // println!("{}", icon_filename.icon_name().unwrap().to_string_lossy());
                        let is_empty = metadata.size() == 0;
                        let (icon, desc) = if metadata.file_type().is_dir() {
                            ("folder".to_owned(), "Folder".to_owned())
                        } else {
                            let guess = self
                                .mime_db
                                .guess_mime_type()
                                .zero_size(false)
                                .file_name(&x.file_name().to_string_lossy())
                                .guess();
                            let mime = guess.mime_type();

                            let names = self.mime_db.lookup_icon_names(mime);
                            println!(
                                "{} - {} - {:?}",
                                x.file_name().to_string_lossy(),
                                mime,
                                names
                            );
                            (
                                names
                                    .iter()
                                    .find(|x| *x != "application-x-zerosize")
                                    .map_or(
                                        if is_empty {
                                            "application-x-zerosize"
                                        } else {
                                            "application-x-generic"
                                        },
                                        |x| x,
                                    )
                                    .to_owned(),
                                mime.essence_str().to_owned(),
                            )
                        };

                        Entry {
                            name: x.file_name().to_string_lossy().to_string(),
                            tag: None,
                            description: Some(desc),
                            icon: EntryIcon::Name(icon),
                            small_icon: EntryIcon::None,
                            action: EntryAction::Nothing,
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
                })));
            }
            Ok(Box::new(std::iter::empty()))
        } else {
            Ok(Box::new(std::iter::empty()))
        }
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
