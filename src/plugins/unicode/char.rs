#[derive(Clone, Debug)]
pub struct Char<'a> {
    pub scalar: char,
    pub codepoint: u32,
    pub name: &'a str,
}
