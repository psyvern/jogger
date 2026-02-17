use gtk::{
    EventControllerKey, EventSequenceState, GestureClick, InputHints, PropagationPhase,
    gdk::{self, Key, ModifierType},
    glib::Propagation,
    prelude::{EditableExt, EntryExt, EventControllerExt, GestureExt, GestureSingleExt, WidgetExt},
};
use relm4::{Component, ComponentParts, ComponentSender};

use crate::MoveDirection;

#[derive(Debug)]
pub enum SearchEntryMsg {
    Change(String),
    Move(MoveDirection),
    Activate,
    Shortcut(Key, ModifierType),
    UnselectPlugin,
    Close,
    Reload,
    ToggleActions,
    ToggleLock,
}

pub struct SearchEntryModel {}

#[relm4::component(pub)]
impl Component for SearchEntryModel {
    type Init = ();
    type Input = String;
    type Output = SearchEntryMsg;
    type CommandOutput = ();

    view! {
        gtk::Entry {
            set_hexpand: true,
            set_placeholder_text: Some("Search..."),
            set_input_hints: InputHints::NO_EMOJI,

            connect_changed[sender] => move |entry| {
                sender.output(SearchEntryMsg::Change(entry.text().to_string())).unwrap()
            },

            add_controller = GestureClick {
                set_button: gdk::BUTTON_SECONDARY,
                set_propagation_phase: PropagationPhase::Capture,

                connect_pressed => move |s, _, _, _| {
                    s.set_state(EventSequenceState::Claimed);
                },
            },

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
                        Key::b => {
                            if modifier == ModifierType::CONTROL_MASK {
                                sender.output(SearchEntryMsg::ToggleActions).unwrap();
                                return Propagation::Stop;
                            }
                        }
                        Key::c => {
                            if modifier == ModifierType::CONTROL_MASK && root.selection_bounds().is_none() {
                                return Propagation::Stop;
                            }
                        }
                        Key::l => {
                            if modifier == ModifierType::CONTROL_MASK {
                                sender.output(SearchEntryMsg::ToggleLock).unwrap();
                                return Propagation::Stop;
                            }
                        }
                        Key::r => {
                            if modifier == ModifierType::CONTROL_MASK {
                                sender.output(SearchEntryMsg::Reload).unwrap();
                                return Propagation::Stop;
                            }
                        }
                        Key::F5 => {
                            sender.output(SearchEntryMsg::Reload).unwrap();
                            return Propagation::Stop;
                        },
                        _ => {}
                    }

                    Propagation::Proceed
                },

                connect_key_released[sender, root] => move |_, key, _, modifier| {
                    match key {
                        Key::Return | Key::KP_Enter if modifier == ModifierType::NO_MODIFIER_MASK  => {
                            sender.output(SearchEntryMsg::Activate).unwrap();
                        }
                        Key::Escape => {
                            sender.output(SearchEntryMsg::Close).unwrap();
                        }
                        Key::c => {
                            if modifier == ModifierType::CONTROL_MASK && root.selection_bounds().is_none() {
                                sender.output(SearchEntryMsg::Shortcut(key, modifier)).unwrap();
                            }
                        }
                        _ => {
                            if modifier != ModifierType::NO_MODIFIER_MASK {
                                sender.output(SearchEntryMsg::Shortcut(key, modifier)).unwrap();
                            }
                        }
                    };
                },
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

    fn update(&mut self, message: Self::Input, _: ComponentSender<Self>, root: &Self::Root) {
        root.set_text(&message);
        root.set_position(-1);
    }
}
