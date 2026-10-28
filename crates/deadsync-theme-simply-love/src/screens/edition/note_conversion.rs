use super::{Panel, SNAP_ROWS, State};
use crate::i18n;
use deadlib_platform::input::KeyCode;
use deadsync_editor::{notes::Kind, transform::Transform, note_swap::Pair};
const PAIRS: [Pair; 10] = [Pair::TapsMines, Pair::TapsFakes, Pair::TapsLifts, Pair::MinesFakes, Pair::MinesLifts, Pair::HoldsRolls, Pair::HoldsTaps, Pair::HoldsMines, Pair::RollsTaps, Pair::RollsMines];
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
    swap: usize,
    to: usize,
    pub duration: usize,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            from: 0,
            swap: 0,
            to: 1,
            duration: 0,
        }
    }
}
impl State {
    pub(super) fn note_conversion_key(&mut self, index: usize, code: KeyCode) {
        match code {
            KeyCode::Escape => self.panel = Some(Panel::Transforms(9)),
            KeyCode::ArrowUp => self.panel = Some(Panel::NoteConversion((index + 6) % 7)),
            KeyCode::ArrowDown | KeyCode::Tab => {
                self.panel = Some(Panel::NoteConversion((index + 1) % 7))
            }
            KeyCode::ArrowLeft | KeyCode::ArrowRight | KeyCode::Enter if index < 3 => {
                let forward = code != KeyCode::ArrowLeft;
                let (value, count) = match index {
                    0 => (&mut self.note_conversion.from, SOURCES.len()),
                    1 => (&mut self.note_conversion.to, TARGETS.len()),
                    _ => (&mut self.note_conversion.duration, SNAP_ROWS.len()-1),
                };
                *value = (*value + if forward { 1 } else { count - 1 }) % count;
            }
            KeyCode::ArrowLeft | KeyCode::ArrowRight | KeyCode::Enter if index == 5 => {
                let count = PAIRS.len();
                self.note_conversion.swap = (self.note_conversion.swap + if code == KeyCode::ArrowLeft { count - 1 } else { 1 }) % count;
            }
            KeyCode::Enter if index == 6 => {
                let pair = PAIRS[self.note_conversion.swap];
                self.apply_dialog_operation(
                    |workspace| workspace.swap_note_types(pair).map(|_| ()),
                    "NoteSwapApplied", "NoteSwapNoChange", "NoteSwapFailed",
                );
            }
            KeyCode::Enter if index == 3 => {
                let settings = &self.note_conversion;
                let transform = Transform::Convert {
                    from: SOURCES[settings.from],
                    to: TARGETS[settings.to],
                    sustain_rows: SNAP_ROWS[settings.duration],
                };
                self.apply_dialog_transform(
                    transform,
                    "NoteConversionApplied",
                    "NoteConversionNoChange",
                    "NoteConversionFailed",
                );
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
            5 => {
                let (a,b) = PAIRS[settings.swap].kinds();
                let a = i18n::tr("Edition", &format!("{a:?}"));
                let b = i18n::tr("Edition", &format!("{b:?}"));
                format!("{a} / {b}")
            }
            _ => return i18n::tr("Edition", key),
        };
        i18n::tr_fmt("Edition", key, &[("value", &value)])
    }
}
