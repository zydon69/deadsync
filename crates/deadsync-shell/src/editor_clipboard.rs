//! Main-thread clipboard ownership persists for the app lifetime (Linux selection owner).
use deadsync_editor::clipboard::{Error, TextClipboard};
#[derive(Default)]
pub(crate) struct NativeClipboard {
    clipboard: Option<arboard::Clipboard>,
}
impl NativeClipboard {
    fn open(&mut self) -> Result<&mut arboard::Clipboard, Error> {
        if self.clipboard.is_none() {
            self.clipboard = Some(arboard::Clipboard::new().map_err(|_| Error::Unavailable)?);
        }
        Ok(self.clipboard.as_mut().expect("clipboard initialized"))
    }
    pub(crate) fn close(&mut self) {
        self.clipboard = None;
    }
}
impl TextClipboard for NativeClipboard {
    fn read_text(&mut self) -> Result<String, Error> {
        self.open()?.get_text().map_err(|error| match error {
            arboard::Error::ContentNotAvailable => Error::NoText,
            _ => Error::Unavailable,
        })
    }
    fn write_text(&mut self, text: &str) -> Result<(), Error> {
        self.open()?.set_text(text).map_err(|_| Error::Unavailable)
    }
}

#[cfg(test)]
mod tests {
    use super::NativeClipboard;
    #[test]
    fn startup_and_repeated_shutdown_do_not_open_or_read_the_system_clipboard() {
        let mut clipboard = NativeClipboard::default();
        assert!(clipboard.clipboard.is_none());
        clipboard.close();
        clipboard.close();
        assert!(clipboard.clipboard.is_none());
    }
}
