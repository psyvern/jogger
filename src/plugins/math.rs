use std::fmt::Write;
use std::{collections::HashMap, time::Instant};

use fend_core::SpanKind;
use itertools::Itertools;

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
    fn name(&self) -> &str {
        "Calculator"
    }

    fn icon(&self) -> Option<&str> {
        Some("accessories-calculator")
    }

    fn search(&self, query: &str) -> Box<dyn Iterator<Item = Entry>> {
        let val = fend_core::evaluate_preview_with_interrupt(
            query,
            &mut self.context.clone(),
            &CustomInterrupt(Instant::now()),
        );

        if val.get_main_result().is_empty() && val.is_unit_type() {
            Box::new(std::iter::empty())
        } else {
            let mut spans = val.get_main_result_spans().collect_vec().into_iter();
            let first = spans.take_while_ref(|x| x.kind() == SpanKind::Ident).fold(
                String::new(),
                |mut output, x| {
                    let _ = write!(
                        output,
                        "<span color=\"#{:06X}\">{}</span>",
                        if x.string() == "approx. " {
                            0xA2C9FEu32
                        } else {
                            0xFFFFFF
                        },
                        gtk::glib::markup_escape_text(x.string())
                    );
                    output
                },
            );
            let string: String = spans
                .take_while_ref(|x| x.kind() != SpanKind::Ident)
                .filter(|x| !x.string().is_empty())
                .map(|x| x.string().to_owned())
                .collect();
            let units = spans
                .filter(|x| !x.string().is_empty())
                .enumerate()
                .map(|(i, x)| {
                    // 0xA2C9FEFF
                    gtk::glib::markup_escape_text(if i == 0 {
                        x.string().trim_start()
                    } else {
                        x.string()
                    })
                    .to_string()
                })
                .collect();

            let val = Entry {
                name: format!("{first}{string}"),
                tag: None,
                description: Some(units),
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
