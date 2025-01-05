pub mod interface;
mod plugins;
mod search_entry;

use futures::Future;
use std::process::Command;
use std::{os::unix::process::CommandExt, pin::Pin};

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
    Component, ComponentController, ComponentParts, ComponentSender, Controller, FactorySender,
    RelmApp, RelmWidgetExt,
};
use search_entry::{SearchEntryModel, SearchEntryMsg};

#[derive(Debug)]
enum GridEntryMsg {
    Select,
    Unselect,
}

struct GridEntryComponent {
    entry: Entry,
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
    type Init = (Entry, usize);
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
            entry: value.0,
            selected: index.current_index() == 0,
            grid_size: value.1,
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
    entry: Entry,
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
    type Init = Entry;
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
            entry: value,
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
}

#[derive(Debug)]
enum CommandMsg {
    PluginLoaded(Box<dyn Plugin>),
}

struct AppModel {
    query: String,
    plugins: Vec<Box<dyn Plugin>>,
    selected_plugin: Option<usize>,
    selected_entry: usize,
    list_entries: FactoryVecDeque<ListEntryComponent>,
    grid_entries: FactoryVecDeque<GridEntryComponent>,
    grid_size: usize,
    search_entry: Controller<SearchEntryModel>,
}

impl AppModel {
    fn use_grid(&self) -> bool {
        self.selected_plugin.is_none() && self.query.is_empty()
    }
}

#[relm4::component]
impl Component for AppModel {
    type Input = AppMsg;
    type Output = ();
    type CommandOutput = CommandMsg;

    type Init = Vec<fn() -> Pin<Box<dyn Future<Output = Box<dyn Plugin>> + Send>>>;

    view! {
        Window {
            set_title: Some("Jogger"),
            set_default_width: 720,
            set_default_height: 720,

            init_layer_shell: (),
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

    fn init(
        plugins: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        for plugin in plugins {
            sender.oneshot_command(async move { CommandMsg::PluginLoaded(plugin().await) })
        }

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
            plugins: vec![],
            selected_plugin: None,
            selected_entry: 0,
            list_entries,
            grid_entries,
            grid_size,
            search_entry,
        };

        let entries = model.list_entries.widget();
        let entries_grid = model.grid_entries.widget();
        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update_with_view(
        &mut self,
        widgets: &mut Self::Widgets,
        message: Self::Input,
        sender: ComponentSender<Self>,
        root: &Self::Root,
    ) {
        let scroll = matches!(message, AppMsg::ScrollToSelected);

        self.update(message, sender.clone(), root);
        self.update_view(widgets, sender);

        if scroll {
            let list = &widgets.entries;

            list.row_at_index(self.selected_entry as i32)
                .and_then(|row| row.compute_bounds(list))
                .map(|bounds| {
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
                });
        }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>, root: &Self::Root) {
        match message {
            AppMsg::Search(query) => {
                self.selected_entry = 0;
                self.query = query;

                if self.selected_plugin.is_none() {
                    let plugin = self
                        .plugins
                        .iter()
                        .enumerate()
                        .find(|(_, plugin)| plugin.prefix().is_some_and(|x| x == &self.query));

                    if plugin.is_some() {
                        self.search_entry.widget().set_text("");
                        self.selected_plugin = plugin.map(|(i, _)| i);
                        return;
                    }
                };

                let mut list_entries = self.list_entries.guard();
                list_entries.clear();
                match self.selected_plugin.and_then(|i| self.plugins.get_mut(i)) {
                    None => {
                        for entry in self.plugins.iter_mut().flat_map(|x| x.search(&self.query)) {
                            list_entries.push_back(entry);
                        }
                    }
                    Some(plugin) => {
                        for entry in plugin.search(&self.query) {
                            list_entries.push_back(entry);
                        }
                    }
                }
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
                        EntryAction::Close => root.close(),
                        EntryAction::Copy(value) => {
                            let mut opts = wl_clipboard_rs::copy::Options::new();
                            opts.foreground(true);
                            opts.copy(
                                wl_clipboard_rs::copy::Source::Bytes(value.bytes().collect()),
                                wl_clipboard_rs::copy::MimeType::Autodetect,
                            )
                            .expect("Failed to serve copy bytes");

                            root.close();
                        }
                        EntryAction::Shell(exec, shell, path) => {
                            root.close();

                            Command::new(shell.as_deref().unwrap_or("sh"))
                                .arg("-c")
                                .arg(exec)
                                .current_dir(
                                    path.as_ref()
                                        .filter(|x| x.exists())
                                        .unwrap_or(&std::env::current_dir().unwrap()),
                                )
                                .exec();
                        }
                        EntryAction::Command(command, path) => {
                            root.close();

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
                    }
                }
            }
            AppMsg::Close => {
                root.close();
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
            AppMsg::ScrollToSelected => {}
        }
    }

    fn update_cmd(
        &mut self,
        message: Self::CommandOutput,
        sender: ComponentSender<Self>,
        root: &Self::Root,
    ) {
        match message {
            CommandMsg::PluginLoaded(plugin) => {
                self.plugins.push(plugin);
                sender.input(AppMsg::Search(self.query.clone()));

                let mut grid_entries = self.grid_entries.guard();
                grid_entries.clear();
                for entry in self
                    .plugins
                    .iter_mut()
                    .flat_map(|x| x.search(""))
                    .take(self.grid_size * self.grid_size)
                {
                    grid_entries.push_back((entry, self.grid_size));
                }
            }
        }
    }
}

fn main() {
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
            Box::pin(async { Box::new(plugins::rink::Rink::new().await) as Box<dyn Plugin> })
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

    app.run::<AppModel>(plugins);
}
