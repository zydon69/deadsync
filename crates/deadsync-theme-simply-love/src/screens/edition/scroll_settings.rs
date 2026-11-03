use super::*;
use deadsync_editor::{scroll_preferences::Preferences};
impl State {
    pub fn scroll_preferences(&self) -> Preferences {
        self.scroll_preferences
    }
    pub(super) fn open_scroll_settings(&mut self) {
        self.scroll_preferences_draft = Some(self.scroll_preferences);
        self.panel = Some(Panel::ScrollSettings(0));
        self.status = i18n::tr("Edition", "ScrollSettingsHelp");
    }
    pub(super) fn scroll_settings_label(&self, key: &str) -> Arc<str> {
        let settings = self
            .scroll_preferences_draft
            .unwrap_or(self.scroll_preferences);
        if key == "ScrollWheelTimeValue" || key == "ScrollWheelRowValue" {
            let value=if key=="ScrollWheelTimeValue"{format!("{:.6}",settings.wheel_time())}else{settings.wheel_rows().to_string()};
            return i18n::tr_fmt("Edition",key,&[("value",&value)]);
        }
        let stop = if key == "ScrollStopDragging" {
            settings.stop_playback_when_dragging()
        } else {
            settings.stop_playback()
        };
        let value = i18n::tr("Edition", if stop { "WaveformYes" } else { "WaveformNo" });
        i18n::tr_fmt("Edition", key, &[("value", &value)])
    }
    pub(super) fn scroll_settings_key(&mut self, index: usize, code: KeyCode) {
        if code==KeyCode::F11{self.open_scroll_modifiers();return;}
        if code==KeyCode::F12{self.open_scroll_values();return;}
        let enter = matches!(code, KeyCode::Enter | KeyCode::NumpadEnter);
        if code == KeyCode::Escape || (index == 4 && enter) {
            self.scroll_preferences_draft = None;
            if let Some(file) = self.audio_preferences_file.as_mut() {
                file.set_scroll(self.scroll_preferences);
            }
            self.panel = None;
            self.status = i18n::tr("Edition", "ScrollSettingsDiscarded");
            return;
        }
        if matches!(code, KeyCode::ArrowUp | KeyCode::ArrowDown | KeyCode::Tab) {
            let back = code == KeyCode::ArrowUp || (code == KeyCode::Tab && self.shift);
            self.panel = Some(Panel::ScrollSettings(
                (index + if back { 6 } else { 1 }) % 7,
            ));
            return;
        }
        if index >= 5 && matches!(code,KeyCode::ArrowLeft|KeyCode::ArrowRight|KeyCode::Home) {
            if let Some(draft)=self.scroll_preferences_draft.as_mut(){
                let default=Preferences::default();let delta=if code==KeyCode::ArrowLeft{-1.0}else{1.0};
                if index==5{let value=if code==KeyCode::Home{default.wheel_time()}else{(draft.wheel_time()+delta*if self.shift{0.000001}else{0.001}).clamp(0.0,10.0)};let _=draft.set_wheel_time(value);}
                else{let value=if code==KeyCode::Home{default.wheel_rows()}else{draft.wheel_rows().saturating_add(delta as i32).clamp(0,384)};let _=draft.set_wheel_rows(value);}
            }
            return;
        }
        if index <= 1
            && (enter
                || matches!(
                    code,
                    KeyCode::ArrowLeft
                        | KeyCode::ArrowRight
                        | KeyCode::Home
                        | KeyCode::Digit0
                        | KeyCode::Numpad0
                ))
        {
            if let Some(settings) = self.scroll_preferences_draft.as_mut() {
                let value = if matches!(code, KeyCode::Home | KeyCode::Digit0 | KeyCode::Numpad0) {
                    false
                } else if index == 0 {
                    !settings.stop_playback()
                } else {
                    !settings.stop_playback_when_dragging()
                };
                if index == 0 {
                    settings.set_stop_playback(value);
                } else {
                    settings.set_stop_playback_when_dragging(value);
                }
            }
            return;
        }
        if !enter {
            return;
        }
        if index == 2 {
            self.restore_source_scroll();
            return;
        }
        if index != 3 {
            return;
        }
        let Some(settings) = self.scroll_preferences_draft else {
            return;
        };
        let result = (|| -> Result<(), deadsync_editor::Error> {
            self.commit_scroll_preferences(settings)?;
            Ok(())
        })();
        match result {
            Ok(()) => {
                self.scroll_preferences_draft = None;
                self.panel = None;
                self.status = i18n::tr("Edition", "ScrollSettingsSaved");
            }
            Err(error) => {
                self.status = i18n::tr_fmt("Edition", "Error", &[("message", &crate::i18n::editor_error(&error))])
            }
        }
    }
}
