use std::path::Path;

use crate::interface::{Context, Entry, EntryAction, EntryIcon, Plugin};

#[derive(Debug)]
struct SshConnection {
    address: String,
    name: String,
    user: Option<String>,
    port: Option<u16>,
    ssh_key: Option<String>,
    command: Option<String>,
}

fn inner<P: AsRef<Path>>(path: P) -> std::io::Result<Vec<SshConnection>> {
    let mut connections = Vec::new();
    let mut current = None;

    for line in std::fs::read_to_string(path)?.lines().map(str::trim) {
        if line.starts_with("#") || line.is_empty() {
            continue;
        }

        if let Some(line) = line.strip_prefix("Host ").filter(|line| *line != "*") {
            if let Some(current) = current {
                connections.push(current);
            }
            current = Some(SshConnection {
                address: "".to_owned(),
                name: line.to_owned(),
                user: None,
                port: None,
                ssh_key: None,
                command: None,
            });
        } else if let Some(current) = current.as_mut() {
            let (key, value) = line.split_once(' ').unwrap();

            match key {
                "HostName" => {
                    current.address = value.to_owned();
                }
                "User" => {
                    current.user = Some(value.to_owned());
                }
                "Port" => {
                    if let Ok(value) = value.parse() {
                        current.port = Some(value);
                    }
                }
                "IdentityFile" => {
                    current.ssh_key = Some(value.to_owned());
                }
                "HostNameKey" => {
                    // Ignore this key
                }
                "RemoteCommand" => {
                    current.command = Some(value.to_owned());
                }
                _ => {}
            }
        }
    }

    if let Some(current) = current {
        connections.push(current);
    }

    Ok(connections)
}

#[derive(Debug)]
pub struct Ssh {
    connections: Vec<SshConnection>,
}

impl Ssh {
    pub fn new() -> Self {
        #[allow(deprecated)]
        let home = std::env::home_dir().unwrap();
        let connections = inner(home.join(".ssh").join("config")).unwrap_or_default();

        Self { connections }
    }
}

impl Plugin for Ssh {
    fn name(&self) -> &str {
        "Ssh connections"
    }

    fn icon(&self) -> Option<&str> {
        Some("network-wired")
    }

    fn search(&self, query: &str, _: &mut Context) -> Vec<Entry> {
        let query = query.to_owned();
        self.connections
            .iter()
            .filter(move |x| x.name.contains(&query) || x.address.contains(&query))
            .map(|x| Entry {
                name: x.name.clone(),
                tag: None,
                description: Some(format!(
                    "{}{}{}",
                    x.user.clone().map(|x| x + "@").unwrap_or_default(),
                    x.address,
                    x.port.map(|x| format!(":{x}")).unwrap_or_default(),
                )),
                icon: EntryIcon::Name("network-wired".to_owned()),
                small_icon: EntryIcon::None,
                actions: vec![
                    EntryAction::LaunchTerminal("ssh".to_owned(), vec![x.name.clone()]).into(),
                ],
                id: "".to_owned(),
            })
            .collect()
    }

    fn has_entry(&self) -> bool {
        true
    }
}
