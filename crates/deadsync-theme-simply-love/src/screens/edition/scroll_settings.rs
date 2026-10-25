use super::*;
use deadsync_editor::{audio_preferences::File, scroll_preferences::Preferences};
impl State {
    pub fn scroll_preferences(&self) -> Preferences {
        self.scroll_preferences
    }
    pub(super) fn open_scroll_settings(&mut self) {
        self.stop_audio();
        self.scroll_preferences_draft = Some(self.scroll_preferences);
        self.panel = Some(Panel::ScrollSettings(0));
        self.status = i18n::tr("Edition", "ScrollSettingsHelp");
    }
    pub(super) fn scroll_settings_label(&self, key: &str) -> Arc<str> {
        let settings = self
            .scroll_preferences_draft
            .unwrap_or(self.scroll_preferences);
        let value = i18n::tr(
            "Edition",
            if settings.stop_playback() {
                "WaveformYes"
            } else {
                "WaveformNo"
            },
        );
        i18n::tr_fmt("Edition", key, &[("value", &value)])
    }
    pub(super) fn scroll_settings_key(&mut self, index: usize, code: KeyCode) {
        let enter = matches!(code, KeyCode::Enter | KeyCode::NumpadEnter);
        if code == KeyCode::Escape || (index == 3 && enter) {
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
                (index + if back { 3 } else { 1 }) % 4,
            ));
            return;
        }
        if index == 0
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
                } else {
                    !settings.stop_playback()
                };
                settings.set_stop_playback(value);
            }
            return;
        }
        if !enter {
            return;
        }
        if index == 1 {
            self.scroll_preferences_draft = Some(Preferences::default());
            return;
        }
        if index != 2 {
            return;
        }
        let Some(path) = self.audio_preferences_path.clone() else {
            self.status = i18n::tr("Edition", "ScrollSettingsUnavailable");
            return;
        };
        let Some(settings) = self.scroll_preferences_draft else {
            return;
        };
        let result = (|| -> Result<(), deadsync_editor::Error> {
            if self.audio_preferences_file.is_none() {
                self.audio_preferences_file = Some(File::open(&path)?);
            }
            let file = self.audio_preferences_file.as_mut().unwrap();
            file.set_scroll(settings);
            file.save()?;
            Ok(())
        })();
        match result {
            Ok(()) => {
                self.scroll_preferences = settings;
                self.scroll_preferences_draft = None;
                self.panel = None;
                self.status = i18n::tr("Edition", "ScrollSettingsSaved");
            }
            Err(error) => {
                self.status = i18n::tr_fmt("Edition", "Error", &[("message", &error.to_string())])
            }
        }
    }
}
