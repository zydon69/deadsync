use super::{
    Panel, State,
    note_attributes_typed::{Inspector, Row},
};
use crate::i18n;
use deadlib_platform::input::KeyCode;
use deadsync_editor::note_attributes::Field;
impl State {
    pub(super) fn open_note_attributes(&mut self) {
        self.note_attribute_lane = (0..self.workspace.grid().columns())
            .find(|&lane| self.workspace.note_attributes(lane).is_ok())
            .unwrap_or(0);
        self.load_note_attributes();
        self.panel = Some(Panel::NoteAttributes(0));
        self.note_attribute_typed = self
            .note_attributes
            .as_ref()
            .is_some_and(Inspector::available);
        self.note_attributes_help();
        self.focus_text();
    }
    fn load_note_attributes(&mut self) {
        self.note_attribute_inspector = Inspector::default();
        self.note_attributes = self
            .workspace
            .note_attributes(self.note_attribute_lane)
            .ok();
    }
    fn note_attributes_help(&mut self) {
        self.status = i18n::tr(
            "Edition",
            if self.note_attribute_typed {
                "NoteAttributesTypedHelp"
            } else if self.workspace.grid().uses_stepf2() {
                "NoteAttributesRawHelp"
            } else {
                "NoteAttributesHelp"
            },
        );
    }
    fn toggle_note_attribute_view(&mut self, index: usize) {
        if !self.workspace.grid().uses_stepf2() || !self.sync_text() {
            return;
        }
        let Some(draft) = self.note_attributes.as_ref() else {
            self.status = i18n::tr("Edition", "NoteAttributesMissing");
            return;
        };
        let next = if self.note_attribute_typed {
            match self.note_attribute_inspector.endpoint() {
                Field::Head => 1,
                Field::Tail => 2,
            }
        } else {
            let endpoint = match index {
                1 => Field::Head,
                2 => Field::Tail,
                _ => self.note_attribute_inspector.endpoint(),
            };
            if self
                .note_attribute_inspector
                .select_endpoint(draft, endpoint)
                .is_err()
            {
                self.status = i18n::tr("Edition", "NoteAttributesTypedInvalid");
                return;
            }
            5
        };
        self.note_attribute_typed = !self.note_attribute_typed;
        self.panel = Some(Panel::NoteAttributes(next));
        self.focus_text();
        self.note_attributes_help();
    }
    pub(super) fn note_attributes_key(&mut self, index: usize, code: KeyCode) {
        let count = if self.note_attribute_typed {
            Row::ALL.len()
        } else {
            5
        };
        let enter = matches!(code, KeyCode::Enter | KeyCode::NumpadEnter);
        if code == KeyCode::F2 {
            self.toggle_note_attribute_view(index);
            return;
        }
        if code == KeyCode::Escape || (index == count - 1 && enter) {
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
                KeyCode::PageDown => count - 1,
                _ => (index + if back { count - 1 } else { 1 }) % count,
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
        if self.note_attribute_typed
            && matches!(index, 1..=3)
            && matches!(code, KeyCode::ArrowLeft | KeyCode::ArrowRight)
        {
            if !self.sync_text() {
                return;
            }
            let Some(draft) = self.note_attributes.as_mut() else {
                self.status = i18n::tr("Edition", "NoteAttributesMissing");
                return;
            };
            let result = match Row::ALL[index] {
                Row::Endpoint => self
                    .note_attribute_inspector
                    .toggle_endpoint(draft)
                    .map(|_| ()),
                Row::Visibility => self
                    .note_attribute_inspector
                    .cycle_visibility(draft, code == KeyCode::ArrowRight),
                Row::Fake => self.note_attribute_inspector.toggle_fake(draft),
                _ => unreachable!(),
            };
            if result.is_err() {
                self.status = i18n::tr("Edition", "NoteAttributesTypedInvalid");
            } else {
                self.focus_text();
                self.note_attributes_help();
            }
            return;
        }
        if index == count - 2 && enter {
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
        if self.text_target().is_some() {
            self.text_field_key(index, code);
        }
    }
    pub(super) fn note_attributes_label(&self, index: usize, key: &str) -> std::sync::Arc<str> {
        if self.note_attribute_typed && (1..=5).contains(&index) {
            let Some(draft) = self.note_attributes.as_ref() else {
                return i18n::tr("Edition", "NoteAttributesMissing");
            };
            if matches!(index, 4 | 5) {
                return self.text_field_label(index);
            }
            let value = match Row::ALL[index] {
                Row::Endpoint => i18n::tr(
                    "Edition",
                    match self.note_attribute_inspector.endpoint() {
                        Field::Head => "NoteAttributesEndpointHead",
                        Field::Tail => "NoteAttributesEndpointTail",
                    },
                ),
                Row::Visibility | Row::Fake => {
                    let Ok(attributes) = self.note_attribute_inspector.attributes(draft) else {
                        return i18n::tr("Edition", "NoteAttributesTypedInvalid");
                    };
                    i18n::tr(
                        "Edition",
                        if index == 3 {
                            if attributes.fake {
                                "NoteAttributesYes"
                            } else {
                                "NoteAttributesNo"
                            }
                        } else {
                            match attributes.visibility {
                                deadsync_editor::stepf2::Visibility::Normal => {
                                    "NoteAttributesNormal"
                                }
                                deadsync_editor::stepf2::Visibility::Sudden => {
                                    "NoteAttributesSudden"
                                }
                                deadsync_editor::stepf2::Visibility::Vanish => {
                                    "NoteAttributesVanish"
                                }
                                deadsync_editor::stepf2::Visibility::Hidden => {
                                    "NoteAttributesHidden"
                                }
                            }
                        },
                    )
                }
                _ => unreachable!(),
            };
            return i18n::tr_fmt("Edition", key, &[("value", &value)]);
        }
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
