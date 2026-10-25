use super::{Panel, State};
use crate::i18n;
use deadlib_platform::input::KeyCode;
use deadsync_editor::display_tempo::Mode;

impl State {
    pub(super) fn open_display_tempo(&mut self) {
        if !self.stage_chart_rating() {
            return;
        }
        let Some(draft) = &self.chart_properties else {
            return;
        };
        self.display_tempo_draft = Some(draft.display_tempo().clone());
        self.display_tempo_input = None;
        self.panel = Some(Panel::DisplayTempo(0));
        self.status = i18n::tr(
            "Edition",
            if draft.format() == deadsync_editor::document::Format::Sm {
                "DisplayTempoHelpSm"
            } else {
                "DisplayTempoHelpSsc"
            },
        );
    }
    fn stage_display_tempo_input(&mut self) -> bool {
        let Some((index, text)) = &self.display_tempo_input else {
            return true;
        };
        let result = text.trim().parse::<f64>().ok().filter(|v| v.is_finite());
        let Some(value) = result else {
            self.status = i18n::tr("Edition", "DisplayTempoInvalid");
            return false;
        };
        let Some(draft) = self.display_tempo_draft.as_mut() else {
            return false;
        };
        let result = if *index == 1 {
            draft.set_min(value)
        } else {
            draft.set_max(value)
        };
        if result.is_err() {
            self.status = i18n::tr("Edition", "DisplayTempoInvalid");
            return false;
        }
        self.display_tempo_input = None;
        true
    }
    pub(super) fn display_tempo_key(&mut self, index: usize, code: KeyCode) {
        if code == KeyCode::Escape || (index == 5 && code == KeyCode::Enter) {
            self.display_tempo_draft = None;
            self.display_tempo_input = None;
            self.panel = Some(Panel::ChartProperties(10));
            self.status = i18n::tr("Edition", "DisplayTempoDiscarded");
            return;
        }
        let Some(draft) = self.display_tempo_draft.as_ref() else {
            return;
        };
        if !self.control
            && matches!(index, 1 | 3)
            && draft.mode() == Mode::Specified
            && (index == 1 || draft.separate_max())
        {
            let current = if index == 1 { draft.min() } else { draft.max() };
            let character = match code {
                KeyCode::Digit0 | KeyCode::Numpad0 => Some('0'),
                KeyCode::Digit1 | KeyCode::Numpad1 => Some('1'),
                KeyCode::Digit2 | KeyCode::Numpad2 => Some('2'),
                KeyCode::Digit3 | KeyCode::Numpad3 => Some('3'),
                KeyCode::Digit4 | KeyCode::Numpad4 => Some('4'),
                KeyCode::Digit5 | KeyCode::Numpad5 => Some('5'),
                KeyCode::Digit6 | KeyCode::Numpad6 => Some('6'),
                KeyCode::Digit7 | KeyCode::Numpad7 => Some('7'),
                KeyCode::Digit8 | KeyCode::Numpad8 => Some('8'),
                KeyCode::Digit9 | KeyCode::Numpad9 => Some('9'),
                KeyCode::Minus | KeyCode::NumpadSubtract => Some('-'),
                KeyCode::Period | KeyCode::NumpadDecimal => Some('.'),
                KeyCode::KeyE => Some('e'),
                KeyCode::NumpadAdd => Some('+'),
                KeyCode::Equal if self.shift => Some('+'),
                _ => None,
            };
            if let Some(character) = character {
                let (_, text) = self
                    .display_tempo_input
                    .get_or_insert_with(|| (index, String::new()));
                if text.len() < 64 {
                    text.push(character);
                }
                return;
            }
            if code == KeyCode::Backspace {
                self.display_tempo_input
                    .get_or_insert_with(|| (index, current.to_string()))
                    .1
                    .pop();
                return;
            }
            if code == KeyCode::Delete {
                self.display_tempo_input = Some((index, String::new()));
                return;
            }
        }
        if !self.stage_display_tempo_input() {
            return;
        }
        if matches!(code, KeyCode::ArrowUp | KeyCode::ArrowDown) {
            self.panel = Some(Panel::DisplayTempo(
                (index + if code == KeyCode::ArrowUp { 5 } else { 1 }) % 6,
            ));
            return;
        }
        if index == 4 && code == KeyCode::Enter {
            if let (Some(parent), Some(value)) = (
                self.chart_properties.as_mut(),
                self.display_tempo_draft.take(),
            ) {
                parent.set_display_tempo(value);
            }
            self.panel = Some(Panel::ChartProperties(4));
            self.status = i18n::tr("Edition", "DisplayTempoStaged");
            return;
        }
        if !matches!(
            code,
            KeyCode::ArrowLeft | KeyCode::ArrowRight | KeyCode::Enter
        ) {
            return;
        }
        if index == 0 {
            let mode = self.display_tempo_draft.as_ref().unwrap().mode();
            let modes = [Mode::Actual, Mode::Random, Mode::Specified];
            let current = modes.iter().position(|m| *m == mode).unwrap();
            let next = modes[(current + if code == KeyCode::ArrowLeft { 2 } else { 1 }) % 3];
            let actual = if next == Mode::Specified {
                let Some(parent) = self.chart_properties.as_ref() else {
                    return;
                };
                let session = &self.workspace.file().session;
                // The draft carries its own chart/revision guard; the workspace owns selection.
                match parent.actual_display_tempo_range(session, self.workspace.chart()) {
                    Ok(range) => range,
                    Err(error) => {
                        self.status =
                            i18n::tr_fmt("Edition", "Error", &[("message", &error.to_string())]);
                        return;
                    }
                }
            } else {
                (0.0, 0.0)
            };
            let _ = self
                .display_tempo_draft
                .as_mut()
                .unwrap()
                .set_mode(next, actual);
        } else {
            let draft = self.display_tempo_draft.as_mut().unwrap();
            if draft.mode() != Mode::Specified {
                return;
            }
            if index == 2 {
                draft.set_separate_max(!draft.separate_max());
            }
            if matches!(index, 1 | 3) && code != KeyCode::Enter {
                let delta = if self.shift { 10.0 } else { 1.0 }
                    * if code == KeyCode::ArrowLeft {
                        -1.0
                    } else {
                        1.0
                    };
                let _ = if index == 1 {
                    draft.set_min(draft.min() + delta)
                } else {
                    draft.set_max(draft.max() + delta)
                };
            }
        }
    }
    pub(super) fn display_tempo_label(&self, index: usize, key: &str) -> std::sync::Arc<str> {
        let Some(draft) = &self.display_tempo_draft else {
            return i18n::tr("Edition", key);
        };
        let value = match index {
            0 => i18n::tr(
                "Edition",
                match draft.mode() {
                    Mode::Actual => "DisplayTempoActual",
                    Mode::Random => "DisplayTempoRandom",
                    Mode::Specified => "DisplayTempoSpecified",
                },
            )
            .to_string(),
            1 | 3 => self
                .display_tempo_input
                .as_ref()
                .filter(|(i, _)| *i == index)
                .map(|(_, s)| s.clone())
                .unwrap_or_else(|| {
                    format!("{:.6}", if index == 1 { draft.min() } else { draft.max() })
                }),
            2 => i18n::tr(
                "Edition",
                if draft.separate_max() {
                    "DisplayTempoOn"
                } else {
                    "DisplayTempoOff"
                },
            )
            .to_string(),
            _ => return i18n::tr("Edition", key),
        };
        i18n::tr_fmt("Edition", key, &[("value", &value)])
    }
}
