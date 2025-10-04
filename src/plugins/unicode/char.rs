#[derive(Clone, Debug)]
pub struct Char<'a> {
    pub scalar: char,
    pub codepoint: &'a str,
    pub name: &'a str,
}
