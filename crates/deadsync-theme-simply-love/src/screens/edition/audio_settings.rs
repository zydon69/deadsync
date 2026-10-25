use super::*;
use deadsync_editor::audio_preferences::{Audio, File};
impl State {
    pub(super) fn open_audio_settings(&mut self) {
        self.stop_audio();
        self.audio_preferences_draft = Some(self.audio_preferences);
        self.panel = Some(Panel::AudioSettings(0));
        self.status = i18n::tr("Edition", "AudioSettingsHelp");
    }
    pub(super) fn audio_settings_label(&self, index: usize, key: &str) -> Arc<str> {
        let audio = self
            .audio_preferences_draft
            .unwrap_or(self.audio_preferences);
        let seconds = if index == 0 {
            audio.fade_in()
        } else {
            audio.fade_out()
        };
        i18n::tr_fmt("Edition", key, &[("seconds", &format!("{seconds:.4}"))])
    }
    pub(super) fn audio_settings_key(&mut self, index: usize, code: KeyCode) {
        if code == KeyCode::Escape
            || (index == 4 && matches!(code, KeyCode::Enter | KeyCode::NumpadEnter))
        {
            self.audio_preferences_draft = None;
            if let Some(file) = self.audio_preferences_file.as_mut() {
                file.set_audio(self.audio_preferences);
            }
            self.panel = Some(Panel::SongSample(0));
            self.status = i18n::tr("Edition", "AudioSettingsDiscarded");
            return;
        }
        if matches!(code, KeyCode::ArrowUp | KeyCode::ArrowDown | KeyCode::Tab) {
            let backward = code == KeyCode::ArrowUp || (code == KeyCode::Tab && self.shift);
            self.panel = Some(Panel::AudioSettings(
                (index + if backward { 4 } else { 1 }) % 5,
            ));
            return;
        }
        if index < 2
            && matches!(
                code,
                KeyCode::ArrowLeft
                    | KeyCode::ArrowRight
                    | KeyCode::Home
                    | KeyCode::Digit0
                    | KeyCode::Numpad0
            )
        {
            if let Some(audio) = self.audio_preferences_draft.as_mut() {
                let current = if index == 0 {
                    audio.fade_in()
                } else {
                    audio.fade_out()
                };
                let delta = if self.shift { 0.0001 } else { 0.001 };
                let value = match code {
                    KeyCode::Home => {
                        if index == 0 {
                            Audio::default().fade_in()
                        } else {
                            Audio::default().fade_out()
                        }
                    }
                    KeyCode::Digit0 | KeyCode::Numpad0 => 0.0,
                    KeyCode::ArrowLeft => (current - delta).max(0.0),
                    _ => current + delta,
                };
                let result = if index == 0 {
                    audio.set_fade_in(value)
                } else {
                    audio.set_fade_out(value)
                };
                if let Err(error) = result {
                    self.status =
                        i18n::tr_fmt("Edition", "Error", &[("message", &error.to_string())]);
                }
            }
        } else if matches!(code, KeyCode::Enter | KeyCode::NumpadEnter) {
            if index == 2 {
                self.audio_preferences_draft = Some(Audio::default());
            }
            if index != 3 {
                return;
            }
            let Some(path) = self.audio_preferences_path.clone() else {
                self.status = i18n::tr("Edition", "AudioSettingsUnavailable");
                return;
            };
            let Some(audio) = self.audio_preferences_draft else {
                return;
            };
            let result = (|| -> Result<(), deadsync_editor::Error> {
                if self.audio_preferences_file.is_none() {
                    self.audio_preferences_file = Some(File::open(&path)?);
                }
                let file = self.audio_preferences_file.as_mut().unwrap();
                file.set_audio(audio);
                file.save()?;
                Ok(())
            })();
            match result {
                Ok(()) => {
                    self.audio_preferences = audio;
                    self.audio_preferences_draft = None;
                    self.panel = Some(Panel::SongSample(0));
                    self.status = i18n::tr("Edition", "AudioSettingsSaved");
                }
                Err(error) => {
                    self.status =
                        i18n::tr_fmt("Edition", "Error", &[("message", &error.to_string())])
                }
            }
        }
    }
}
