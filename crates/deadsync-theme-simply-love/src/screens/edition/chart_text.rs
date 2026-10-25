use super::{Panel, State};
use crate::i18n;
use deadlib_platform::input::KeyCode;
use deadsync_editor::{
    chart_text::Field,
    text_buffer::{Buffer, Movement},
};
impl State {
    pub(super) fn open_chart_text(&mut self) {
        let first = if self
            .chart_properties
            .as_ref()
            .is_some_and(|d| d.supports_text(Field::Name))
        {
            0
        } else {
            1
        };
        self.label_preedit.clear();
        self.panel = Some(Panel::ChartText(first));
        self.focus_chart_text();
        self.status = i18n::tr(
            "Edition",
            if first == 0 {
                "ChartTextHelpSsc"
            } else {
                "ChartTextHelpSm"
            },
        );
    }
    pub(super) fn chart_text_field(&self) -> Option<Field> {
        let Some(Panel::ChartText(index)) = self.panel else {
            return None;
        };
        let field = Field::ALL.get(index).copied()?;
        self.chart_properties
            .as_ref()?
            .supports_text(field)
            .then_some(field)
    }
    fn focus_chart_text(&mut self) {
        self.text_edit_serial = self.text_edit_serial.wrapping_add(1);
        self.chart_text_buffer = self.chart_text_field().and_then(|field| {
            self.chart_properties
                .as_ref()
                .map(|draft| Buffer::new(draft.text(field), true))
        });
        self.label_preedit.clear();
    }
    pub(super) fn sync_chart_text(&mut self) -> bool {
        let Some(field) = self.chart_text_field() else {
            return true;
        };
        if let (Some(draft), Some(buffer)) = (&mut self.chart_properties, &self.chart_text_buffer) {
            if buffer.value() != draft.text(field) && draft.set_text(field, buffer.value()).is_err()
            {
                self.status = i18n::tr("Edition", "ChartTextSingleLine");
                return false;
            }
        }
        true
    }
    pub(super) fn chart_text_input(&mut self, text: &str) -> bool {
        self.text_edit_serial = self.text_edit_serial.wrapping_add(1);
        let Some(buffer) = self.chart_text_buffer.as_mut() else {
            return false;
        };
        buffer.insert(text);
        self.sync_chart_text();
        self.label_preedit.clear();
        true
    }
    pub(super) fn chart_text_key(&mut self, index: usize, code: KeyCode) {
        self.text_edit_serial = self.text_edit_serial.wrapping_add(1);
        if self.control && matches!(code, KeyCode::KeyC | KeyCode::KeyX | KeyCode::KeyV) {
            self.queue_text_clipboard(code);
            return;
        }
        if matches!(code, KeyCode::Escape) || (index == 4 && code == KeyCode::Enter) {
            let accepted = self.sync_chart_text();
            self.label_preedit.clear();
            self.chart_text_buffer = None;
            self.panel = Some(Panel::ChartProperties(9));
            self.status = i18n::tr(
                "Edition",
                if accepted {
                    "ChartTextStaged"
                } else {
                    "ChartTextInvalidDiscarded"
                },
            );
            return;
        }
        if matches!(code, KeyCode::ArrowUp | KeyCode::ArrowDown | KeyCode::Tab) {
            if !self.sync_chart_text() {
                return;
            }
            let backward = code == KeyCode::ArrowUp || (code == KeyCode::Tab && self.shift);
            let delta = if backward { 4 } else { 1 };
            let mut next = (index + delta) % 5;
            while next < 4
                && self
                    .chart_properties
                    .as_ref()
                    .is_some_and(|d| !d.supports_text(Field::ALL[next]))
            {
                next = (next + delta) % 5;
            }
            self.panel = Some(Panel::ChartText(next));
            self.focus_chart_text();
            return;
        }
        let Some(buffer) = self.chart_text_buffer.as_mut() else {
            return;
        };
        match code {
            KeyCode::ArrowLeft => buffer.move_cursor(
                if self.control {
                    Movement::WordLeft
                } else {
                    Movement::Left
                },
                self.shift,
            ),
            KeyCode::ArrowRight => buffer.move_cursor(
                if self.control {
                    Movement::WordRight
                } else {
                    Movement::Right
                },
                self.shift,
            ),
            KeyCode::Home => buffer.move_cursor(Movement::Start, self.shift),
            KeyCode::End => buffer.move_cursor(Movement::End, self.shift),
            KeyCode::KeyA if self.control => buffer.select_all(),
            KeyCode::Backspace => {
                buffer.backspace(self.control);
            }
            KeyCode::Delete => {
                buffer.delete(self.control);
            }
            KeyCode::KeyZ if self.control => {
                if self.shift {
                    buffer.redo();
                } else {
                    buffer.undo();
                }
            }
            KeyCode::KeyY if self.control => {
                buffer.redo();
            }
            _ => return,
        }
        self.sync_chart_text();
    }
    pub(super) fn chart_text_label(&self, index: usize) -> std::sync::Arc<str> {
        let field = Field::ALL[index];
        let key = match field {
            Field::Name => "ChartName",
            Field::Description => "ChartDescription",
            Field::Credit => "ChartCredit",
            Field::Style => "ChartStyle",
        };
        let value = self
            .chart_properties
            .as_ref()
            .map(|d| {
                if !d.supports_text(field) {
                    return i18n::tr("Edition", "ChartTextUnsupportedSm").to_string();
                }
                if matches!(self.panel,Some(Panel::ChartText(selected)) if selected==index) {
                    self.chart_text_buffer
                        .as_ref()
                        .map(|buffer| buffer.display(&self.label_preedit, 48))
                        .unwrap_or_else(|| d.text(field).to_owned())
                } else {
                    // Unfocused fields also need a bounded preview.
                    let buffer = Buffer::new(d.text(field), false);
                    let preview = buffer.display("", 48);
                    preview.strip_suffix('|').unwrap_or(&preview).to_owned()
                }
            })
            .unwrap_or_default();
        i18n::tr_fmt("Edition", key, &[("value", &value)])
    }
}
