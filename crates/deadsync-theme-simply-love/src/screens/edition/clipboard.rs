use super::text_fields::Target;
use super::{Panel, State};
use crate::i18n;
use deadlib_platform::input::KeyCode;
use deadsync_editor::clipboard::TextClipboard;
pub(super) struct Request {
    serial: u64,
    field: ClipboardTarget,
    operation: Operation,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum ClipboardTarget {
    Property(Target),
    ProfileText(super::profile_edit::TextField),
    KeyProfileImport,
}
enum Operation {
    Copy(String),
    Cut(String),
    Paste,
    ImportKeyProfile(std::sync::Arc<deadsync_editor::key_binding_preferences::Preferences>),
}
impl State {
    fn clipboard_target(&self) -> Option<ClipboardTarget> {
        if matches!(self.panel, Some(Panel::KeyBindingSettings(_)))
            && self.key_binding_draft.is_some()
        {
            return Some(ClipboardTarget::KeyProfileImport);
        }
        if matches!(self.panel, Some(Panel::ProfileText)) {
            self.profile_edit.as_ref()?.text_buffer.as_ref()?;
            Some(ClipboardTarget::ProfileText(
                self.profile_edit.as_ref()?.text_field,
            ))
        } else {
            self.text_target().map(ClipboardTarget::Property)
        }
    }
    fn clipboard_buffer(&self) -> Option<&deadsync_editor::text_buffer::Buffer> {
        if matches!(self.panel, Some(Panel::ProfileText)) {
            self.profile_edit.as_ref()?.text_buffer.as_ref()
        } else {
            self.text_buffer.as_ref()
        }
    }
    fn clipboard_buffer_mut(&mut self) -> Option<&mut deadsync_editor::text_buffer::Buffer> {
        if matches!(self.panel, Some(Panel::ProfileText)) {
            self.profile_edit.as_mut()?.text_buffer.as_mut()
        } else {
            self.text_buffer.as_mut()
        }
    }
    fn clipboard_sync(&mut self) -> bool {
        matches!(self.panel, Some(Panel::ProfileText)) || self.sync_text()
    }

    pub(super) fn queue_text_clipboard(&mut self, code: KeyCode) {
        let Some(field) = self.clipboard_target() else {
            return;
        };
        let Some(buffer) = self.clipboard_buffer() else {
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
    pub(super) fn queue_key_profile_import(&mut self) {
        let Some(settings) = self.key_binding_draft.as_ref() else {
            return;
        };
        let expected = settings.draft.staged();
        if !self.key_profile_import_matches(&expected) {
            return;
        }
        self.text_clipboard_request = Some(Request {
            serial: self.text_edit_serial,
            field: ClipboardTarget::KeyProfileImport,
            operation: Operation::ImportKeyProfile(expected),
        });
        self.status = i18n::tr("Edition", "KeyProfileClipboardRequested");
    }
    /// Only called by the shell after raw key dispatch. No OS access occurs on
    /// focus changes, redraws or ordinary typing. Stale requests never touch IO.
    pub fn process_text_clipboard(&mut self, clipboard: &mut impl TextClipboard) -> bool {
        let Some(request) = self.text_clipboard_request.take() else {
            return false;
        };
        if request.serial != self.text_edit_serial
            || self.clipboard_target() != Some(request.field)
            || !self.label_preedit.is_empty()
        {
            return false;
        }
        if let Operation::ImportKeyProfile(expected) = &request.operation {
            // No OS read after close/reopen, draft replacement or capture entry.
            if !self.key_profile_import_matches(expected) {
                return false;
            }
        }
        let result = match request.operation {
            Operation::ImportKeyProfile(_) => clipboard.read_text().map(|text| {
                self.key_profile_import_text(&text);
                None
            }),
            Operation::Copy(text) => clipboard
                .write_text(&text)
                .map(|()| Some("ClipboardCopied")),
            Operation::Cut(text) => clipboard.write_text(&text).map(|()| {
                if let Some(buffer) = self.clipboard_buffer_mut() {
                    buffer.delete(false);
                }
                self.text_edit_serial = self.text_edit_serial.wrapping_add(1);
                self.clipboard_sync().then_some("ClipboardCut")
            }),
            Operation::Paste => clipboard.read_text().map(|text| {
                if text.is_empty() || text.chars().all(char::is_control) {
                    return Some("ClipboardNoChange");
                }
                if matches!(request.field, ClipboardTarget::ProfileText(_)) {
                    self.profile_text_input(&text);
                } else {
                    self.edit_text_input(&text);
                }
                self.clipboard_sync().then_some("ClipboardPasted")
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
