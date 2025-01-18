pub mod interface;
mod plugins;
mod search_entry;

use dbus::channel::MatchingReceiver;
use dbus::message::MatchRule;
use dbus_crossroads::Crossroads;
use futures::Future;
use hyprland::dispatch::{Dispatch, DispatchType};
use itertools::Itertools;
use relm4::prelude::{AsyncComponent, AsyncComponentParts};
use relm4::AsyncComponentSender;
use std::os::unix::process::CommandExt;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;
use std::{pin::Pin, process::Command};
use tokio::select;
use tokio_util::sync::CancellationToken;

use gtk::{
    gdk::{self},
    pango::EllipsizeMode,
    prelude::{
        AdjustmentExt, BoxExt, ButtonExt, EditableExt, GridExt, GtkWindowExt, ListBoxRowExt,
        OrientableExt, ToggleButtonExt, WidgetExt,
    },
    Align, Box as GBox, Button, Grid, Image, Justification, Label, ListBox, ListBoxRow,
    Orientation::Vertical,
    Overlay, Revealer, ScrolledWindow, ToggleButton, Window,
};
use gtk_layer_shell::{KeyboardMode, Layer, LayerShell};
use interface::{Entry, EntryAction, Plugin, SubEntry};
use relm4::{
    factory::{positions::GridPosition, Position},
    prelude::{DynamicIndex, FactoryComponent, FactoryVecDeque},
    Component, ComponentController, Controller, FactorySender, RelmApp, RelmWidgetExt,
};
use search_entry::{SearchEntryModel, SearchEntryMsg};

#[derive(Debug)]
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

                self.entry.icon.to_gtk_image() {
                    set_pixel_size: 48,
                    set_vexpand: true,
                    set_valign: Align::End,
                    add_css_class: "icon",
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

    fn init_model(value: Self::Init, index: &DynamicIndex, _sender: FactorySender<Self>) -> Self {
        Self {
            plugin: value.0,
            entry: value.1,
            selected: index.current_index() == 0,
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
    show_actions: bool,
    selected: bool,
}

#[derive(Debug)]
enum EntryMsg {
    ToggleAction(bool),
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
                        set_child = &self.entry.icon.to_gtk_image() {
                            set_use_fallback: true,
                            set_pixel_size: 48,
                            add_css_class: "icon",
                        },

                        add_overlay = &self.entry.small_icon.to_gtk_image() {
                            set_use_fallback: true,
                            set_pixel_size: 24,
                            set_halign: Align::End,
                            set_valign: Align::End,
                            add_css_class: "icon",
                        },
                    },

                    GBox {
                        set_orientation: Vertical,
                        set_valign: Align::Center,
                        set_hexpand: true,
                        add_css_class: "texts",

                        Label {
                            set_label: &self.entry.name,
                            set_use_markup: true,
                            set_ellipsize: EllipsizeMode::End,
                            set_halign: Align::Start,
                            add_css_class: "name",
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
                                }
                            }
                            None => {
                                GBox {}
                            }
                        }
                    },

                    // if !self.actions.is_empty() {
                        ToggleButton {
                            set_icon_name: "go-down-symbolic",
                            set_visible: !self.entry.sub_entries.is_empty(),

                            connect_toggled[sender] => move |x| sender.input(EntryMsg::ToggleAction(x.is_active()))
                        }
                    // }
                },

                Revealer {
                    #[watch]
                    set_reveal_child: self.show_actions,

                    GBox {
                        set_orientation: Vertical,

                        Label {
                            set_label: "Boi",
                        },

                        // #[iterate]
                        // append: self.entry.actions.iter().map(|(_, action)| {
                        //     let label = Label::builder()
                        //         .label(&action.name)
                        //         .halign(Align::Start)
                        //         .build();
                        //     let arg = action.exec.clone().unwrap_or_default();
                        //     let button = Button::builder()
                        //         .child(&label)
                        //         .css_classes(["action"])
                        //         .build();
                        //     button.connect_clicked(move |_| {
                        //         let current_dir = &std::env::current_dir().unwrap();

                        //         Command::new("sh")
                        //             .arg("-c")
                        //             .arg(&arg)
                        //             .current_dir(current_dir)
                        //             // .current_dir(if let Some(path) = &entry.path {
                        //             //     if path.exists() {
                        //             //         path
                        //             //     } else {
                        //             //         current_dir
                        //             //     }
                        //             // } else {
                        //             //     current_dir
                        //             // })
                        //             .spawn()
                        //             .unwrap();
                        //         std::process::exit(0);
                        //     });
                        //     button
                        // })
                    }
                }
            },
        }
    }

    fn init_model(value: Self::Init, index: &DynamicIndex, _sender: FactorySender<Self>) -> Self {
        Self {
            plugin: value.0,
            entry: value.1,
            show_actions: false,
            selected: index.current_index() == 0,
        }
    }

    fn update(&mut self, message: Self::Input, _: FactorySender<Self>) {
        match message {
            EntryMsg::ToggleAction(x) => self.show_actions = x,
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
    Select(usize),
    SelectSelected,
    ClearPrefix,
    Move(MoveDirection),
    ScrollToSelected,
    Close,
    Open,
    SearchResults(Vec<(usize, Entry)>),
    PluginLoaded(Box<dyn Plugin>),
}

enum CommandMsg {}

impl std::fmt::Debug for CommandMsg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "nothing")
    }
}

