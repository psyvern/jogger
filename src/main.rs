mod color;
pub mod interface;
mod plugins;
mod search_entry;
pub mod utils;
pub mod xdg_database;

use dbus::channel::MatchingReceiver;
use dbus::message::MatchRule;
use dbus_crossroads::Crossroads;
use gtk::cairo::Region;
use gtk::gdk::prelude::SurfaceExt;
use gtk::gdk::{ContentProvider, Display, FileList, Key, ModifierType};
use gtk::glib::Propagation;
use gtk::glib::translate::ToGlibPtr;
use gtk::glib::value::ToValue;
use gtk::prelude::{EventControllerExt, GestureSingleExt, NativeExt};
use gtk::{
    CenterBox, CssProvider, DragSource, EventControllerKey, GestureClick, IconTheme, Orientation,
    PropagationPhase, Separator,
};
use hyprland::dispatch::{Dispatch, DispatchType};
use itertools::Itertools;
use parking_lot::RwLock;
use relm4::prelude::{AsyncComponent, AsyncComponentParts};
use relm4::{AsyncComponentSender, view};
use serde::Deserialize;
use std::path::Path;
use std::process::Command;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;
use xdg::BaseDirectories;

use gtk::{
    Align, Box as GBox, Button, Grid, Image, Justification, Label, ListBox, ListBoxRow,
    Orientation::Vertical,
    Overlay, ScrolledWindow, Window,
    gdk::{self},
    pango::EllipsizeMode,
    prelude::{
        AdjustmentExt, BoxExt, ButtonExt, EditableExt, GridExt, GtkWindowExt, ListBoxRowExt,
        OrientableExt, WidgetExt,
    },
};
use gtk_layer_shell::{KeyboardMode, Layer, LayerShell};
use interface::{Entry, EntryAction, Plugin};
use relm4::{
    Component, ComponentController, Controller, FactorySender, RelmApp, RelmWidgetExt,
    factory::{Position, positions::GridPosition},
    prelude::{DynamicIndex, FactoryComponent, FactoryVecDeque},
};
use search_entry::SearchEntryModel;

use crate::color::PangoColor;
use crate::interface::{Context, EntryIcon, FormattedString};
use crate::plugins::files::Files;
use crate::utils::CommandExt;

trait FactoryVecDequeExt<T> {
    type Input;

    fn try_send(&self, index: usize, msg: Self::Input);
}

