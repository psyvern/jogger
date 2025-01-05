use rink_core::{ast, loader::gnu_units, parsing::datetime, CURRENCY_FILE};

use std::collections::HashMap;

use crate::{interface::EntryIcon, Entry, EntryAction, Plugin};

#[derive(Debug)]
pub struct Rink {
    context: rink_core::Context,
}

impl Rink {
    pub async fn new() -> Self {
        let mut context = rink_core::Context::new();

        let units = gnu_units::parse_str(rink_core::DEFAULT_FILE.unwrap());
        let dates = datetime::parse_datefile(rink_core::DATES_FILE.unwrap());

        let mut currency_defs = Vec::new();

        // match reqwest::blocking::get("https://rinkcalc.app/data/currency.json") {
        //     Ok(response) => match response.json::<ast::Defs>() {
        //         Ok(mut live_defs) => {
        //             currency_defs.append(&mut live_defs.defs);
        //         }
        //         Err(why) => println!("Error parsing currency json: {}", why),
        //     },
        //     Err(why) => println!("Error fetching up-to-date currency conversions: {}", why),
        // }

        currency_defs.append(&mut gnu_units::parse_str(CURRENCY_FILE.unwrap()).defs);

        context.load(units).unwrap();
        // context
        //     .load(ast::Defs {
        //         defs: currency_defs,
        //     })
        //     .unwrap();
        context.load_dates(dates);

        Self { context }
    }
}

impl Plugin for Rink {
    fn icon(&self) -> Option<&str> {
        Some("accessories-calculator")
    }

    fn search(&mut self, query: &str) -> Box<dyn Iterator<Item = crate::Entry> + '_> {
        let val = rink_core::one_line(&mut self.context, query)
            .map(|x| x.clone())
            .into_iter()
            .map(|result| {
                let (title, desc) = parse_result(result.to_string());
                Entry {
                    name: title.clone(),
                    description: desc,
                    icon: EntryIcon::Name("accessories-calculator".to_string()),
                    small_icon: EntryIcon::None,
                    sub_entries: HashMap::new(),
                    action: EntryAction::Copy(title),
                }
            });
        Box::new(val)
    }
}

/// Extracts the title and description from `rink` result.
/// The description is anything inside brackets from `rink`, if present.
fn parse_result(result: String) -> (String, Option<String>) {
    result
        .split_once(" (")
        .map(|(title, desc)| {
            (
                title.to_string(),
                Some(desc.trim_end_matches(')').to_string()),
            )
        })
        .unwrap_or((result, None))
}
