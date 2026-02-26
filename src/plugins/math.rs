use std::time::Instant;

use fend_core::SpanKind;
use itertools::Itertools;

use crate::{
    Entry, Plugin,
    interface::{Context, EntryAction, EntryIcon, FormatStyle, FormattedString},
};

#[derive(Debug)]
pub struct Math {
    context: fend_core::Context,
}

impl Math {
    pub fn new(_: &Context) -> Self {
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

    fn search(&self, query: &str, _: &Context) -> Vec<Entry> {
        let val = fend_core::evaluate_preview_with_interrupt(
            query,
            &mut self.context.clone(),
            &CustomInterrupt(Instant::now()),
        );

        if val.get_main_result().is_empty() && val.is_unit_type() {
            Vec::new()
        } else {
            let mut spans = val.get_main_result_spans().collect_vec().into_iter();
            let mut parts = spans
                .take_while_ref(|x| x.kind() == SpanKind::Ident)
                .map(|x| {
                    let x = x.string();
                    (
                        x,
                        if x == "approx. " {
                            Some(FormatStyle::Highlight)
                        } else {
                            None
                        },
                    )
                })
                .collect_vec();
            let string: String = spans
                .take_while_ref(|x| x.kind() != SpanKind::Ident)
                .filter(|x| !x.string().is_empty())
                .map(|x| x.string().to_owned())
                .collect();
            parts.push((&string, None));
            let units: String = spans
                .filter(|x| !x.string().is_empty())
                .enumerate()
                .map(|(i, x)| {
                    // 0xA2C9FEFF
                    if i == 0 {
                        x.string().trim_start()
                    } else {
                        x.string()
                    }
                })
                .collect();

            let val = Entry {
                name: FormattedString::from_styles(parts),
                tag: None,
                description: Some(FormattedString::plain(units)),
                icon: EntryIcon::Name("accessories-calculator".to_string()),
                small_icon: EntryIcon::None,
                actions: vec![EntryAction {
                    icon: "edit-copy".into(),
                    name: "Copy".into(),
                    function: EntryAction::copy(
                        val.get_main_result().trim_start_matches("approx. "),
                    ),
                    ..Default::default()
                }],
                id: "".to_owned(),
                ..Default::default()
            };

            vec![val]
        }
    }
}
