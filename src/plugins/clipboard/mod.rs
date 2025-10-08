use crate::interface::Plugin;

#[derive(Debug)]
pub struct Clipboard {}

impl Clipboard {
    pub fn new() -> Self {
        Self {}
    }
}

impl Plugin for Clipboard {
    fn name(&self) -> &str {
        "Clipboard"
    }

    fn icon(&self) -> Option<&str> {
        Some("clipboard")
    }

    fn has_entry(&self) -> bool {
        true
    }
}
