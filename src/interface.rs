use std::collections::VecDeque;
use std::fmt::Debug;
use std::ops::Range;
use std::path::Path;
use std::path::PathBuf;

use gtk::Image;
use gtk::pango::AttrColor;
use gtk::pango::AttrFontDesc;
use gtk::pango::AttrList;
use gtk::pango::Attribute;
use gtk::pango::Color;
use gtk::pango::FontDescription;

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

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FormatStyle {
    Highlight,
    Special,
    Monospace,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct FormattedString {
    pub text: String,
    pub ranges: Vec<(FormatStyle, Range<usize>)>,
}

impl FormattedString {
    pub fn plain(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            ranges: vec![],
        }
    }

    pub fn from_style(text: impl Into<String>, style: FormatStyle) -> Self {
        let text = text.into();
        let len = text.len();

        Self {
            text,
            ranges: vec![(style, 0..len)],
        }
    }

    pub fn from_styles(ranges: Vec<(&str, Option<FormatStyle>)>) -> Self {
        let mut result = String::new();
        let mut res_ranges = Vec::new();

        let mut tot_size = 0;
        for (text, style) in ranges {
            let size = text.len();

            if let Some(style) = style {
                res_ranges.push((style, tot_size..size));
            }

            result.push_str(text);
            tot_size += size;
        }

        Self {
            text: result,
            ranges: res_ranges,
        }
    }

    pub fn to_pango_escaped(&self) -> String {
        fn escape(text: &str) -> String {
            let mut result = String::new();
            for c in text.chars() {
                match c {
                    '&' => result.push_str("&amp;"),
                    '<' => result.push_str("&lt;"),
                    '>' => result.push_str("&gt;"),
                    '\'' => result.push_str("&apos;"),
                    '"' => result.push_str("&quot;"),
                    _ => result.push(c),
                }
            }

            result
        }

        let mut buffer = String::new();

        let mut last = 0;
        for (style, range) in &self.ranges {
            buffer.push_str(&escape(&self.text[last..range.start]));
            last = range.end;

            let text = escape(&self.text[range.clone()]);
            buffer.push_str(&match style {
                FormatStyle::Highlight => format!("<span color=\"#A2C9FE\">{text}</span>"),
                FormatStyle::Special => format!("<span color=\"#FFAF00\">{text}</span>"),
                FormatStyle::Monospace => format!("<tt>{text}</tt>"),
            });
        }
        buffer.push_str(&escape(&self.text[last..]));

        buffer
    }

    pub fn to_attr_list(&self, highlight_color: [u16; 3]) -> AttrList {
        let list = AttrList::new();
        let highlight: Attribute =
            AttrColor::new_foreground(highlight_color[0], highlight_color[1], highlight_color[2])
                .into();
        let special: Attribute = {
            let color = Color::parse("#FFAF00").unwrap();
            AttrColor::new_foreground(color.red(), color.green(), color.blue()).into()
        };
        let monospace: Attribute =
            AttrFontDesc::new(&FontDescription::from_string("monospace")).into();

        for (style, range) in &self.ranges {
            let mut attribute = match style {
                FormatStyle::Highlight => highlight.clone(),
                FormatStyle::Special => special.clone(),
                FormatStyle::Monospace => monospace.clone(),
            };

            attribute.set_start_index(range.start as u32);
            attribute.set_end_index(range.end as u32);
            list.insert(attribute);
        }

        list
    }
}

impl PartialOrd for FormattedString {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for FormattedString {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.text.cmp(&other.text)
    }
}

#[derive(Clone, Debug, Default)]
pub struct Entry {
    pub name: FormattedString,
    pub tag: Option<FormattedString>,
    pub description: Option<FormattedString>,
    pub icon: EntryIcon,
    pub small_icon: EntryIcon,
    pub actions: Vec<(EntryAction, gtk::gdk::Key, gtk::gdk::ModifierType)>,
    pub id: String,
    pub drag_file: Option<PathBuf>,
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
