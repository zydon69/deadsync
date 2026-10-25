//! Text-only OS boundary; editor tests can supply a deterministic transport.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Unavailable,
    NoText,
}
pub trait TextClipboard {
    fn read_text(&mut self) -> Result<String, Error>;
    fn write_text(&mut self, text: &str) -> Result<(), Error>;
}
