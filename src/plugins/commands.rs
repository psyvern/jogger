use crate::interface::EntryIcon;
use crate::Plugin;

use crate::{Entry, EntryAction};
use std::collections::HashMap;
use std::env;

#[derive(Debug)]
pub struct Commands {
    shell: Option<String>,
}

impl Commands {
    pub async fn new() -> Self {
        Self {
            shell: env::var("SHELL").ok(),
        }
    }
}

impl Plugin for Commands {
    fn icon(&self) -> Option<&str> {
        Some("terminal")
    }

    fn prefix(&self) -> Option<&str> {
        Some(">")
    }

    fn search(&self, query: &str) -> Box<dyn Iterator<Item = crate::interface::Entry>> {
        Box::new(std::iter::once(Entry {
            name: format!("<tt>{}</tt>", gtk::glib::markup_escape_text(query.trim())),
            description: self.shell.as_ref().map(|x| format!("<tt>{x}</tt>")),
            icon: EntryIcon::Name("terminal".to_owned()),
            small_icon: EntryIcon::None,
            action: EntryAction::Shell(query.trim().into(), /* self.shell.clone(), */ None),
            sub_entries: HashMap::new(),
            id: "".to_owned(),
        }))
    }
}
