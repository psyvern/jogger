use derivative::Derivative;
use std::collections::VecDeque;
use std::fmt::Debug;
use std::ops::Range;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use gtk::Image;
use gtk::gdk::Key;
use gtk::gdk::ModifierType;
use gtk::pango::AttrColor;
use gtk::pango::AttrFontDesc;
use gtk::pango::AttrList;
use gtk::pango::Attribute;
use gtk::pango::Color;
use gtk::pango::FontDescription;

use crate::utils::CommandExt;
use crate::utils::IteratorExt;
use crate::xdg_database::XdgAppDatabase;

pub trait Plugin: Debug + Send + Sync {
    fn open(&mut self) {}

    fn name(&self) -> &str;

    fn icon(&self) -> Option<&str> {
        None
    }

    #[allow(unused)]
    fn search(&self, query: &str, context: &Context) -> Vec<Entry> {
        Vec::new()
    }

    fn select(&self, _entry: &Entry) {}
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct EntryAction {
    pub icon: String,
    pub name: String,
    pub key: Key,
    pub modifier: ModifierType,
    #[derivative(Debug = "ignore")]
    pub function: Box<ActionType>,
}

impl EntryAction {
    pub fn write(value: impl Into<String>) -> Box<ActionType> {
        let value = value.into();
        Box::new(move |_| ActionResult::SetText(value.clone()))
    }

    pub fn copy(value: impl Into<String>) -> Box<ActionType> {
        let value = value.into();

        Box::new(move |_| {
            let mut opts = wl_clipboard_rs::copy::Options::new();
            opts.foreground(true);
            opts.copy(
                wl_clipboard_rs::copy::Source::Bytes(value.bytes().collect()),
                wl_clipboard_rs::copy::MimeType::Autodetect,
            )
            .is_ok()
            .into()
        })
    }

    pub fn copy_bytes(value: &[u8]) -> Box<ActionType> {
        let value: Box<[u8]> = value.into();

        Box::new(move |_| {
            let mut opts = wl_clipboard_rs::copy::Options::new();
            opts.foreground(true);
            opts.copy(
                wl_clipboard_rs::copy::Source::Bytes(value.clone()),
                wl_clipboard_rs::copy::MimeType::Autodetect,
            )
            .is_ok()
            .into()
        })
    }

    pub fn command(command: String, args: Vec<String>, path: Option<PathBuf>) -> Box<ActionType> {
        Box::new(move |_| {
            Command::new(&command)
                .args(&args)
                .current_dir(
                    path.as_ref()
                        .filter(|x| x.exists())
                        .unwrap_or(&std::env::current_dir().unwrap()),
                )
                .spawn_detached()
                .is_ok()
                .into()
        })
    }

    pub fn open(id: String, action: Option<String>, path: Option<PathBuf>) -> Box<ActionType> {
        Box::new(move |context| {
            if let Some(app) = context.apps.app_map.get(&id) {
                let args = match &path {
                    Some(path) => vec![path.to_string_lossy().to_string()],
                    None => vec![],
                };
                match &action {
                    Some(action) => context.apps.launch_action(app, action, &args),
                    None => context.apps.launch(app, &args),
                }
            } else {
                false
            }
            .into()
        })
    }

    pub fn launch_terminal(
        program: Option<String>,
        arguments: Vec<String>,
        working_directory: Option<PathBuf>,
    ) -> Box<ActionType> {
        Box::new(move |context| {
            if let Some(emulator) = context.apps.terminal_emulator() {
                let mut command = Command::new(emulator.program());

                if let Some(working_directory) = &working_directory
                    && let Some(arg) = &emulator.terminal_args.dir
                {
                    if arg.ends_with('=') {
                        command.arg(format!("{arg}{}", working_directory.to_string_lossy()));
                    } else {
                        command.arg(arg);
                        command.arg(working_directory);
                    }
                }

                if let Some(program) = &program {
                    command.arg(emulator.terminal_args.exec.as_deref().unwrap_or("-e"));

                    command.arg(program);
                    command.args(&arguments);
                }

                match command.spawn_detached() {
                    Err(error) => {
                        println!(
                            "Failed to start terminal {:?} {:?}",
                            command.get_args(),
                            error
                        );
                        false
                    }
                    _ => true,
                }
            } else {
                false
            }
            .into()
        })
    }
}

impl Default for EntryAction {
    fn default() -> Self {
        Self {
            icon: "image-missing".into(),
            name: "Run".into(),
            key: Key::Return,
            modifier: ModifierType::empty(),
            function: Box::new(|_| ActionResult::Error),
        }
    }
}

pub enum ActionResult {
    Ok,
    Error,
    SetText(String),
    SetPlugin(Option<usize>),
}

impl From<bool> for ActionResult {
    fn from(value: bool) -> Self {
        if value { Self::Ok } else { Self::Error }
    }
}

pub type ActionType = dyn Fn(&mut Context) -> ActionResult + Send;

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

    pub fn from_indices(string: &str, indices: impl IntoIterator<Item = usize>) -> Self {
        Self {
            text: string.to_owned(),
            ranges: indices
                .into_iter()
                .ranges()
                .map(|x| (FormatStyle::Highlight, x))
                .collect(),
        }
    }

    pub fn from_indices_with_prefix(
        string: &str,
        prefix: char,
        indices: impl IntoIterator<Item = usize>,
    ) -> Self {
        let offset = prefix.len_utf8();

        Self {
            text: format!("{prefix}{string}"),
            ranges: indices
                .into_iter()
                .ranges()
                .map(|x| (FormatStyle::Highlight, (x.start + offset)..(x.end + offset)))
                .collect(),
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

impl<S: Into<String>> From<S> for FormattedString {
    fn from(value: S) -> Self {
        Self {
            text: value.into(),
            ranges: vec![],
        }
    }
}

#[derive(Debug, Default)]
pub struct Entry {
    pub name: FormattedString,
    pub tag: Option<FormattedString>,
    pub description: Option<FormattedString>,
    pub icon: EntryIcon,
    pub small_icon: EntryIcon,
    pub actions: Vec<EntryAction>,
    pub id: String,
    pub drag_file: Option<PathBuf>,
    pub score: u64,
}

#[derive(Clone, Debug, Default)]
pub enum EntryIcon {
    Name(String),
    Path(PathBuf),
    Text(String),
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

#[derive(Default)]
pub struct Context {
    messages: VecDeque<String>,
    pub apps: XdgAppDatabase,
}

impl Context {
    pub fn show_dialog(&mut self, message: &str) {
        self.messages.push_back(message.to_owned());
    }
}
