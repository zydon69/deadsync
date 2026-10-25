use super::State;
use crate::i18n;
use deadlib_platform::input::KeyCode;
use deadsync_editor::{chart_text::Field, clipboard::TextClipboard};
pub(super) struct Request {
    serial: u64,
    field: Field,
    operation: Operation,
}
enum Operation {
    Copy(String),
    Cut(String),
    Paste,
}
impl State {
    pub(super) fn queue_text_clipboard(&mut self, code: KeyCode) {
        let Some(field) = self.chart_text_field() else {
            return;
        };
        let Some(buffer) = self.chart_text_buffer.as_ref() else {
            return;
        };
        let selected = buffer.selection();
        // A NUL can terminate native clipboard text on some platforms. Never
        // cut a selection that cannot be represented faithfully there.
        if matches!(code, KeyCode::KeyC | KeyCode::KeyX)
            && buffer.value()[selected.clone()].contains('\0')
        {
            self.status = i18n::tr("Edition", "ClipboardInvalidSelection");
            return;
        }
        let operation = match code {
            KeyCode::KeyC if !selected.is_empty() => {
                Operation::Copy(buffer.value()[selected].to_owned())
            }
            KeyCode::KeyX if !selected.is_empty() => {
                Operation::Cut(buffer.value()[selected].to_owned())
            }
            KeyCode::KeyV => Operation::Paste,
            _ => return,
        };
        self.text_clipboard_request = Some(Request {
            serial: self.text_edit_serial,
            field,
            operation,
        });
    }
    /// Only called by the shell after raw key dispatch. No OS access occurs on
    /// focus changes, redraws or ordinary typing. Stale requests never touch IO.
    pub fn process_text_clipboard(&mut self, clipboard: &mut impl TextClipboard) -> bool {
        let Some(request) = self.text_clipboard_request.take() else {
            return false;
        };
        if request.serial != self.text_edit_serial
            || self.chart_text_field() != Some(request.field)
            || !self.label_preedit.is_empty()
        {
            return false;
        }
        let result = match request.operation {
            Operation::Copy(text) => clipboard
                .write_text(&text)
                .map(|()| Some("ClipboardCopied")),
            Operation::Cut(text) => clipboard.write_text(&text).map(|()| {
                if let Some(buffer) = self.chart_text_buffer.as_mut() {
                    buffer.delete(false);
                }
                self.text_edit_serial = self.text_edit_serial.wrapping_add(1);
                self.sync_chart_text().then_some("ClipboardCut")
            }),
            Operation::Paste => clipboard.read_text().map(|text| {
                if text.is_empty() || text.chars().all(char::is_control) {
                    return Some("ClipboardNoChange");
                }
                self.chart_text_input(&text);
                self.sync_chart_text().then_some("ClipboardPasted")
            }),
        };
        match result {
            Ok(Some(key)) => self.status = i18n::tr("Edition", key),
            Ok(None) => {}
            Err(error) => {
                self.status = i18n::tr(
                    "Edition",
                    match error {
                        deadsync_editor::clipboard::Error::Unavailable => "ClipboardUnavailable",
                        deadsync_editor::clipboard::Error::NoText => "ClipboardNoText",
                    },
                )
            }
        }
        true
    }
}
