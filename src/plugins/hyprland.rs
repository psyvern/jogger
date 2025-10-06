use freedesktop_desktop_entry::{default_paths, get_languages_from_env};
use fuzzy_matcher::FuzzyMatcher;
use gtk::glib;
use hyprland::{
    data::{Clients, Workspace},
    shared::{Address, HyprData, HyprDataActive, HyprDataActiveOptional},
};
use itertools::Itertools;
use std::collections::HashMap;

use crate::interface::{Context, Entry, EntryAction, EntryIcon, Plugin};

#[derive(Debug)]
pub struct Hyprland {
    entries: HashMap<String, (Option<String>, Option<String>)>,
    clients: Vec<HyprlandClient>,
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Debug)]
enum SelectionStatus {
    Selected,
    SameWorkspace,
    None,
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
    selection_status: SelectionStatus,
}

impl From<&HyprlandClient> for Entry {
    fn from(value: &HyprlandClient) -> Self {
        Entry {
            name: format!(
                "{}{}",
                match value.selection_status {
                    SelectionStatus::None => "",
                    _ => "<span color=\"#FFAF00\">🞱 </span>",
                },
                glib::markup_escape_text(&value.title)
            ),
            tag: Some(format!("Workspace {}", value.workspace)),
            description: Some(
                value
                    .app_name
                    .clone()
                    .unwrap_or_else(|| value.class.clone()),
            ),
            icon: EntryIcon::Name(value.path.clone().unwrap_or("image-missing".to_owned())),
            small_icon: EntryIcon::Name("window".to_owned()),
            actions: vec![
                EntryAction::Command(
                    "Focus window".to_owned(),
                    format!("hyprctl dispatch focuswindow address:{}", value.address),
                    None,
                ),
                EntryAction::Command(
                    "Close window".to_owned(),
                    format!("hyprctl dispatch closewindow address:{}", value.address),
                    None,
                ),
            ],
            id: value.address.to_string(),
        }
    }
}

impl Hyprland {
    pub fn new() -> Self {
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
            .flatten()
            .collect();

        let mut plugin = Self {
            entries,
            clients: Vec::new(),
        };
        plugin.open();

        plugin
    }
}

impl Plugin for Hyprland {
    fn name(&self) -> &str {
        "Windows"
    }

    fn has_entry(&self) -> bool {
        true
    }

    fn open(&mut self) {
        let Ok(current_workspace) = Workspace::get_active() else {
            return;
        };
        let current_workspace = current_workspace.id;
        let current_window = hyprland::data::Client::get_active()
            .unwrap()
            .map(|x| x.address)
            .unwrap_or(Address::new(""));
        let clients = Clients::get().unwrap();
        let clients = clients
            .into_iter()
            .map(|x| {
                let data = self.entries.get(&x.class.to_lowercase());

                let name = data
                    .map(|(x, _)| x)
                    .and_then(Option::as_deref)
                    .map(str::to_owned);

                let icon = data
                    .map(|(_, x)| x)
                    .and_then(Option::as_deref)
                    .map(|x| x.to_string());

                HyprlandClient {
                    selection_status: if current_window == x.address {
                        SelectionStatus::Selected
                    } else if current_workspace == x.workspace.id {
                        SelectionStatus::SameWorkspace
                    } else {
                        SelectionStatus::None
                    },
                    title: x.title,
                    class: x.class,
                    app_name: name,
                    address: x.address,
                    workspace: x.workspace.id,
                    position: x.at,
                    path: icon,
                }
            })
            .collect_vec();

        self.clients = clients;
    }

    fn icon(&self) -> Option<&str> {
        Some("window")
    }

    fn search(&self, query: &str, _: &mut Context) -> Vec<Entry> {
        if query.is_empty() {
            self.clients
                .iter()
                .sorted_by_cached_key(|x| (x.selection_status, x.workspace, x.position))
                .map(Entry::from)
                .collect()
        } else {
            let matcher = fuzzy_matcher::skim::SkimMatcherV2::default().smart_case();
            // let mut matcher = Matcher::new(Config::DEFAULT.match_paths());
            // let pattern = Pattern::new(
            //     query,
            //     CaseMatching::Ignore,
            //     Normalization::Smart,
            //     AtomKind::Fuzzy,
            // );
            self.clients
                .iter()
                .filter_map(|client| {
                    let mut score = 0;

                    score += 4 * matcher.fuzzy_match(&client.title, query).unwrap_or(0);

                    score += matcher.fuzzy_match(&client.class, query).unwrap_or(0);

                    if score == 0 {
                        None
                    } else {
                        Some((score, client))
                    }
                })
                .sorted_by_cached_key(|(x, _)| *x)
                .rev()
                .map(|(_, x)| Entry::from(x))
                .collect()
        }
    }
}
