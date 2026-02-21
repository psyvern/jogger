use crate::interface::{Context, Plugin};

#[derive(Debug)]
pub struct Clipboard {}

impl Clipboard {
    pub fn new(_: &Context) -> Self {
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
}
