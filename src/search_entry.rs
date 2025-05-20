use gtk::{
    EventControllerKey, PropagationPhase,
    gdk::{Key, ModifierType},
    glib::Propagation,
    prelude::{EditableExt, EntryExt, EventControllerExt, WidgetExt},
};
use relm4::{ComponentParts, ComponentSender, SimpleComponent};

use crate::MoveDirection;

#[derive(Debug)]
pub enum SearchEntryMsg {
    Change(String),
    Move(MoveDirection),
    Activate,
    UnselectPlugin,
    Close,
    Reload,
}

pub struct SearchEntryModel {}

#[relm4::component(pub)]
impl SimpleComponent for SearchEntryModel {
    type Init = ();
    type Input = ();
    type Output = SearchEntryMsg;

    view! {
        root = gtk::Entry {
            set_hexpand: true,
            set_placeholder_text: Some("Search..."),

            connect_changed[sender] => move |entry| { sender.output(SearchEntryMsg::Change(entry.text().to_string())).unwrap() },

            add_controller = EventControllerKey::new() {
                set_propagation_phase: PropagationPhase::Capture,

                connect_key_pressed[sender, root] => move |_, key, _, modifier| {
                    let is_empty = root.text().is_empty();
                    match key {
                        Key::Tab | Key::ISO_Left_Tab => {
                            if modifier.contains(ModifierType::SHIFT_MASK) {
                                sender.output(SearchEntryMsg::Move(MoveDirection::Back)).unwrap();
                            } else {
                                sender.output(SearchEntryMsg::Move(MoveDirection::Forward)).unwrap();
                            }
                            return Propagation::Stop;
                        }
                        Key::Home | Key::KP_Home => {
                            if is_empty {
                                sender.output(SearchEntryMsg::Move(MoveDirection::Start)).unwrap();
                                return Propagation::Stop;
                            }
                        }
                        Key::End | Key::KP_End => {
                            if is_empty {
                                sender.output(SearchEntryMsg::Move(MoveDirection::End)).unwrap();
                                return Propagation::Stop;
                            }
                        }
                        Key::Page_Up | Key::KP_Page_Up => {
                            sender.output(SearchEntryMsg::Move(MoveDirection::PageUp)).unwrap();
                            return Propagation::Stop;
                        }
                        Key::Page_Down | Key::KP_Page_Down => {
                            sender.output(SearchEntryMsg::Move(MoveDirection::PageDown)).unwrap();
                            return Propagation::Stop;
                        }
                        Key::Up | Key::KP_Up => {
                            sender.output(SearchEntryMsg::Move(MoveDirection::Up)).unwrap();
                            return Propagation::Stop;
                        }
                        Key::Down | Key::KP_Down => {
                            sender.output(SearchEntryMsg::Move(MoveDirection::Down)).unwrap();
                            return Propagation::Stop;
                        }
                        Key::Left | Key::KP_Left => {
                            if is_empty {
                                sender.output(SearchEntryMsg::Move(MoveDirection::Left)).unwrap();
                                return Propagation::Stop;
                            }
                        }
                        Key::Right | Key::KP_Right => {
                            if is_empty {
                                sender.output(SearchEntryMsg::Move(MoveDirection::Right)).unwrap();
                                return Propagation::Stop;
                            }
                        }
                        Key::BackSpace => {
                            if is_empty {
                                sender.output(SearchEntryMsg::UnselectPlugin).unwrap();
                                return Propagation::Stop;
                            }
                        }
                        Key::Return | Key::KP_Enter => {
                            sender.output(SearchEntryMsg::Activate).unwrap();
                            return Propagation::Stop;
                        }
                        Key::Escape => {
                            sender.output(SearchEntryMsg::Close).unwrap();
                            return Propagation::Stop;
                        }
                        Key::r => {
                            if modifier.contains(ModifierType::CONTROL_MASK) {
                                sender.output(SearchEntryMsg::Reload).unwrap();
                                return Propagation::Stop;
                            }
                        }
                        Key::F5 => {
                            sender.output(SearchEntryMsg::Reload).unwrap();
                            return Propagation::Stop;
                        }
                        _ => {}
                    };

                    Propagation::Proceed
                }
            }
        }
    }

    fn init(
        _params: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = SearchEntryModel {};
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, _: Self::Input, _: ComponentSender<Self>) {}
}
