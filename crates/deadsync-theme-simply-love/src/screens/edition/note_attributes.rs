use super::{Panel, State};
use crate::i18n;
use deadlib_platform::input::KeyCode;
impl State {
    pub(super) fn open_note_attributes(&mut self) {
        self.note_attribute_lane = (0..self.workspace.grid().columns())
            .find(|&lane| self.workspace.note_attributes(lane).is_ok())
            .unwrap_or(0);
        self.load_note_attributes();
        self.panel = Some(Panel::NoteAttributes(0));
        self.status = i18n::tr("Edition", "NoteAttributesHelp");
        self.focus_text();
    }
    fn load_note_attributes(&mut self) {
        self.note_attributes = self
            .workspace
            .note_attributes(self.note_attribute_lane)
            .ok();
    }
    pub(super) fn note_attributes_key(&mut self, index: usize, code: KeyCode) {
        if code == KeyCode::Escape || (index == 4 && code == KeyCode::Enter) {
            self.text_edit_serial = self.text_edit_serial.wrapping_add(1);
            self.text_buffer = None;
            self.note_attributes = None;
            self.label_preedit.clear();
            self.panel = Some(Panel::Transforms(11));
            return;
        }
        if matches!(
            code,
            KeyCode::ArrowUp
                | KeyCode::ArrowDown
                | KeyCode::Tab
                | KeyCode::PageUp
                | KeyCode::PageDown
        ) {
            if !self.sync_text() {
                return;
            }
            let back = code == KeyCode::ArrowUp || (code == KeyCode::Tab && self.shift);
            let next = match code {
                KeyCode::PageUp => 0,
                KeyCode::PageDown => 4,
                _ => (index + if back { 4 } else { 1 }) % 5,
            };
            self.panel = Some(Panel::NoteAttributes(next));
            self.focus_text();
            return;
        }
        if index == 0 && matches!(code, KeyCode::ArrowLeft | KeyCode::ArrowRight) {
            // Pending edits belong to a specific note; require applying or cancelling them first.
            if self
                .note_attributes
                .as_ref()
                .is_some_and(|d| self.workspace.grid().get(d.position()) != Some(d.note()))
            {
                self.status = i18n::tr("Edition", "NoteAttributesPending");
                return;
            }
            let count = self.workspace.grid().columns();
            self.note_attribute_lane = (self.note_attribute_lane
                + if code == KeyCode::ArrowLeft {
                    count - 1
                } else {
                    1
                })
                % count;
            self.load_note_attributes();
            return;
        }
        if index == 3 && code == KeyCode::Enter {
            if !self.sync_text() {
                return;
            }
            let Some(draft) = self.note_attributes.take() else {
                self.status = i18n::tr("Edition", "NoteAttributesMissing");
                return;
            };
            self.apply_dialog_operation(
                |workspace| workspace.apply_note_attributes(&draft).map(|_| ()),
                "NoteAttributesApplied",
                "NoteAttributesNoChange",
                "NoteAttributesFailed",
            );
            if self.panel.is_some() {
                self.note_attributes = Some(draft);
            } else {
                self.text_buffer = None;
                self.label_preedit.clear();
            }
            return;
        }
        if matches!(index, 1 | 2) {
            self.text_field_key(index, code);
        }
    }
    pub(super) fn note_attributes_label(&self, index: usize, key: &str) -> std::sync::Arc<str> {
        match index {
            0 => {
                let mut value = (self.note_attribute_lane + 1).to_string();
                if let Some(draft) = &self.note_attributes {
                    let position = draft.position();
                    let kind = i18n::tr("Edition", &format!("{:?}", draft.note().kind));
                    let rows = draft
                        .note()
                        .end
                        .map(|end| format!("{}-{end}", position.row))
                        .unwrap_or_else(|| position.row.to_string());
                    value = format!("{value} ({kind}, P{}, {rows})", position.player + 1);
                }
                i18n::tr_fmt("Edition", key, &[("value", &value)])
            }
            2 if self
                .note_attributes
                .as_ref()
                .is_some_and(|d| d.note().end.is_none()) =>
            {
                i18n::tr("Edition", "NoteAttributesNoTail")
            }
            1 | 2 if self.note_attributes.is_some() => self.text_field_label(index),
            1 | 2 => i18n::tr("Edition", "NoteAttributesMissing"),
            _ => i18n::tr("Edition", key),
        }
    }
}
