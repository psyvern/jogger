use gtk::{
    EventSequenceState, GestureClick, InputHints, PropagationPhase, gdk,
    prelude::{EditableExt, EntryExt, EventControllerExt, GestureExt, GestureSingleExt, WidgetExt},
};
use relm4::{Component, ComponentParts, ComponentSender};

pub struct SearchEntryModel;

#[relm4::component(pub)]
impl Component for SearchEntryModel {
    type Init = ();
    type Input = String;
    type Output = String;
    type CommandOutput = ();

    view! {
        gtk::Entry {
            set_hexpand: true,
            set_placeholder_text: Some("Search..."),
            set_input_hints: InputHints::NO_EMOJI,

            connect_changed[sender] => move |entry| {
                sender.output(entry.text().to_string()).unwrap()
            },

            add_controller = GestureClick {
                set_button: gdk::BUTTON_SECONDARY,
                set_propagation_phase: PropagationPhase::Capture,

                connect_pressed => move |s, _, _, _| {
                    s.set_state(EventSequenceState::Claimed);
                },
            },
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