impl<C> FactoryVecDequeExt<C> for FactoryVecDeque<C>
where
    C: FactoryComponent<Index = DynamicIndex>,
{
    type Input = C::Input;

    fn try_send(&self, index: usize, msg: Self::Input) {
        if index < self.len() {
            self.send(index, msg);
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum EntryMsg {
    Select,
    Unselect,
}

#[derive(Debug)]
enum EntryOutput {
    Activate(DynamicIndex),
    ButtonDown(DynamicIndex, bool),
    DragStart,
    DragEnd,
}

impl From<EntryOutput> for AppMsg {
    fn from(value: EntryOutput) -> Self {
        match value {
            EntryOutput::Activate(index) => AppMsg::Activate(index.current_index()),
            EntryOutput::DragStart => AppMsg::SetDragging(true),
            EntryOutput::DragEnd => AppMsg::SetDragging(false),
            EntryOutput::ButtonDown(index, secondary) => {
                AppMsg::SelectEntry(index.current_index(), secondary)
            }
        }
    }
}

struct GridEntryComponent {
    plugin: usize,
    entry: Rc<Entry>,
    selected: bool,
    grid_size: usize,
}

impl Position<GridPosition, DynamicIndex> for GridEntryComponent {
    fn position(&self, index: &DynamicIndex) -> GridPosition {
        let index = index.current_index();
        let x = index % self.grid_size;
        let y = index / self.grid_size;

        GridPosition {
            column: x as i32,
            row: y as i32,
            width: 1,
            height: 1,
        }
    }
}

#[relm4::factory]
impl FactoryComponent for GridEntryComponent {
    type Init = (usize, Rc<Entry>, usize);
    type Input = EntryMsg;
    type Output = EntryOutput;
    type CommandOutput = ();
    type ParentWidget = Grid;

    view! {
        #[root]
        ListBoxRow {
            set_expand: true,
            #[watch]
            set_class_active: ("selected", self.selected),
            set_cursor_from_name: Some("pointer"),
            // set_tooltip: &self.entry.name.text,

            connect_activate[sender, index] => move |_| {
                sender.output(EntryOutput::Activate(index.clone())).unwrap();
            },

            add_controller = GestureClick {
                connect_pressed[sender, index] => move |_, _, _, _| {
                    sender.output(EntryOutput::ButtonDown(index.clone(), false)).unwrap();
                },
            },

            add_controller = GestureClick {
                set_button: gdk::BUTTON_SECONDARY,

                connect_pressed[sender, index] => move |_, _, _, _| {
                    sender.output(EntryOutput::ButtonDown(index.clone(), true)).unwrap();
                },
            },

            GBox {
                set_orientation: Vertical,

                append = match &self.entry.icon {
                    EntryIcon::Name(value) => {
                        Image {
                            #[watch]
                            set_icon_name: Some(value),
                            set_pixel_size: 48,
                            set_vexpand: true,
                            set_valign: Align::End,
                            add_css_class: "icon",
                        }
                    },
                    EntryIcon::Path(value) => {
                        Image {
                            #[watch]
                            set_from_file: Some(value),
                            set_pixel_size: 48,
                            set_vexpand: true,
                            set_valign: Align::End,
                            add_css_class: "icon",
                        }
                    },
                    EntryIcon::Text(value) => {
                        Label {
                            #[watch]
                            set_label: &value,
                            add_css_class: "icon",
                        }
                    },
                    EntryIcon::None => Image::new(),
                },

                Label {
                    set_label: &self.entry.name.text,
                    set_ellipsize: EllipsizeMode::End,
                    set_lines: 2,
                    set_vexpand: true,
                    set_justify: Justification::Center,
                    add_css_class: "grid_name",
                },
            }
        }
    }

    fn init_model(value: Self::Init, _index: &DynamicIndex, _sender: FactorySender<Self>) -> Self {
        Self {
            plugin: value.0,
            entry: value.1,
            selected: false,
            grid_size: value.2,
        }
    }

    fn update(&mut self, message: Self::Input, _: FactorySender<Self>) {
        match message {
            EntryMsg::Select => self.selected = true,
            EntryMsg::Unselect => self.selected = false,
        }
    }
}

struct ListEntryComponent {
    plugin: usize,
    entry: Rc<Entry>,
    selected: bool,
    color: PangoColor,
}

fn create_drag_controller(
    path: Option<impl AsRef<Path>>,
    icon: Option<&str>,
    sender: FactorySender<ListEntryComponent>,
) -> DragSource {
    let drag_source = DragSource::new();

    if let Some(path) = path {
        let path_clone = path.as_ref().to_path_buf();

        drag_source.connect_prepare(move |_, _, _| {
            let content = FileList::from_array(&[gtk::gio::File::for_path(&path_clone)]);
            Some(ContentProvider::for_value(&content.to_value()))
        });

        if let Some(icon) = icon {
            let theme = IconTheme::for_display(&Display::default().unwrap());
            let icon = theme.lookup_icon(
                icon,
                &[],
                48,
                1,
                gtk::TextDirection::Ltr,
                gtk::IconLookupFlags::empty(),
            );
            drag_source.set_icon(Some(&icon), 0, 0);
        }

        let sender_clone = sender.clone();
        drag_source.connect_drag_begin(move |_, _| {
            sender_clone.output(EntryOutput::DragStart).unwrap();
        });

        let sender_clone = sender.clone();
        drag_source.connect_drag_end(move |_, _, _| {
            sender_clone.output(EntryOutput::DragEnd).unwrap();
        });
    }

    drag_source
}

#[relm4::factory]
impl FactoryComponent for ListEntryComponent {
    type Init = (usize, Rc<Entry>, PangoColor);
    type Input = EntryMsg;
    type Output = EntryOutput;
    type CommandOutput = ();
    type ParentWidget = ListBox;

    view! {
        #[root]
        ListBoxRow {
            #[watch]
            set_class_active: ("selected", self.selected),
            set_cursor_from_name: Some("pointer"),

            connect_activate[sender, index] => move |_| {
                sender.output(EntryOutput::Activate(index.clone())).unwrap()
            },

            add_controller = GestureClick {
                connect_pressed[sender, index] => move |_, _, _, _| {
                    sender.output(EntryOutput::ButtonDown(index.clone(), false)).unwrap();
                },
            },

            add_controller = GestureClick {
                set_button: gdk::BUTTON_SECONDARY,

                connect_pressed[sender, index] => move |_, _, _, _| {
                    sender.output(EntryOutput::ButtonDown(index.clone(), true)).unwrap();
                },
            },

            add_controller: create_drag_controller(
                self.entry.drag_file.as_ref(),
                match &self.entry.icon {
                    EntryIcon::Name(name) => Some(name),
                    // EntryIcon::Path(path_buf) => todo!(),
                    // EntryIcon::Character(_) => todo!(),
                    _ => None,
                },
                sender,
            ),

            GBox {
                set_orientation: Vertical,

                GBox {
                    Overlay {
                        #[wrap(Some)]
                        set_child = match &self.entry.icon {
                            EntryIcon::Name(value) => {
                                Image {
                                    #[watch]
                                    set_icon_name: Some(value),
                                    set_use_fallback: true,
                                    set_pixel_size: 48,
                                    add_css_class: "icon",
                                }
                            },
                            EntryIcon::Path(value) => {
                                Image {
                                    #[watch]
                                    set_from_file: Some(value),
                                    set_use_fallback: true,
                                    set_pixel_size: 48,
                                    add_css_class: "icon",
                                }
                            },
                            EntryIcon::Text(value) => {
                                Label {
                                    #[watch]
                                    set_label: &value,
                                    add_css_class: "icon",
                                }
                            },
                            EntryIcon::None => Image::new(),
                        },

                        add_overlay = &self.entry.small_icon.to_gtk_image() {
                            set_use_fallback: true,
                            set_pixel_size: 24,
                            set_halign: Align::End,
                            set_valign: Align::End,
                            add_css_class: "icon",
                            add_css_class: "small_icon",
                        },
                    },

                    GBox {
                        set_orientation: Vertical,
                        set_valign: Align::Center,
                        set_hexpand: true,
                        add_css_class: "texts",

                        GBox {
                            Label {
                                set_label: &self.entry.name.text,
                                set_attributes: Some(&self.entry.name.to_attr_list(self.color.into())),
                                set_ellipsize: EllipsizeMode::End,
                                set_halign: Align::Start,
                                add_css_class: "name",
                            },

                            match &self.entry.tag {
                                Some(tag) => {
                                    Label {
                                        #[watch]
                                        set_label: &tag.text,
                                        #[watch]
                                        set_attributes: Some(&tag.to_attr_list(self.color.into())),
                                        set_ellipsize: EllipsizeMode::End,
                                        set_halign: Align::End,
                                        set_hexpand: true,
                                        add_css_class: "tag",
                                        set_lines: 1,
                                    }
                                }
                                None => {
                                    GBox {}
                                }
                            }
                        },

                        match &self.entry.description {
                            Some(description) => {
                                Label {
                                    #[watch]
                                    set_label: &description.text,
                                    #[watch]
                                    set_attributes: Some(&description.to_attr_list(self.color.into())),
                                    set_ellipsize: EllipsizeMode::End,
                                    set_halign: Align::Start,
                                    add_css_class: "description",
                                    set_lines: 1,
                                }
                            }
                            None => {
                                GBox {}
                            }
                        }
                    },
                },
            },
        }
    }

    fn init_model(value: Self::Init, index: &DynamicIndex, _sender: FactorySender<Self>) -> Self {
        Self {
            plugin: value.0,
            entry: value.1,
            selected: index.current_index() == 0,
            color: value.2,
        }
    }

    fn update(&mut self, message: Self::Input, _: FactorySender<Self>) {
        match message {
            EntryMsg::Select => self.selected = true,
            EntryMsg::Unselect => self.selected = false,
        }
    }
}

#[derive(Debug)]
enum MoveDirection {
    Back,
    Forward,
    Start,
    End,
    PageUp,
    PageDown,
    Up,
    Down,
    Left,
    Right,
}

#[derive(Debug)]
enum AppMsg {
    Search(String),
    Activate(usize),
    ActivateSelected,
    ActivateSelectedWithAction(usize),
    Shortcut(Key, ModifierType),
    ClearPrefix,
    Move(MoveDirection),
    SelectEntry(usize, bool),
    ScrollToSelected,
    ScrollToStart,
    Escape,
    Show,
    Hide,
    MaybeHide,
    Toggle,
    ToggleActions,
    Reload,
    SearchResults(Vec<(usize, Entry)>),
    PluginLoaded(Box<dyn Plugin>),
    SetDragging(bool),
    ToggleLock,
}

#[derive(Debug)]
enum CommandMsg {}

fn default_highlight_color() -> PangoColor {
    "#A2C9FE".parse().unwrap()
}

fn default_window_size() -> [usize; 2] {
    [760, 760]
}

#[derive(Debug, Deserialize, Default)]
struct AppConfig {
    drag_command: Option<String>,
    drop_command: Option<String>,
    #[serde(default = "default_highlight_color")]
    highlight_color: PangoColor,
    #[serde(default = "default_window_size")]
    window_size: [usize; 2],
}

struct AppModel {
    query: String,
    thread_handle: Option<stoppable_thread::StoppableHandle<()>>,
    plugins: Arc<RwLock<Vec<Box<dyn Plugin>>>>,
    plugins_fn: Vec<fn(&Context) -> Box<dyn Plugin>>,
    selected_plugin: Option<usize>,
    selected_entry: usize,
    list_entries: FactoryVecDeque<ListEntryComponent>,
    grid_entries: FactoryVecDeque<GridEntryComponent>,
    grid_size: usize,
    search_entry: Controller<SearchEntryModel>,
    visible: bool,
    context: Arc<RwLock<Context>>,
    dragging: bool,
    config: AppConfig,
    css_provider: CssProvider,
    selected_action: Option<usize>,
    loading: bool,
    locked: bool,
}

impl AppModel {
    fn use_grid(&self) -> bool {
        self.selected_plugin.is_none() && self.query.is_empty()
    }

    fn current_entry(&self) -> Option<&Rc<Entry>> {
        self.get_entry(self.selected_entry)
    }

    fn get_entry(&self, index: usize) -> Option<&Rc<Entry>> {
        if self.use_grid() {
            self.grid_entries.get(index).map(|x| &x.entry)
        } else {
            self.list_entries.get(index).map(|x| &x.entry)
        }
    }

    fn execute_action(&mut self, action: &EntryAction, sender: AsyncComponentSender<Self>) {
        match action {
            EntryAction::Close => sender.input(AppMsg::Hide),
            EntryAction::Write { text, .. } => {
                self.search_entry.emit(text.clone());
            }
            EntryAction::ChangePlugin(plugin) => {
                self.selected_plugin = *plugin;
                self.search_entry.emit(String::new());
            }
            EntryAction::Open(app, action, path, _) => {
                let context = self.context.read();
                if let Some(app) = context.apps.app_map.get(app) {
                    let args = match path {
                        Some(path) => vec![path.to_string_lossy().to_string()],
                        None => vec![],
                    };
                    if match action {
                        Some(action) => context.apps.launch_action(app, action, &args),
                        None => context.apps.launch(app, &args),
                    } {
                        sender.input(AppMsg::MaybeHide);
                    }
                }
            }
            EntryAction::LaunchTerminal {
                program,
                arguments,
                working_directory,
            } => {
                if let Some(emulator) = self.context.read().apps.terminal_emulator() {
                    let mut command = Command::new(emulator.program());

                    if let Some(working_directory) = working_directory
                        && let Some(arg) = &emulator.terminal_args.dir
                    {
                        if arg.ends_with('=') {
                            command.arg(format!("{arg}{}", working_directory.to_string_lossy()));
                        } else {
                            command.arg(arg);
                            command.arg(working_directory);
                        }
                    }

                    if let Some(program) = program {
                        command.arg(emulator.terminal_args.exec.as_deref().unwrap_or("-e"));

                        command.arg(program);
                        command.args(arguments);
                    }

                    match command.spawn_detached() {
                        Err(error) => println!(
                            "Failed to start terminal {:?} {:?}",
                            command.get_args(),
                            error
                        ),
                        _ => sender.input(AppMsg::MaybeHide),
                    }
                }
            }
            EntryAction::Copy(value, _) => {
                let mut opts = wl_clipboard_rs::copy::Options::new();
                opts.foreground(true);
                opts.copy(
                    wl_clipboard_rs::copy::Source::Bytes(value.bytes().collect()),
                    wl_clipboard_rs::copy::MimeType::Autodetect,
                )
                .expect("Failed to serve copy bytes");

                sender.input(AppMsg::MaybeHide);
            }
            EntryAction::Shell(exec) => {
                sender.input(AppMsg::MaybeHide);

                Dispatch::call(DispatchType::Exec(exec)).unwrap();

                // Command::new(shell.as_deref().unwrap_or("sh"))
                //     .arg("-c")
                //     .arg(exec)
                //     .current_dir(
                //         path.as_ref()
                //             .filter(|x| x.exists())
                //             .unwrap_or(&std::env::current_dir().unwrap()),
                //     )
                //     .exec();
            }
            EntryAction::Command {
                command,
                args,
                path,
                ..
            } => {
                sender.input(AppMsg::MaybeHide);

                Command::new(command)
                    .args(args)
                    .current_dir(
                        path.as_ref()
                            .filter(|x| x.exists())
                            .unwrap_or(&std::env::current_dir().unwrap()),
                    )
                    .spawn()
                    .unwrap()
                    .wait()
                    .unwrap();
            }
            EntryAction::HyprctlExec(value) => {
                Dispatch::call(DispatchType::Exec(value)).unwrap();
                sender.input(AppMsg::MaybeHide);
            }
        }
    }
}

fn widget_for_keybind(description: &str, key: Key, modifier: ModifierType) -> Button {
    view! {
        res = Button {
            add_css_class: "keybind",
            set_can_focus: false,
            set_cursor_from_name: Some("pointer"),

            GBox {
                Label {
                    set_label: description,
                    add_css_class: "description",
                },

                Label {
                    set_label: "keyboard_command_key",
                    add_css_class: "key_symbol",
                    set_visible: modifier.contains(ModifierType::SUPER_MASK),
                },

                Label {
                    set_label: "keyboard_control_key",
                    add_css_class: "key_symbol",
                    set_visible: modifier.contains(ModifierType::CONTROL_MASK),
                },

                Label {
                    set_label: "keyboard_option_key",
                    add_css_class: "key_symbol",
                    set_visible: modifier.contains(ModifierType::ALT_MASK),
                },

                Label {
                    set_label: "shift_lock",
                    add_css_class: "key_symbol",
                    set_visible: modifier.contains(ModifierType::SHIFT_MASK),
                },

                Label {
                    set_label: &match key {
                        Key::Return => "keyboard_return".to_owned(),
                        _ => key
                            .name()
                                .map(|x| x.to_uppercase())
                                .unwrap_or("<none>".to_owned()),
                        },
                        add_css_class: if key == Key::Return { "key_symbol" } else { "key" },
                    }
                }
            }
    }

    res
}

const MASK_ICONS: [(ModifierType, &str); 4] = [
    (ModifierType::SUPER_MASK, "keyboard_command_key"),
    (ModifierType::CONTROL_MASK, "keyboard_control_key"),
    (ModifierType::ALT_MASK, "keyboard_option_key"),
    (ModifierType::SHIFT_MASK, "shift"),
];

fn create_actions_box(
    actions: &[(EntryAction, gtk::gdk::Key, ModifierType)],
    index: usize,
    context: &Context,
    sender: &AsyncComponentSender<AppModel>,
) -> GBox {
    let result = GBox::new(Orientation::Vertical, 0);
    result.add_css_class("actions_box");

    for (i, (action, key, modifier)) in actions.iter().enumerate() {
        let action_box = GBox::default();
        action_box.add_css_class("action");
        action_box.set_class_active("selected", i == index);

        {
            let icon = Image::from_icon_name(&action.icon(context));
            icon.set_pixel_size(24);
            icon.set_halign(Align::Start);
            icon.add_css_class("action_icon");
            action_box.append(&icon);
        }

        {
            let label = Label::new(Some(&action.description()));
            label.set_halign(Align::Start);
            label.set_hexpand(true);
            label.add_css_class("action_name");
            action_box.append(&label);
        }

        for (mask, icon) in MASK_ICONS {
            if modifier.contains(mask) {
                let label = Label::new(Some(icon));
                label.add_css_class("key_symbol");
                label.set_halign(Align::End);
                label.set_valign(Align::Center);
                action_box.append(&label);
            }
        }

        if *key != Key::Escape {
            let label = Label::new(Some(&match *key {
                Key::Return => "keyboard_return".to_owned(),
                _ => key
                    .name()
                    .map(|x| x.to_uppercase())
                    .unwrap_or("<none>".to_owned()),
            }));
            label.set_halign(Align::End);
            label.set_valign(Align::Center);
            label.add_css_class(if *key == Key::Return {
                "key_symbol"
            } else {
                "key"
            });
            action_box.append(&label);
        }

        let button = Button::new();
        button.set_can_focus(false);
        button.set_cursor_from_name(Some("pointer"));
        button.set_child(Some(&action_box));
        {
            let sender = sender.clone();
            button.connect_clicked(move |_| {
                sender.input(AppMsg::ActivateSelectedWithAction(i));
            });
        }

        result.append(&button);
    }

    result
}

#[relm4::component(async)]
impl AsyncComponent for AppModel {
    type Input = AppMsg;
    type Output = ();
    type CommandOutput = CommandMsg;

    type Init = (AppConfig, Vec<fn(&Context) -> Box<dyn Plugin>>, CssProvider);

    view! {
        Window {
            set_title: Some("Jogger"),
            #[watch]
            set_default_size: (
                model.config.window_size[0] as i32,
                model.config.window_size[1] as i32,
            ),
            #[watch]
            set_visible: model.visible,

            init_layer_shell: (),
            set_namespace: Some("jogger"),
            set_layer: Layer::Overlay,
            set_keyboard_mode: KeyboardMode::OnDemand,

            #[watch]
            set_class_active: ("dragging", model.dragging),

            GBox {
                set_orientation: Vertical,

                add_controller = EventControllerKey::new() {
                    set_propagation_phase: PropagationPhase::Capture,

                    connect_key_pressed[sender, entry = model.search_entry.widget().clone()] => move |_, key, _, modifier| {
                        let is_empty = entry.text().is_empty();

                        match key {
                            Key::Tab | Key::ISO_Left_Tab => {
                                if modifier.contains(ModifierType::SHIFT_MASK) {
                                    sender.input(AppMsg::Move(MoveDirection::Back));
                                } else {
                                    sender.input(AppMsg::Move(MoveDirection::Forward));
                                }
                                return Propagation::Stop;
                            }
                            Key::Home | Key::KP_Home => {
                                if is_empty {
                                    sender.input(AppMsg::Move(MoveDirection::Start));
                                    return Propagation::Stop;
                                }
                            }
                            Key::End | Key::KP_End => {
                                if is_empty {
                                    sender.input(AppMsg::Move(MoveDirection::End));
                                    return Propagation::Stop;
                                }
                            }
                            Key::Page_Up | Key::KP_Page_Up => {
                                sender.input(AppMsg::Move(MoveDirection::PageUp));
                                return Propagation::Stop;
                            }
                            Key::Page_Down | Key::KP_Page_Down => {
                                sender.input(AppMsg::Move(MoveDirection::PageDown));
                                return Propagation::Stop;
                            }
                            Key::Up | Key::KP_Up => {
                                sender.input(AppMsg::Move(MoveDirection::Up));
                                return Propagation::Stop;
                            }
                            Key::Down | Key::KP_Down => {
                                sender.input(AppMsg::Move(MoveDirection::Down));
                                return Propagation::Stop;
                            }
                            Key::Left | Key::KP_Left => {
                                if is_empty {
                                    sender.input(AppMsg::Move(MoveDirection::Left));
                                    return Propagation::Stop;
                                }
                            }
                            Key::Right | Key::KP_Right => {
                                if is_empty {
                                    sender.input(AppMsg::Move(MoveDirection::Right));
                                    return Propagation::Stop;
                                }
                            }
                            Key::BackSpace => {
                                if is_empty {
                                    sender.input(AppMsg::ClearPrefix);
                                    return Propagation::Stop;
                                }
                            }
                            Key::b => {
                                if modifier == ModifierType::CONTROL_MASK {
                                    sender.input(AppMsg::ToggleActions);
                                    return Propagation::Stop;
                                }
                            }
                            Key::c => {
                                if modifier == ModifierType::CONTROL_MASK && entry.selection_bounds().is_none() {
                                    return Propagation::Stop;
                                }
                            }
                            Key::l => {
                                if modifier == ModifierType::CONTROL_MASK {
                                    sender.input(AppMsg::ToggleLock);
                                    return Propagation::Stop;
                                }
                            }
                            Key::r => {
                                if modifier == ModifierType::CONTROL_MASK {
                                    sender.input(AppMsg::Reload);
                                    return Propagation::Stop;
                                }
                            }
                            Key::F5 => {
                                sender.input(AppMsg::Reload);
                                return Propagation::Stop;
                            },
                            _ => {}
                        }

                        Propagation::Proceed
                    },

                    connect_key_released[sender, entry = model.search_entry.widget().clone()] => move |_, key, _, modifier| {
                        match key {
                            Key::Return | Key::KP_Enter if modifier == ModifierType::NO_MODIFIER_MASK  => {
                                sender.input(AppMsg::ActivateSelected);
                            }
                            Key::Escape => {
                                sender.input(AppMsg::Escape);
                            }
                            Key::c => {
                                if modifier == ModifierType::CONTROL_MASK && entry.selection_bounds().is_none() {
                                    sender.input(AppMsg::Shortcut(key, modifier));
                                }
                            }
                            _ => {
                                if modifier != ModifierType::NO_MODIFIER_MASK {
                                    sender.input(AppMsg::Shortcut(key, modifier));
                                }
                            }
                        };
                    },
                },

                GBox {
                    set_widget_name: "search_bar",

                    Button {
                        set_focusable: false,
                        set_cursor_from_name: Some("pointer"),

                        Image {
                            #[watch]
                            set_icon_name: Some(model.selected_plugin
                                .and_then(|index| {
                                    let plugin = model.plugins.read();
                                    Some(plugin.get(index)?.icon()?.to_owned())
                                })
                                // .and_then(|plugin| plugin.icon())
                                .unwrap_or("edit-find".to_string()))
                            .as_deref()
                        },
                    },

                    append: model.search_entry.widget(),

                    Button {
                        set_focusable: false,
                        set_cursor_from_name: Some("pointer"),
                        set_widget_name: "lock",

                        connect_clicked[sender] => move |_| {
                            sender.input(AppMsg::ToggleLock);
                        },

                        Label {
                            #[watch]
                            set_label: if model.locked { "lock" } else { "lock_open_right" },
                            #[watch]
                            set_class_active: ("active", model.locked),
                        },
                    }
                },

                GBox {
                    add_css_class: "loader",
                    #[watch]
                    set_class_active: ("loading", model.loading),
                },

                Overlay {
                    set_can_focus: false,

                    if model.use_grid() {
                        &GBox {
                            #[local_ref]
                            entries_grid -> Grid {
                                #[watch]
                                set_sensitive: model.selected_action.is_none(),

                                set_row_homogeneous: true,
                                set_column_homogeneous: true,
                                set_expand: true,
                            },
                        }
                    } else {
                        scrolled_window = &ScrolledWindow {
                            #[local_ref]
                            entries -> ListBox {
                                #[watch]
                                set_sensitive: model.selected_action.is_none(),

                                connect_row_activated[sender] => move |_, row| {
                                    sender.input(AppMsg::Activate(row.index() as usize));
                                },
                            },
                        }
                    },

                    add_overlay = &GBox {
                        set_expand: true,
                        #[watch]
                        set_visible: model.selected_action.is_some(),

                        add_controller = GestureClick {
                            connect_released[sender] => move |_, _, _, _| {
                                sender.input(AppMsg::ToggleActions);
                            },
                        },
                    },

                    add_overlay = &Overlay {
                        set_halign: Align::End,
                        set_valign: Align::End,
                        #[watch]
                        set_visible: model.selected_action.is_some(),

                        #[watch]
                        set_child: Some(&create_actions_box(
                            model.current_entry().map_or(&[], |x| &x.actions),
                            model.selected_action.unwrap_or(0),
                            &model.context.read(),
                            &sender,
                        )),
                    },
                },

                CenterBox {
                    set_widget_name: "action_bar",

                    #[wrap(Some)]
                    set_start_widget = &GBox {
                        Image {
                            #[watch]
                            set_icon_name: Some(model.selected_plugin
                                .and_then(|index| {
                                    let plugin = model.plugins.read();
                                    Some(plugin.get(index)?.icon()?.to_owned())
                                })
                                // .and_then(|plugin| plugin.icon())
                                .unwrap_or("edit-find".to_string()))
                            .as_deref()
                        },

                        Label {
                            #[watch]
                            set_label: &model.selected_plugin
                                .and_then(|index| {
                                    let plugin = model.plugins.read();
                                    Some(plugin.get(index)?.name().to_string())
                                })
                                .unwrap_or_default()
                        },
                    },

                    #[wrap(Some)]
                    set_end_widget = &GBox {
                        CenterBox {
                            #[watch]
                            set_end_widget: model.current_entry()
                                .and_then(|x| x.actions.first())
                                .map(|x| {
                                    let sender = sender.clone();
                                    let selected_entry = model.selected_entry;

                                    let button = widget_for_keybind(&x.0.description(), x.1, x.2);
                                    button.connect_clicked(move |_| {
                                        sender.input(AppMsg::Activate(selected_entry))
                                    });
                                    button
                                })
                                .as_ref(),
                        },

                        Separator {
                            set_orientation: Vertical,
                            add_css_class: "separator",
                        },

                        append = &widget_for_keybind("Actions", Key::b, ModifierType::CONTROL_MASK) -> Button {
                            #[watch]
                            set_class_active: ("selected", model.selected_action.is_some()),
                            #[watch]
                            set_sensitive: model.current_entry().filter(|x| x.actions.len() > 1).is_some(),

                            connect_clicked[sender] => move |_| {
                                sender.input(AppMsg::ToggleActions);
                            },
                        },
                    },
                },
            },
        }
    }

    async fn init(
        init: Self::Init,
        root: Self::Root,
        sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        let list_entries = FactoryVecDeque::builder()
            .launch(gtk::ListBox::default())
            .forward(sender.input_sender(), EntryOutput::into);

        let grid_entries = FactoryVecDeque::<GridEntryComponent>::builder()
            .launch(Grid::default())
            .forward(sender.input_sender(), EntryOutput::into);

        let grid_size = 5;

        let search_entry = SearchEntryModel::builder()
            .launch(())
            .forward(sender.input_sender(), AppMsg::Search);

        let model = AppModel {
            query: String::new(),
            thread_handle: None,
            plugins: Arc::new(RwLock::new(Vec::new())),
            plugins_fn: init.1.clone(),
            selected_plugin: None,
            selected_entry: 0,
            list_entries,
            grid_entries,
            grid_size,
            search_entry,
            visible: false,
            context: Arc::new(RwLock::new(Context::default())),
            dragging: false,
            config: init.0,
            css_provider: init.2,
            selected_action: None,
            loading: false,
            locked: false,
        };

        let entries = model.list_entries.widget();
        let entries_grid = model.grid_entries.widget();
        let widgets = view_output!();

        let _sender = sender.clone();
        tokio::spawn(async move {
            let (resource, c) = dbus_tokio::connection::new_session_sync().unwrap();
            let mut cr = Crossroads::new();
            let token = cr.register("com.psyvern.jogger", move |b| {
                let sender = _sender.clone();
                b.method("ShowWindow", (), ("status",), move |_, _, (): ()| {
                    sender.input(AppMsg::Show);
                    Ok((true,))
                });

                let sender = _sender.clone();
                b.method("ToggleWindow", (), ("status",), move |_, _, (): ()| {
                    sender.input(AppMsg::Toggle);
                    Ok((true,))
                });
            });
            cr.insert("/com/psyvern/jogger", &[token], ());
            c.start_receive(
                MatchRule::new_method_call(),
                Box::new(move |msg, conn| {
                    cr.handle_message(msg, conn).unwrap();
                    true
                }),
            );

            let _handle = tokio::spawn(async {
                let err = resource.await;
                panic!("Lost connection to D-Bus: {}", err);
            });

            c.request_name("com.psyvern.jogger.jogger", false, true, false)
                .await
                .unwrap();

            std::future::pending::<()>().await;
        });

        let context = model.context.clone();
        tokio::spawn(async move {
            let context = context.read();
            for plugin in init.1 {
                sender.input(AppMsg::PluginLoaded(plugin(&context)));
            }
        });

        AsyncComponentParts { model, widgets }
    }

    async fn update_with_view(
        &mut self,
        widgets: &mut Self::Widgets,
        message: Self::Input,
        sender: AsyncComponentSender<Self>,
        root: &Self::Root,
    ) {
        let scroll = matches!(message, AppMsg::ScrollToSelected);
        let scroll2 = matches!(message, AppMsg::ScrollToStart);

        self.update(message, sender.clone(), root).await;
        self.update_view(widgets, sender);

        if scroll {
            let list = &widgets.entries;

            if let Some(bounds) = list
                .row_at_index(self.selected_entry as i32)
                .and_then(|row| row.compute_bounds(list))
            {
                let scrolled = &widgets.scrolled_window;
                let adj = scrolled.vadjustment();
                if f64::from(bounds.y()) < adj.value() {
                    adj.set_value(bounds.y().into());
                } else if f64::from(bounds.y()) + f64::from(bounds.height())
                    > adj.value() + adj.page_size()
                {
                    // additional vertical spacing of the list
                    let spacing = 16.0;
                    adj.set_value(
                        f64::from(bounds.y()) + f64::from(bounds.height()) - adj.page_size()
                            + spacing,
                    );
                }
                scrolled.set_vadjustment(Some(&adj));
            }
        }

        if scroll2 {
            let scrolled = &widgets.scrolled_window;
            let adj = scrolled.vadjustment();
            adj.set_value(0.0);
            scrolled.set_vadjustment(Some(&adj));
        }
    }

    async fn update(
        &mut self,
        message: Self::Input,
        sender: AsyncComponentSender<Self>,
        root: &Self::Root,
    ) {
        match message {
            AppMsg::Search(query) => {
                if self.use_grid() {
                    self.grid_entries
                        .try_send(self.selected_entry, EntryMsg::Unselect);
                    self.grid_entries.try_send(0, EntryMsg::Select);
                }

                self.selected_entry = 0;
                self.selected_action = None;
                self.query = query;

                if self.selected_plugin.is_none() && !self.query.is_empty() {
                    let plugin = self.plugins.read();
                    let plugin = plugin
                        .iter()
                        .enumerate()
                        .find(|(_, plugin)| plugin.prefix().is_some_and(|x| x == self.query));

                    if plugin.is_some() {
                        self.search_entry.widget().set_text("");
                        self.selected_plugin = plugin.map(|(i, _)| i);
                        return;
                    }
                };

                if let Some(handle) = self.thread_handle.take() {
                    handle.stop();
                }

                if !self.query.is_empty() || self.selected_plugin.is_some() {
                    self.loading = true;

                    let plugins = self.plugins.clone();
                    let selected_plugin = self.selected_plugin;
                    let query = self.query.clone();
                    let context = self.context.clone();
                    self.thread_handle = Some(stoppable_thread::spawn(move |stopped| {
                        let context = context.read();
                        let entries = {
                            let plugins = plugins.read();
                            match selected_plugin.and_then(|i| Some((i, plugins.get(i)?))) {
                                None => {
                                    if query.starts_with(['~', '/']) {
                                        Files::new(&context)
                                            .search(&query, &context)
                                            .map(|x| (999, x))
                                            .collect_vec()
                                    } else {
                                        plugins
                                            .iter()
                                            .enumerate()
                                            .filter(|x| x.1.has_entry())
                                            .filter(|x| {
                                                x.1.name()
                                                    .to_lowercase()
                                                    .contains(&query.to_lowercase())
                                            })
                                            .map(|(i, x)| {
                                                (
                                                    i,
                                                    Entry {
                                                        name: FormattedString::plain(x.name()),
                                                        tag: None,
                                                        description: None,
                                                        icon: EntryIcon::from(
                                                            x.icon().map(str::to_owned),
                                                        ),
                                                        small_icon: EntryIcon::None,
                                                        actions: vec![
                                                            EntryAction::ChangePlugin(Some(i))
                                                                .into(),
                                                        ],
                                                        id: String::new(),
                                                        ..Default::default()
                                                    },
                                                )
                                            })
                                            .chain(
                                                plugins
                                                    .iter()
                                                    .enumerate()
                                                    .filter(|x| !x.1.has_entry())
                                                    .filter(|(_, x)| x.prefix().is_none())
                                                    .flat_map(|(i, x)| {
                                                        x.search(&query, &context)
                                                            .into_iter()
                                                            .map(move |x| (i, x))
                                                    }),
                                            )
                                            .collect_vec()
                                    }
                                }
                                Some((i, plugin)) => plugin
                                    .search(&query, &context)
                                    .into_iter()
                                    .map(|x| (i, x))
                                    .collect_vec(),
                            }
                        };

                        if !stopped.get() {
                            sender.input(AppMsg::SearchResults(entries));
                        }
                    }));
                } else {
                    sender.input(AppMsg::SearchResults(vec![]))
                }
            }
            AppMsg::Activate(index) => {
                let entry = self.get_entry(index);

                if let Some((action, _, _)) = entry.and_then(|x| x.actions.first()) {
                    self.execute_action(&action.clone(), sender);
                }
            }
            AppMsg::Shortcut(key, modifier) => {
                let entry = self.current_entry();
                let key = key.to_lower();

                if let Some(entry) = entry {
                    for (action, a_key, a_modifier) in &entry.actions {
                        if key == *a_key && modifier == *a_modifier {
                            self.execute_action(&action.clone(), sender);
                            break;
                        }
                    }
                }
            }
            AppMsg::Escape => {
                if self.selected_action.is_some() {
                    self.selected_action = None;
                } else {
                    sender.input(AppMsg::Hide);
                }
            }
            AppMsg::Show => {
                self.visible = true;

                for plugin in self.plugins.write().iter_mut() {
                    plugin.open();
                }

                self.grid_entries.try_send(0, EntryMsg::Select);
            }
            AppMsg::Hide => {
                self.visible = false;
                self.selected_action = None;
                self.search_entry.widget().set_text("");
                self.thread_handle = None;
                self.selected_plugin = None;
                self.selected_entry = 0;
                self.grid_entries.broadcast(EntryMsg::Unselect);
                self.locked = false;
            }
            AppMsg::MaybeHide => {
                if self.locked {
                    self.selected_action = None;
                } else {
                    sender.input(AppMsg::Hide);
                }
            }
            AppMsg::Toggle => sender.input(if self.visible {
                AppMsg::Hide
            } else {
                AppMsg::Show
            }),
            AppMsg::ToggleActions => {
                if let Some(entry) = self.current_entry()
                    && entry.actions.len() > 1
                {
                    self.selected_action = match self.selected_action {
                        Some(_) => None,
                        None => Some(0),
                    }
                }
            }
            AppMsg::Reload => {
                self.context = Arc::new(RwLock::new(Context::default()));

                self.plugins.write().clear();

                {
                    let sender = sender.clone();
                    let plugins = self.plugins_fn.clone();
                    let context = self.context.clone();
                    tokio::spawn(async move {
                        let context = context.read();
                        for plugin in plugins {
                            sender.input(AppMsg::PluginLoaded(plugin(&context)));
                        }
                    });
                }

                sender.input(AppMsg::ScrollToStart);

                let base_dirs = BaseDirectories::with_prefix("jogger").unwrap();

                let config = base_dirs.place_config_file("config.toml").unwrap();
                let config: AppConfig = if std::fs::exists(&config).unwrap_or(false) {
                    let content = std::fs::read_to_string(&config).unwrap();
                    toml::from_str(&content).unwrap()
                } else {
                    Default::default()
                };

                self.config = config;

                load_css(&base_dirs, &self.config, &self.css_provider);
            }
            AppMsg::Move(direction) => {
                if let Some(action) = self.selected_action {
                    let action = action as isize;
                    let Some(entry) = self.current_entry() else {
                        return;
                    };
                    let count = entry.actions.len() as isize;
                    let action = match direction {
                        MoveDirection::Back | MoveDirection::Up => action - 1,
                        MoveDirection::Forward | MoveDirection::Down => action + 1,
                        MoveDirection::Start => 0,
                        MoveDirection::End => count - 1,
                        _ => action,
                    };

                    self.selected_action = Some(action.rem_euclid(count) as usize);
                    return;
                }

                let use_grid = self.use_grid();
                if if use_grid {
                    self.grid_entries.is_empty()
                } else {
                    self.list_entries.is_empty()
                } {
                    return;
                }

                let move_grid = |f: fn(i32, i32) -> i32| -> usize {
                    f(self.selected_entry as i32, self.grid_size as i32) as usize
                };
                let move_list = |f: fn(i32, i32) -> i32| -> usize {
                    let size = self.list_entries.len() as i32;
                    f(self.selected_entry as i32, size) as usize
                };
                let move_grid_back = || move_grid(|i, size| (i - 1).rem_euclid(size * size));
                let move_grid_forward = || move_grid(|i, size| (i + 1).rem_euclid(size * size));
                let move_grid_left = || {
                    move_grid(|i, size| {
                        let row = i / size;
                        let column = i % size;
                        row * size + (column - 1).rem_euclid(size)
                    })
                };
                let move_grid_right = || {
                    move_grid(|i, size| {
                        let row = i / size;
                        let column = i % size;
                        row * size + (column + 1).rem_euclid(size)
                    })
                };
                let move_grid_up = || move_grid(|i, size| (i - size).rem_euclid(size * size));
                let move_grid_down = || move_grid(|i, size| (i + size).rem_euclid(size * size));
                let move_list_back = || move_list(|i, size| (i - 1).rem_euclid(size));
                let move_list_forward = || move_list(|i, size| (i + 1).rem_euclid(size));

                let new = match direction {
                    MoveDirection::Back => {
                        if use_grid {
                            move_grid_back()
                        } else {
                            move_list_back()
                        }
                    }
                    MoveDirection::Forward => {
                        if use_grid {
                            move_grid_forward()
                        } else {
                            move_list_forward()
                        }
                    }
                    MoveDirection::Start => {
                        if use_grid {
                            move_grid(|_, _| 0)
                        } else {
                            self.selected_entry
                        }
                    }
                    MoveDirection::End => {
                        if use_grid {
                            move_grid(|_, size| size * size - 1)
                        } else {
                            self.selected_entry
                        }
                    }
                    MoveDirection::Up => {
                        if use_grid {
                            move_grid_up()
                        } else {
                            move_list_back()
                        }
                    }
                    MoveDirection::Down => {
                        if use_grid {
                            move_grid_down()
                        } else {
                            move_list_forward()
                        }
                    }
                    MoveDirection::Left => {
                        if use_grid {
                            move_grid_left()
                        } else {
                            self.selected_entry
                        }
                    }
                    MoveDirection::Right => {
                        if use_grid {
                            move_grid_right()
                        } else {
                            self.selected_entry
                        }
                    }
                    _ => self.selected_entry,
                };

                if new != self.selected_entry {
                    sender.input(AppMsg::SelectEntry(new, false));
                }
            }
            AppMsg::SelectEntry(index, secondary) => {
                if self.use_grid() {
                    self.grid_entries
                        .try_send(self.selected_entry, EntryMsg::Unselect);
                    self.grid_entries.try_send(index, EntryMsg::Select);
                } else {
                    self.list_entries
                        .try_send(self.selected_entry, EntryMsg::Unselect);
                    self.list_entries.try_send(index, EntryMsg::Select);
                    sender.input(AppMsg::ScrollToSelected);
                }

                self.selected_entry = index;

                if secondary {
                    sender.input(AppMsg::ToggleActions);
                }
            }
            AppMsg::ActivateSelected => {
                if let Some(action) = self.selected_action {
                    sender.input(AppMsg::ActivateSelectedWithAction(action));
                } else {
                    sender.input(AppMsg::Activate(self.selected_entry));
                }
            }
            AppMsg::ActivateSelectedWithAction(action) => {
                let action = self.current_entry().map(|x| x.actions[action].clone());

                if let Some(action) = action {
                    self.execute_action(&action.0, sender);
                }
            }
            AppMsg::ClearPrefix => {
                self.selected_plugin = None;
            }
            AppMsg::ScrollToSelected => {
                self.list_entries
                    .get(self.selected_entry)
                    .and_then(|entry| {
                        self.plugins.read().get(entry.plugin)?.select(&entry.entry);
                        Some(())
                    });
            }
            AppMsg::ScrollToStart => {}
            AppMsg::SearchResults(entries) => {
                self.loading = false;

                let mut list_entries = self.list_entries.guard();
                list_entries.clear();

                for (a, b) in entries {
                    list_entries.push_back((a, Rc::new(b), self.config.highlight_color));
                }
                sender.input(AppMsg::ScrollToStart);
            }
            AppMsg::PluginLoaded(plugin) => {
                self.plugins.write().push(plugin);
                let plugins = self.plugins.read();
                if plugins.len() == self.plugins_fn.len() {
                    let plugins = self.plugins.read();
                    let entries = plugins
                        .iter()
                        .enumerate()
                        .filter(|(_, x)| x.prefix().is_none())
                        .flat_map(|(i, x)| {
                            x.search("", &self.context.read())
                                .into_iter()
                                .map(move |x| (i, Rc::new(x)))
                        })
                        .take(self.grid_size * self.grid_size);

                    let mut grid_entries = self.grid_entries.guard();
                    grid_entries.clear();
                    for entry in entries {
                        grid_entries.push_back((entry.0, entry.1, self.grid_size));
                    }

                    sender.input(AppMsg::Search(self.query.clone()));
                }
            }
            AppMsg::SetDragging(dragging) => {
                self.dragging = dragging;
                if let Some(surface) = root.surface() {
                    if dragging {
                        surface.set_input_region(&Region::create());
                    } else {
                        unsafe {
                            gdk::ffi::gdk_surface_set_input_region(
                                surface.to_glib_none().0,
                                core::ptr::null_mut(),
                            );
                        }
                    }
                }

                if let Some(command) = if dragging {
                    &self.config.drag_command
                } else {
                    &self.config.drop_command
                } {
                    Command::new("sh").arg("-c").arg(command).output().unwrap();
                }
            }
            AppMsg::ToggleLock => {
                self.locked = !self.locked;
            }
        }
    }
}

fn main() {
    if std::env::args().contains(&"--show".to_string()) {
        let conn = dbus::blocking::Connection::new_session().unwrap();

        let proxy = conn.with_proxy(
            "com.psyvern.jogger.jogger",
            "/com/psyvern/jogger",
            Duration::from_millis(5000),
        );

        let (_status,): (bool,) = proxy
            .method_call("com.psyvern.jogger", "ShowWindow", ())
            .unwrap();
    } else if std::env::args().contains(&"--toggle".to_string()) {
        let conn = dbus::blocking::Connection::new_session().unwrap();

        let proxy = conn.with_proxy(
            "com.psyvern.jogger.jogger",
            "/com/psyvern/jogger",
            Duration::from_millis(5000),
        );

        match proxy.method_call::<(bool,), _, _, _>("com.psyvern.jogger", "ToggleWindow", ()) {
            Ok((_status,)) => {}
            Err(_) => start(),
        };
    } else {
        start();
    }
}

fn load_css(base: &BaseDirectories, config: &AppConfig, provider: &CssProvider) {
    let style = include_str!("../style.scss");
    let custom_style = base.place_config_file("style.scss").unwrap();
    let custom_style = std::fs::read_to_string(custom_style).unwrap_or_default();
    let style = grass::from_string(
        format!(
            "$accent: {};\n{}\n{}",
            config.highlight_color, style, custom_style,
        ),
        &Default::default(),
    )
    .or_else(|_| {
        grass::from_string(
            format!("$accent: {};\n{}", config.highlight_color, style),
            &Default::default(),
        )
    })
    .unwrap_or_default();

    provider.load_from_string(&style);
}

fn start() {
    let plugins = plugin_vec![
        plugins::applications::Applications,
        plugins::hyprland::Hyprland,
        plugins::math::Math,
        plugins::clipboard::Clipboard,
        plugins::commands::Commands,
        plugins::ssh::Ssh,
        plugins::unicode::Unicode,
        plugins::emoji::Emojis,
    ];

    let app = RelmApp::new("com.psyvern.jogger").with_args(Vec::new());

    let base_dirs = BaseDirectories::with_prefix("jogger").unwrap();

    let config = base_dirs.place_config_file("config.toml").unwrap();
    let config: AppConfig = if std::fs::exists(&config).unwrap_or(false) {
        let content = std::fs::read_to_string(&config).unwrap();
        toml::from_str(&content).unwrap()
    } else {
        Default::default()
    };

    let provider = gtk::CssProvider::new();
    load_css(&base_dirs, &config, &provider);
    gtk::style_context_add_provider_for_display(
        &gdk::Display::default().expect("Could not connect to a display."),
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_USER,
    );

    app.run_async::<AppModel>((config, plugins, provider));
    // app.run::<AppModel>(plugins);
}

#[macro_export]
macro_rules! plugin_vec {
    ( $( $x:path ),+ $(,)? ) => {
        {
            let mut temp_vec: Vec<fn(&Context) -> Box<dyn Plugin>> = Vec::new();
            $(
                temp_vec.push(|x| {
                    Box::new(<$x>::new(x))
                });
            )*
            temp_vec
        }
    };
}
