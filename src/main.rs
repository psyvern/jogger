pub mod interface;
mod plugins;
mod search_entry;
pub mod utils;
pub mod xdg_database;

use dbus::channel::MatchingReceiver;
use dbus::message::MatchRule;
use dbus_crossroads::Crossroads;
use gtk::{CenterBox, Separator};
use hyprland::dispatch::{Dispatch, DispatchType};
use itertools::Itertools;
use parking_lot::RwLock;
use relm4::AsyncComponentSender;
use relm4::prelude::{AsyncComponent, AsyncComponentParts};
use std::process::Command;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

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
use search_entry::{SearchEntryModel, SearchEntryMsg};

use crate::interface::{Context, EntryIcon};
use crate::plugins::files::Files;
use crate::utils::CommandExt;

#[derive(Debug, Clone)]
enum GridEntryMsg {
    Select,
    Unselect,
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
    type Input = GridEntryMsg;
    type Output = DynamicIndex;
    type CommandOutput = ();
    type ParentWidget = Grid;

    view! {
        #[root]
        Button {
            set_hexpand: true,
            set_vexpand: true,
            #[watch]
            set_class_active: ("selected", self.selected),

            connect_clicked[sender, index] => move |_| {
                sender.output(index.clone()).unwrap();
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
                    EntryIcon::Character(value) => {
                        Label {
                            #[watch]
                            set_label: &value.to_string(),
                            add_css_class: "icon",
                        }
                    },
                    EntryIcon::None => Image::new(),
                },

                Label {
                    set_label: &self.entry.name,
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
            GridEntryMsg::Select => self.selected = true,
            GridEntryMsg::Unselect => self.selected = false,
        }
    }
}

struct ListEntryComponent {
    plugin: usize,
    entry: Rc<Entry>,
    selected: bool,
}

#[derive(Debug)]
enum EntryMsg {
    Select,
    Unselect,
}

#[relm4::factory]
impl FactoryComponent for ListEntryComponent {
    type Init = (usize, Rc<Entry>);
    type Input = EntryMsg;
    type Output = DynamicIndex;
    type CommandOutput = ();
    type ParentWidget = ListBox;

