use super::{Command, Panel, SNAP_ROWS, State};
use crate::i18n;
use deadlib_platform::input::KeyCode;
use deadsync_editor::{notes::Kind, transform::Transform};
const SOURCES: [Kind; 8] = [
    Kind::Tap,
    Kind::Hold,
    Kind::Roll,
    Kind::Mine,
    Kind::Lift,
    Kind::Fake,
    Kind::Keysound,
    Kind::Attack,
];
const TARGETS: [Kind; 6] = [
    Kind::Tap,
    Kind::Hold,
    Kind::Roll,
    Kind::Mine,
    Kind::Lift,
    Kind::Fake,
];
pub(super) struct Settings {
    from: usize,
    to: usize,
    pub duration: usize,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            from: 0,
            to: 1,
            duration: 0,
        }
    }
}
impl State {
    pub(super) fn note_conversion_key(&mut self, index: usize, code: KeyCode) {
        match code {
            KeyCode::Escape => self.panel = Some(Panel::Transforms(9)),
            KeyCode::ArrowUp => self.panel = Some(Panel::NoteConversion((index + 4) % 5)),
            KeyCode::ArrowDown | KeyCode::Tab => {
                self.panel = Some(Panel::NoteConversion((index + 1) % 5))
            }
            KeyCode::ArrowLeft | KeyCode::ArrowRight | KeyCode::Enter if index < 3 => {
                let forward = code != KeyCode::ArrowLeft;
                let (value, count) = match index {
                    0 => (&mut self.note_conversion.from, SOURCES.len()),
                    1 => (&mut self.note_conversion.to, TARGETS.len()),
                    _ => (&mut self.note_conversion.duration, SNAP_ROWS.len()),
                };
                *value = (*value + if forward { 1 } else { count - 1 }) % count;
            }
            KeyCode::Enter if index == 3 => {
                let settings = &self.note_conversion;
                let transform = Transform::Convert {
                    from: SOURCES[settings.from],
                    to: TARGETS[settings.to],
                    sustain_rows: SNAP_ROWS[settings.duration],
                };
                self.panel = None;
                self.command(Command::Transform(transform));
            }
            KeyCode::Enter => self.panel = Some(Panel::Transforms(9)),
            _ => {}
        }
    }
    pub(super) fn note_conversion_label(&self, index: usize, key: &str) -> std::sync::Arc<str> {
        let settings = &self.note_conversion;
        let value = match index {
            0 => i18n::tr("Edition", &format!("{:?}", SOURCES[settings.from])).to_string(),
            1 => i18n::tr("Edition", &format!("{:?}", TARGETS[settings.to])).to_string(),
            2 => SNAP_ROWS[settings.duration].to_string(),
            _ => return i18n::tr("Edition", key),
        };
        i18n::tr_fmt("Edition", key, &[("value", &value)])
    }
}
