use freedesktop_desktop_entry::{default_paths, get_languages_from_env};
use fuzzy_matcher::FuzzyMatcher;
use gtk::glib;
use hyprland::{
    data::{Clients, Workspace},
    shared::{Address, HyprData, HyprDataActive},
};
use itertools::Itertools;
use std::collections::HashMap;

use crate::interface::{Entry, EntryAction, EntryIcon, Plugin};

#[derive(Debug)]
pub struct Hyprland {
    clients: Vec<HyprlandClient>,
}

#[derive(Debug)]
struct HyprlandClient {
    title: String,
    class: String,
    app_name: Option<String>,
    address: Address,
    workspace: i32,
    position: (i16, i16),
    path: Option<String>,
    current: bool,
}

impl From<&HyprlandClient> for Entry {
    fn from(value: &HyprlandClient) -> Self {
        Entry {
            name: format!(
                "{}{}",
                if value.current {
                    "<span color=\"#FFAF00\">🞱 </span>"
                } else {
                    ""
                },
                glib::markup_escape_text(&value.title)
            ),
            description: Some(
                value
                    .app_name
                    .clone()
                    .unwrap_or_else(|| value.class.clone()),
            ),
            icon: EntryIcon::Name(value.path.clone().unwrap_or("image-missing".to_owned())),
            small_icon: EntryIcon::Name("multitasking-windows".to_owned()),
            sub_entries: HashMap::new(),
            action: EntryAction::Command(format!(
                "hyprctl dispatch focuswindow address:{}",
                value.address
            )),
        }
    }
}

impl Hyprland {
    pub async fn new() -> Self {
        let locales = get_languages_from_env();
        let entries: HashMap<_, _> = freedesktop_desktop_entry::Iter::new(default_paths())
            .entries(Some(&locales))
            .flat_map(|entry| {
                let name = entry.name(&locales).map(|x| x.into_owned());
                let icon = entry.icon().map(|x| x.to_owned());
                [
                    entry
                        .path
                        .file_stem()
                        .and_then(|x| x.to_str())
                        .map(|x| (x.to_lowercase(), (name.clone(), icon.clone()))),
                    entry
                        .startup_wm_class()
                        .map(|x| (x.to_lowercase(), (name, icon))),
                ]
            })
            .flat_map(|x| x)
            .collect();

        let current = Workspace::get_active().unwrap().id;
        let clients = Clients::get().unwrap();
        let clients = clients
            .into_iter()
            .map(|x| {
                let data = entries.get(&x.class.to_lowercase());

                let name = data
                    .map(|(x, _)| x)
                    .and_then(Option::as_deref)
                    .map(str::to_owned);

                let icon = data
                    .map(|(_, x)| x)
                    .and_then(Option::as_deref)
                    .map(|x| x.to_string());
                // let path = linicon::lookup_icon(x.class.to_lowercase())
                //     .flat_map(|x| x)
                //     .sorted_by_cached_key(|x| x.max_size)
                //     .rev()
                //     .next()
                //     .map(|x| x.path);

                HyprlandClient {
                    title: x.title,
                    class: x.class,
                    app_name: name,
                    address: x.address,
                    workspace: x.workspace.id,
                    position: x.at,
                    path: icon,
                    current: current == x.workspace.id,
                }
            })
            .collect_vec();

        Self { clients }
    }
}

impl Plugin for Hyprland {
    fn icon(&self) -> Option<&str> {
        Some("multitasking-windows")
    }

    fn prefix(&self) -> Option<&str> {
        Some("w:")
    }

    fn search(&mut self, query: &str) -> Box<dyn Iterator<Item = crate::interface::Entry> + '_> {
        if query.is_empty() {
            Box::new(
                self.clients
                    .iter()
                    .sorted_by_cached_key(|x| (!x.current, x.workspace, x.position))
                    .map(Entry::from),
            )
        } else {
            let matcher = fuzzy_matcher::skim::SkimMatcherV2::default().smart_case();
            // let mut matcher = Matcher::new(Config::DEFAULT.match_paths());
            // let pattern = Pattern::new(
            //     query,
            //     CaseMatching::Ignore,
            //     Normalization::Smart,
            //     AtomKind::Fuzzy,
            // );
            Box::new(
                self.clients
                    .iter()
                    .filter_map(|client| {
                        let mut score = 0;

                        score += 4 * matcher.fuzzy_match(&client.title, &query).unwrap_or(0);

                        score += matcher.fuzzy_match(&client.class, &query).unwrap_or(0);

                        if score == 0 {
                            None
                        } else {
                            Some((score, client))
                        }
                    })
                    .sorted_by_cached_key(|(x, _)| *x)
                    .rev()
                    .map(|(_, x)| Entry::from(x)),
            )
        }
    }
}