    view! {
        #[root]
        ListBoxRow {
            #[watch]
            set_class_active: ("selected", self.selected),

            connect_activate[sender, index] => move |_| {
                sender.output(index.clone()).unwrap()
            },

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
                            EntryIcon::Character(value) => {
                                Label {
                                    #[watch]
                                    set_label: &value.to_string(),
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
                                set_label: &self.entry.name,
                                set_use_markup: true,
                                set_ellipsize: EllipsizeMode::End,
                                set_halign: Align::Start,
                                add_css_class: "name",
                            },

                            match &self.entry.tag {
                                Some(tag) => {
                                    Label {
                                        #[watch]
                                        set_label: tag,
                                        set_use_markup: true,
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
                                    set_label: description,
                                    set_use_markup: true,
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
    Activate(usize, bool),
    ActivateSelected(bool),
    ClearPrefix,
    Move(MoveDirection),
    ScrollToSelected,
    ScrollToStart,
    Show,
    Hide,
    Toggle,
    Reload,
    SearchResults(Vec<(usize, Entry)>),
    PluginLoaded(Box<dyn Plugin>),
}

#[derive(Debug)]
enum CommandMsg {}

struct AppModel {
    query: String,
    thread_handle: Option<stoppable_thread::StoppableHandle<()>>,
    plugins: Arc<RwLock<Vec<Box<dyn Plugin>>>>,
    plugins_fn: Vec<fn() -> Box<dyn Plugin>>,
    selected_plugin: Option<usize>,
    selected_entry: usize,
    list_entries: FactoryVecDeque<ListEntryComponent>,
    grid_entries: FactoryVecDeque<GridEntryComponent>,
    grid_size: usize,
    search_entry: Controller<SearchEntryModel>,
    visible: bool,
    context: Arc<RwLock<Context>>,
}

impl AppModel {
    fn use_grid(&self) -> bool {
        self.selected_plugin.is_none() && self.query.is_empty()
    }
}

#[relm4::component(async)]
impl AsyncComponent for AppModel {
    type Input = AppMsg;
    type Output = ();
    type CommandOutput = CommandMsg;

    type Init = Vec<fn() -> Box<dyn Plugin>>;

    view! {
        Window {
            set_title: Some("Jogger"),
            set_default_width: 760,
            set_default_height: 760,
            #[watch]
            set_visible: model.visible,

            init_layer_shell: (),
            set_namespace: "jogger",
            set_layer: Layer::Overlay,
            set_keyboard_mode: KeyboardMode::OnDemand,

            GBox {
                set_orientation: Vertical,

                GBox {
                    set_widget_name: "search_bar",

                    #[watch]
                    set_class_active: ("error", model.selected_plugin.is_some() && model.list_entries.is_empty()),

                    Button {
                        set_focusable: false,

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

                    append: model.search_entry.widget()
                },

                if model.use_grid() {
                    &GBox {
                        #[local_ref]
                        entries_grid -> Grid {
                            set_row_homogeneous: true,
                            set_column_homogeneous: true,
                            set_hexpand: true,
                            set_vexpand: true,
                            set_can_focus: false,
                        },
                    }
                } else {
                    scrolled_window = &ScrolledWindow {
                        #[local_ref]
                        entries -> ListBox {
                            set_can_focus: false,

                            connect_row_activated[sender] => move |_, row| {
                                sender.input(AppMsg::Activate(row.index() as usize, false));
                            },
                        },
                    }
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
                        Button {
                            add_css_class: "keybind",

                            GBox {
                                Label {
                                    #[watch]
                                    set_label: &model.list_entries.get(model.selected_entry)
                                        .and_then(|entry| {
                                            let entry = &entry.entry;
                                            let action = entry.actions.first()?;
                                            Some(match action {
                                                EntryAction::Command(name, _, _) => name.to_owned(),
                                                EntryAction::Open(_, None) => "Run application".to_owned(),
                                                EntryAction::Open(_, Some(_)) => "Open".to_owned(),
                                                EntryAction::Copy(_) => "Copy".to_owned(),
                                                _ => "Run".to_owned(),
                                            })
                                        })
                                        .unwrap_or("Run".to_owned()),
                                    add_css_class: "description",
                                },

                                Label {
                                    set_label: "󰌑\u{2009}",
                                    add_css_class: "key_symbol",
                                },
                            }
                        },

                        Separator {
                            set_orientation: Vertical,
                            add_css_class: "separator",
                        },

                        Button {
                            add_css_class: "keybind",

                            GBox {
                                Label {
                                    #[watch]
                                    set_label: &model.list_entries.get(model.selected_entry)
                                        .map(|_| "Actions".to_owned())
                                        .unwrap_or(" ".to_owned()),
                                    add_css_class: "description",
                                },

                                Label {
                                    set_label: "󰘴",
                                    add_css_class: "key_symbol",
                                },

                                Label {
                                    set_label: "B",
                                    add_css_class: "key",
                                },
                            }
                        },
                    },
                },
            },
        }
    }

    async fn init(
        plugins: Self::Init,
        root: Self::Root,
        sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        let list_entries = FactoryVecDeque::builder()
            .launch(gtk::ListBox::default())
            .forward(sender.input_sender(), |index: DynamicIndex| {
                AppMsg::Activate(index.current_index(), false)
            });

        let grid_entries = FactoryVecDeque::<GridEntryComponent>::builder()
            .launch(Grid::default())
            .forward(sender.input_sender(), |output| {
                AppMsg::Activate(output.current_index(), false)
            });

        let grid_size = 5;

        let search_entry =
            SearchEntryModel::builder()
                .launch(())
                .forward(sender.input_sender(), |output| match output {
                    SearchEntryMsg::Change(query) => AppMsg::Search(query),
                    SearchEntryMsg::Move(direction) => AppMsg::Move(direction),
                    SearchEntryMsg::Activate(second) => AppMsg::ActivateSelected(second),
                    SearchEntryMsg::UnselectPlugin => AppMsg::ClearPrefix,
                    SearchEntryMsg::Close => AppMsg::Hide,
                    SearchEntryMsg::Reload => AppMsg::Reload,
                });

        let model = AppModel {
            query: String::new(),
            thread_handle: None,
            plugins: Arc::new(RwLock::new(Vec::new())),
            plugins_fn: plugins.clone(),
            selected_plugin: None,
            selected_entry: 0,
            list_entries,
            grid_entries,
            grid_size,
            search_entry,
            visible: false,
            context: Arc::new(RwLock::new(Context::default())),
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
        tokio::spawn(async move {
            for plugin in plugins {
                sender.input(AppMsg::PluginLoaded(plugin()));
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
        _: &Self::Root,
    ) {
        match message {
            AppMsg::Search(query) => {
                self.grid_entries
                    .send(self.selected_entry, GridEntryMsg::Unselect);
                self.grid_entries.send(0, GridEntryMsg::Select);

                self.selected_entry = 0;
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
                    let plugins = self.plugins.clone();
                    let selected_plugin = self.selected_plugin;
                    let query = self.query.clone();
                    let context = self.context.clone();
                    self.thread_handle = Some(stoppable_thread::spawn(move |stopped| {
                        let mut context = context.write();
                        let entries = {
                            let plugins = plugins.read();
                            match selected_plugin.and_then(|i| Some((i, plugins.get(i)?))) {
                                None => {
                                    if query.starts_with(['~', '/']) {
                                        Files::new()
                                            .search(&query, &mut context)
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
                                                        name: x.name().to_owned(),
                                                        tag: None,
                                                        description: None,
                                                        icon: EntryIcon::from(
                                                            x.icon().map(str::to_owned),
                                                        ),
                                                        small_icon: EntryIcon::None,
                                                        actions: vec![EntryAction::ChangePlugin(
                                                            Some(i),
                                                        )],
                                                        id: String::new(),
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
                                                        x.search(&query, &mut context)
                                                            .into_iter()
                                                            .map(move |x| (i, x))
                                                    }),
                                            )
                                            .collect_vec()
                                    }
                                }
                                Some((i, plugin)) => plugin
                                    .search(&query, &mut context)
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
            AppMsg::Activate(index, second) => {
                let entry = if self.use_grid() {
                    self.grid_entries.get(index).map(|x| &x.entry)
                } else {
                    self.list_entries.get(index).map(|x| &x.entry)
                };

                if let Some(action) = entry.and_then(|x| {
                    if second {
                        x.actions.get(1)
                    } else {
                        x.actions.first()
                    }
                }) {
                    match action {
                        EntryAction::Close => sender.input(AppMsg::Hide),
                        EntryAction::Write(query) => {
                            self.search_entry.emit(query.clone());
                        }
                        EntryAction::ChangePlugin(plugin) => {
                            self.selected_plugin = *plugin;
                            self.search_entry.emit(String::new());
                        }
                        EntryAction::Open(app, path) => {
                            let context = self.context.read();
                            if let Some(app) = context.apps.app_map.get(app) {
                                context.apps.launch(
                                    app,
                                    &match path {
                                        Some(path) => vec![path.to_string_lossy().to_string()],
                                        None => vec![],
                                    },
                                );
                                sender.input(AppMsg::Hide);
                            }
                        }
                        EntryAction::LaunchTerminal(cmd, args) => {
                            if let Some(emulator) = self.context.read().apps.terminal_emulator() {
                                let program = emulator.program();
                                let mut command = Command::new(&program);

                                command.arg(emulator.terminal_args.exec.as_deref().unwrap_or("-e"));

                                command.arg(cmd);
                                command.args(args);

                                match command.spawn_detached() {
                                    Err(error) => println!(
                                        "Failed to start terminal {:?} {:?}",
                                        command.get_args(),
                                        error
                                    ),
                                    _ => sender.input(AppMsg::Hide),
                                }
                            }
                        }
                        EntryAction::Copy(value) => {
                            let mut opts = wl_clipboard_rs::copy::Options::new();
                            opts.foreground(true);
                            opts.copy(
                                wl_clipboard_rs::copy::Source::Bytes(value.bytes().collect()),
                                wl_clipboard_rs::copy::MimeType::Autodetect,
                            )
                            .expect("Failed to serve copy bytes");

                            sender.input(AppMsg::Hide);
                        }
                        EntryAction::Shell(exec, path) => {
                            sender.input(AppMsg::Hide);

                            Dispatch::call(DispatchType::Exec(&match path {
                                Some(path) => format!("cd {}; {exec}", path.to_string_lossy()),
                                None => exec.to_owned(),
                            }))
                            .unwrap();

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
                        EntryAction::Command(_, command, path) => {
                            sender.input(AppMsg::Hide);

                            let mut iter = command.split_whitespace();
                            Command::new(iter.next().unwrap())
                                .args(iter)
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
                            sender.input(AppMsg::Hide);
                        }
                    }
                }
            }
            AppMsg::Show => {
                self.visible = true;

                for plugin in self.plugins.write().iter_mut() {
                    plugin.open();
                }

                self.grid_entries.send(0, GridEntryMsg::Select);
            }
            AppMsg::Hide => {
                self.visible = false;
                self.search_entry.widget().set_text("");
                self.thread_handle = None;
                self.selected_plugin = None;
                self.selected_entry = 0;
                self.grid_entries.broadcast(GridEntryMsg::Unselect);
            }
            AppMsg::Toggle => sender.input(if self.visible {
                AppMsg::Hide
            } else {
                AppMsg::Show
            }),
            AppMsg::Reload => {
                self.plugins.write().clear();

                {
                    let sender = sender.clone();
                    let plugins = self.plugins_fn.clone();
                    tokio::spawn(async move {
                        for plugin in plugins {
                            sender.input(AppMsg::PluginLoaded(plugin()));
                        }
                    });
                }

                self.context = Arc::new(RwLock::new(Context::default()));

                sender.input(AppMsg::ScrollToStart);
            }
            AppMsg::Move(direction) => {
                let use_grid = self.use_grid();
                if if use_grid {
                    self.grid_entries.is_empty()
                } else {
                    self.list_entries.is_empty()
                } {
                    return;
                }

                let move_grid = |f: fn(i32, i32) -> i32| -> usize {
                    self.grid_entries
                        .send(self.selected_entry, GridEntryMsg::Unselect);
                    let new = f(self.selected_entry as i32, self.grid_size as i32) as usize;
                    self.grid_entries.send(new, GridEntryMsg::Select);
                    new
                };
                let move_list = |f: fn(i32, i32) -> i32| -> usize {
                    let size = self.list_entries.len() as i32;

                    self.list_entries
                        .send(self.selected_entry, EntryMsg::Unselect);
                    let new = f(self.selected_entry as i32, size) as usize;
                    self.list_entries.send(new, EntryMsg::Select);
                    sender.input(AppMsg::ScrollToSelected);
                    new
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

                self.selected_entry = match direction {
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
            }
            AppMsg::ActivateSelected(second) => {
                sender.input(AppMsg::Activate(self.selected_entry, second));
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
                let mut list_entries = self.list_entries.guard();
                list_entries.clear();

                for (a, b) in entries {
                    list_entries.push_back((a, Rc::new(b)));
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
                            x.search("", &mut self.context.write())
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

fn start() {
    let plugins = plugin_vec![
        plugins::applications::Applications,
        plugins::hyprland::Hyprland,
        plugins::math::Math,
        plugins::commands::Commands,
        plugins::ssh::Ssh,
        plugins::unicode::Unicode,
    ];

    let app = RelmApp::new("com.psyvern.jogger").with_args(Vec::new());

    // The CSS "magic" happens here.
    let provider = gtk::CssProvider::new();
    provider.load_from_string(include_str!("../style.css"));
    // We give the CssProvided to the default screen so the CSS rules we added
    // can be applied to our window.
    gtk::style_context_add_provider_for_display(
        &gdk::Display::default().expect("Could not connect to a display."),
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_USER,
    );

    app.run_async::<AppModel>(plugins);
    // app.run::<AppModel>(plugins);
}

#[macro_export]
macro_rules! plugin_vec {
    ( $( $x:path ),+ $(,)? ) => {
        {
            let mut temp_vec: Vec<fn() -> Box<dyn Plugin>> = Vec::new();
            $(
                temp_vec.push(|| {
                    Box::new(<$x>::new())
                });
            )*
            temp_vec
        }
    };
}