struct AppModel {
    query: String,
    cancellation_token: CancellationToken,
    plugins: Vec<Arc<dyn Plugin>>,
    plugins_size: usize,
    selected_plugin: Option<usize>,
    selected_entry: usize,
    list_entries: FactoryVecDeque<ListEntryComponent>,
    grid_entries: FactoryVecDeque<GridEntryComponent>,
    grid_size: usize,
    search_entry: Controller<SearchEntryModel>,
    visible: bool,
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

    type Init = Vec<fn() -> Pin<Box<dyn Future<Output = Box<dyn Plugin>> + Send>>>;

    view! {
        Window {
            set_title: Some("Jogger"),
            set_default_width: 720,
            set_default_height: 720,
            #[watch]
            set_visible: model.visible,

            init_layer_shell: (),
            set_namespace: "jogger",
            set_layer: Layer::Overlay,
            set_keyboard_mode: KeyboardMode::Exclusive,

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
                                .and_then(|index| model.plugins.get(index))
                                .and_then(|plugin| plugin.icon())
                                .unwrap_or("edit-find"))
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
                                sender.input(AppMsg::Select(row.index() as usize));
                            },
                        },
                    }
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
                AppMsg::Select(index.current_index())
            });

        let grid_entries = FactoryVecDeque::<GridEntryComponent>::builder()
            .launch(Grid::default())
            .forward(sender.input_sender(), |output| {
                AppMsg::Select(output.current_index())
            });

        let grid_size = 5;

        let search_entry =
            SearchEntryModel::builder()
                .launch(())
                .forward(sender.input_sender(), |output| match output {
                    SearchEntryMsg::Change(query) => AppMsg::Search(query),
                    SearchEntryMsg::Move(direction) => AppMsg::Move(direction),
                    SearchEntryMsg::Select => AppMsg::SelectSelected,
                    SearchEntryMsg::UnselectPlugin => AppMsg::ClearPrefix,
                    SearchEntryMsg::Close => AppMsg::Close,
                });

        let model = AppModel {
            query: String::new(),
            cancellation_token: CancellationToken::new(),
            plugins: vec![],
            plugins_size: plugins.len(),
            selected_plugin: None,
            selected_entry: 0,
            list_entries,
            grid_entries,
            grid_size,
            search_entry,
            visible: false,
        };

        let entries = model.list_entries.widget();
        let entries_grid = model.grid_entries.widget();
        let widgets = view_output!();

        let _sender = sender.clone();
        tokio::spawn(async move {
            let (resource, c) = dbus_tokio::connection::new_session_sync().unwrap();
            let mut cr = Crossroads::new();
            let token = cr.register("com.psyvern.jogger", move |b| {
                b.method("ShowWindow", (), ("result",), move |_, _, (): ()| {
                    _sender.clone().input(AppMsg::Open);
                    Ok(("yea boi",))
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
                sender.input(AppMsg::PluginLoaded(plugin().await));
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
                    scrolled.set_vadjustment(Some(&adj));
                } else if f64::from(bounds.y()) + f64::from(bounds.height())
                    > adj.value() + adj.page_size()
                {
                    adj.set_value(
                        f64::from(bounds.y()) + f64::from(bounds.height()) - adj.page_size(),
                    );
                    scrolled.set_vadjustment(Some(&adj));
                }
            }
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
                self.selected_entry = 0;
                self.query = query;

                if self.selected_plugin.is_none() {
                    let plugin = self
                        .plugins
                        .iter()
                        .enumerate()
                        .find(|(_, plugin)| plugin.prefix().is_some_and(|x| x == self.query));

                    if plugin.is_some() {
                        self.search_entry.widget().set_text("");
                        self.selected_plugin = plugin.map(|(i, _)| i);
                        return;
                    }
                };

                self.cancellation_token.cancel();

                self.cancellation_token = CancellationToken::new();
                let child_token = self.cancellation_token.child_token();

                let plugins = self.plugins.clone();
                let selected_plugin = self.selected_plugin;
                let query = self.query.clone();
                let _sender = sender.clone();
                tokio::spawn(async move {
                    select! {
                        _ = child_token.cancelled() => {}
                        entries = async {
                            match selected_plugin.and_then(|i| Some((i, plugins.get(i)?))) {
                                None => {
                                    plugins.iter().enumerate().flat_map(|(i, x)| x.search(&query).map(move |x| (i, x))).collect_vec()
                                }
                                Some((i, plugin)) => {
                                    plugin.search(&query).map(|x| (i, x)).collect_vec()
                                }
                            }
                        } => {
                            sender.input(AppMsg::SearchResults(entries));
                        }
                    }
                });
            }
            AppMsg::Select(index) => {
                let entry = if self.use_grid() {
                    self.grid_entries.get(index).map(|x| &x.entry)
                } else {
                    self.list_entries.get(index).map(|x| &x.entry)
                };

                if let Some(entry) = entry {
                    match &entry.action {
                        EntryAction::Nothing => {}
                        EntryAction::Close => sender.input(AppMsg::Close),
                        EntryAction::Copy(value) => {
                            let mut opts = wl_clipboard_rs::copy::Options::new();
                            opts.foreground(true);
                            opts.copy(
                                wl_clipboard_rs::copy::Source::Bytes(value.bytes().collect()),
                                wl_clipboard_rs::copy::MimeType::Autodetect,
                            )
                            .expect("Failed to serve copy bytes");

                            sender.input(AppMsg::Close);
                        }
                        EntryAction::Shell(exec, path) => {
                            sender.input(AppMsg::Close);

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
                        EntryAction::Command(command, path) => {
                            sender.input(AppMsg::Close);

                            let mut iter = command.split_whitespace();
                            Command::new(iter.next().unwrap())
                                .args(iter)
                                .current_dir(
                                    path.as_ref()
                                        .filter(|x| x.exists())
                                        .unwrap_or(&std::env::current_dir().unwrap()),
                                )
                                .exec();
                        }
                        EntryAction::HyprctlExec(value) => {
                            Dispatch::call(DispatchType::Exec(value)).unwrap();
                            sender.input(AppMsg::Close);
                        }
                    }
                }
            }
            AppMsg::Close => {
                self.visible = false;
                self.search_entry.widget().set_text("");
                self.cancellation_token = CancellationToken::new();
                self.selected_plugin = None;
                self.selected_entry = 0;
                self.visible = false;
            }
            AppMsg::Open => {
                self.visible = true;
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
            AppMsg::SelectSelected => {
                sender.input(AppMsg::Select(self.selected_entry));
            }
            AppMsg::ClearPrefix => {
                self.selected_plugin = None;
            }
            AppMsg::ScrollToSelected => {
                self.list_entries
                    .get(self.selected_entry)
                    .and_then(|entry| {
                        self.plugins.get(entry.plugin)?.select(&entry.entry);
                        Some(())
                    });
            }
            AppMsg::SearchResults(entries) => {
                let mut entries = entries.into_iter().map(|(a, b)| (a, Rc::new(b)));

                if self.query.is_empty()
                    && self.selected_plugin.is_none()
                    && self.grid_entries.is_empty()
                {
                    let mut grid_entries = self.grid_entries.guard();
                    for entry in entries.by_ref().take(self.grid_size * self.grid_size) {
                        grid_entries.push_back((entry.0, entry.1, self.grid_size));
                    }
                }

                let mut list_entries = self.list_entries.guard();
                list_entries.clear();
                for entry in entries {
                    list_entries.push_back(entry);
                }
            }
            AppMsg::PluginLoaded(plugin) => {
                self.plugins.push(plugin.into());
                if self.plugins.len() == self.plugins_size {
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

        let (has_owner,): (String,) = proxy
            .method_call("com.psyvern.jogger", "ShowWindow", ())
            .unwrap();

        println!("has_owner: {has_owner}");
        return;
    }

    let plugins = {
        fn fn_1() -> Pin<Box<dyn Future<Output = Box<dyn Plugin>> + Send>> {
            Box::pin(async {
                Box::new(plugins::applications::Applications::new().await) as Box<dyn Plugin>
            })
        }
        fn fn_2() -> Pin<Box<dyn Future<Output = Box<dyn Plugin>> + Send>> {
            Box::pin(async {
                Box::new(plugins::hyprland::Hyprland::new().await) as Box<dyn Plugin>
            })
        }
        fn fn_3() -> Pin<Box<dyn Future<Output = Box<dyn Plugin>> + Send>> {
            Box::pin(async { Box::new(plugins::math::Math::new().await) as Box<dyn Plugin> })
        }
        fn fn_4() -> Pin<Box<dyn Future<Output = Box<dyn Plugin>> + Send>> {
            Box::pin(async {
                Box::new(plugins::commands::Commands::new().await) as Box<dyn Plugin>
            })
        }

        vec![fn_1, fn_2, fn_3, fn_4]
    };

    let app = RelmApp::new("com.psyvern.jogger");

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
