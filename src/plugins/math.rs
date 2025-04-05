use std::{collections::HashMap, time::Instant};

use fend_core::SpanKind;

use crate::{Entry, EntryAction, Plugin, interface::EntryIcon};

#[derive(Debug)]
pub struct Math {
    context: fend_core::Context,
}

impl Math {
    pub async fn new() -> Self {
        let context = fend_core::Context::new();
        Self { context }
    }
}

struct CustomInterrupt(Instant);

impl fend_core::Interrupt for CustomInterrupt {
    fn should_interrupt(&self) -> bool {
        self.0.elapsed().as_millis() > 500
    }
}

impl Plugin for Math {
    fn icon(&self) -> Option<&str> {
        Some("accessories-calculator")
    }

    fn search(&self, query: &str) -> Box<dyn Iterator<Item = crate::Entry>> {
        let val = fend_core::evaluate_preview_with_interrupt(
            query,
            &mut self.context.clone(),
            &CustomInterrupt(Instant::now()),
        );

        if val.get_main_result().is_empty() && val.is_unit_type() {
            Box::new(std::iter::empty())
        } else {
            let string = val
                .get_main_result_spans()
                .filter(|x| !x.string().is_empty())
                .map(|x| {
                    if x.kind() == SpanKind::Ident {
                        format!(
                            "<span color=\"#{:08X}\">{}</span>",
                            if x.string() == "approx. " {
                                0xFFFFFF7F_u32
                            } else {
                                0xA2C9FEFF
                            },
                            gtk::glib::markup_escape_text(x.string())
                        )
                    } else {
                        x.string().to_owned()
                    }
                })
                .collect();

            let val = Entry {
                name: string,
                description: None,
                icon: EntryIcon::Name("accessories-calculator".to_string()),
                small_icon: EntryIcon::None,
                sub_entries: HashMap::new(),
                action: EntryAction::Copy(
                    val.get_main_result()
                        .trim_start_matches("approx. ")
                        .to_owned(),
                ),
                id: "".to_owned(),
            };

            Box::new(std::iter::once(val))
        }
    }
}
