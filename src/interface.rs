use std::collections::HashMap;
use std::fmt::Debug;
use std::path::Path;
use std::path::PathBuf;

use gtk::Image;

pub trait Plugin: Debug + Send {
    fn icon(&self) -> Option<&str> {
        None
    }

    fn prefix(&self) -> Option<&str> {
        None
    }

    fn search(&mut self, query: &str) -> Box<dyn Iterator<Item = Entry> + '_> {
        Box::new(std::iter::empty())
    }
}

#[derive(Clone, Debug)]
pub enum EntryAction {
    Nothing,
    Close,
    Copy(String),
    Shell(String, Option<String>, Option<PathBuf>),
    Command(String, Option<PathBuf>),
}

#[derive(Clone, Debug)]
pub struct Entry {
    pub name: String,
    pub description: Option<String>,
    pub icon: EntryIcon,
    pub small_icon: EntryIcon,
    pub sub_entries: HashMap<String, SubEntry>,
    pub action: EntryAction,
}

#[derive(Clone, Debug, Default)]
pub enum EntryIcon {
    Name(String),
    Path(PathBuf),
    #[default]
    None,
}

impl EntryIcon {
    pub fn to_name(&self) -> Option<&str> {
        match self {
            EntryIcon::Name(value) => Some(value),
            EntryIcon::Path(_) => None,
            EntryIcon::None => None,
        }
    }

    pub fn to_path(&self) -> Option<&Path> {
        match self {
            EntryIcon::Name(_) => None,
            EntryIcon::Path(value) => Some(value),
            EntryIcon::None => None,
        }
    }

    pub fn to_gtk_image(&self) -> Image {
        match self {
            EntryIcon::Name(value) => Image::from_icon_name(value),
            EntryIcon::Path(value) => Image::from_file(value),
            EntryIcon::None => Image::new(),
        }
    }
}

impl From<Option<String>> for EntryIcon {
    fn from(value: Option<String>) -> Self {
        match value {
            Some(value) => Self::Name(value),
            None => Self::None,
        }
    }
}

impl From<Option<PathBuf>> for EntryIcon {
    fn from(value: Option<PathBuf>) -> Self {
        match value {
            Some(value) => Self::Path(value),
            None => Self::None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct SubEntry {
    pub name: String,
    pub action: EntryAction,
}
