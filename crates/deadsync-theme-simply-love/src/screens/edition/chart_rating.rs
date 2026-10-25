use super::State;
use crate::i18n;
use deadlib_platform::input::KeyCode;
impl State {
    pub(super) fn rating_input(&mut self, code: KeyCode) -> bool {
        if self.control {
            return false;
        }
        let Some(draft) = self.chart_properties.as_ref() else {
            return false;
        };
        match code {
            KeyCode::Backspace => {
                let value = self
                    .chart_rating_input
                    .get_or_insert_with(|| draft.rating().to_string());
                value.pop();
            }
            KeyCode::Delete => self.chart_rating_input = Some(String::new()),
            _ => {
                let c = match code {
                    KeyCode::Digit0 | KeyCode::Numpad0 => '0',
                    KeyCode::Digit1 | KeyCode::Numpad1 => '1',
                    KeyCode::Digit2 | KeyCode::Numpad2 => '2',
                    KeyCode::Digit3 | KeyCode::Numpad3 => '3',
                    KeyCode::Digit4 | KeyCode::Numpad4 => '4',
                    KeyCode::Digit5 | KeyCode::Numpad5 => '5',
                    KeyCode::Digit6 | KeyCode::Numpad6 => '6',
                    KeyCode::Digit7 | KeyCode::Numpad7 => '7',
                    KeyCode::Digit8 | KeyCode::Numpad8 => '8',
                    KeyCode::Digit9 | KeyCode::Numpad9 => '9',
                    KeyCode::Minus | KeyCode::NumpadSubtract => '-',
                    KeyCode::Period | KeyCode::NumpadDecimal => '.',
                    KeyCode::Equal if self.shift => '+',
                    KeyCode::NumpadAdd => '+',
                    _ => return false,
                };
                let value = self.chart_rating_input.get_or_insert_with(String::new);
                if value.len() < 32 {
                    value.push(c);
                }
            }
        }
        true
    }
    pub(super) fn stage_chart_rating(&mut self) -> bool {
        let Some(value) = self.chart_rating_input.as_ref() else {
            return true;
        };
        let result = value.trim().parse::<i32>().ok().filter(|value| *value >= 1);
        let Some(value) = result else {
            self.status = i18n::tr("Edition", "ChartRatingInvalid");
            return false;
        };
        if let Some(draft) = self.chart_properties.as_mut() {
            if draft.set_rating(value).is_err() {
                self.status = i18n::tr("Edition", "ChartRatingInvalid");
                return false;
            }
        }
        self.chart_rating_input = None;
        true
    }
    pub(super) fn adjust_chart_rating(&mut self, forward: bool) {
        if !self.stage_chart_rating() {
            return;
        }
        if let Some(draft) = self.chart_properties.as_mut() {
            let delta = if self.shift { 100 } else { 1 } * if forward { 1 } else { -1 };
            let _ = draft.set_rating(draft.rating().saturating_add(delta).max(1));
        }
    }
    pub(super) fn chart_difficulty_label(&self) -> std::sync::Arc<str> {
        let value = self
            .chart_properties
            .as_ref()
            .and_then(|d| d.difficulty())
            .map(|d| i18n::tr("Edition", &format!("Difficulty{}", d.as_str())))
            .unwrap_or_else(|| i18n::tr("Edition", "DifficultyUnknown"));
        i18n::tr_fmt("Edition", "ChartDifficulty", &[("value", &value)])
    }
    pub(super) fn chart_rating_label(&self) -> std::sync::Arc<str> {
        let value = self.chart_rating_input.clone().unwrap_or_else(|| {
            self.chart_properties
                .as_ref()
                .map_or(String::new(), |d| d.rating().to_string())
        });
        i18n::tr_fmt("Edition", "ChartRating", &[("value", &value)])
    }
}
