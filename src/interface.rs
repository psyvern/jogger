use std::collections::VecDeque;
use std::fmt::Debug;
use std::path::Path;
use std::path::PathBuf;

use gtk::Image;

use crate::xdg_database::XdgAppDatabase;

pub trait Plugin: Debug + Send + Sync {
    fn open(&mut self) {}

    fn name(&self) -> &str;

    fn icon(&self) -> Option<&str> {
        None
    }

    fn prefix(&self) -> Option<&str> {
        None
    }

    #[allow(unused)]
    fn search(&self, query: &str, context: &mut Context) -> Vec<Entry> {
        Vec::new()
    }

    fn select(&self, _entry: &Entry) {}

    fn has_entry(&self) -> bool {
        false
    }
}

#[derive(Clone, Debug)]
pub enum EntryAction {
    Close,
    Copy(String),
    HyprctlExec(String),
    Shell(String),
    Command(String, String, Vec<String>, Option<PathBuf>),
    LaunchTerminal {
        program: Option<String>,
        arguments: Vec<String>,
        working_directory: Option<PathBuf>,
    },
    Write(String),
    Open(String, Option<String>, Option<PathBuf>),
    ChangePlugin(Option<usize>),
}

impl From<EntryAction> for (EntryAction, gtk::gdk::Key, gtk::gdk::ModifierType) {
    fn from(value: EntryAction) -> Self {
        (
            value,
            gtk::gdk::Key::Return,
            gtk::gdk::ModifierType::NO_MODIFIER_MASK,
        )
    }
}

#[derive(Clone, Debug)]
pub struct Entry {
    pub name: String,
    pub tag: Option<String>,
    pub description: Option<String>,
    pub icon: EntryIcon,
    pub small_icon: EntryIcon,
    pub actions: Vec<(EntryAction, gtk::gdk::Key, gtk::gdk::ModifierType)>,
    pub id: String,
}

#[derive(Clone, Debug, Default)]
pub enum EntryIcon {
    Name(String),
    Path(PathBuf),
    Character(char),
    #[default]
    None,
}

impl EntryIcon {
    pub fn to_name(&self) -> Option<&str> {
        match self {
            EntryIcon::Name(value) => Some(value),
            _ => None,
        }
    }

    pub fn to_path(&self) -> Option<&Path> {
        match self {
            EntryIcon::Path(value) => Some(value),
            _ => None,
        }
    }

    pub fn to_gtk_image(&self) -> Image {
        match self {
            EntryIcon::Name(value) => Image::from_icon_name(value),
            EntryIcon::Path(value) => Image::from_file(value),
            _ => Image::new(),
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

pub struct Context {
    messages: VecDeque<String>,
    pub apps: XdgAppDatabase,
}

impl Default for Context {
    fn default() -> Self {
        Self {
            messages: VecDeque::new(),
            apps: XdgAppDatabase::new(),
        }
    }
}

impl Context {
    pub fn show_dialog(&mut self, message: &str) {
        self.messages.push_back(message.to_owned());
    }
}
