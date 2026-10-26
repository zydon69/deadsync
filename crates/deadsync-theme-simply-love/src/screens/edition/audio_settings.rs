use super::*;
use deadsync_editor::audio_preferences::{Audio, File};
impl State {
    pub(super) fn sync_audio_preferences_history(&mut self) {
        if let Some(update) = self.workspace.take_audio_preferences_update() {
            let previous_rate = self.audio_preferences.music_rate();
            self.audio_preferences = update.audio;
            if let Some(file) = &update.file {
                self.audio_preferences_path = Some(file.path().to_owned());
            }
            self.audio_preferences_file = update.file;
            if previous_rate != self.audio_preferences.music_rate() {
                self.apply_playback_rate();
            }
        }
    }
    fn commit_audio_preferences(&mut self, audio: Audio) -> Result<bool, deadsync_editor::Error> {
        if self.audio_preferences_file.is_none() {
            if let Some(path) = &self.audio_preferences_path {
                self.audio_preferences_file = Some(File::open(path)?);
            }
        }
        let changed = self.workspace.set_audio_preferences(
            self.audio_preferences,
            audio,
            self.audio_preferences_file.as_ref(),
        )?;
        self.sync_audio_preferences_history();
        Ok(changed)
    }
    pub(super) fn step_music_rate(&mut self, increase: bool) {
        let mut audio = self.audio_preferences;
        audio.step_music_rate(increase);
        match self.commit_audio_preferences(audio) {
            Ok(_) => {
                self.status = i18n::tr_fmt(
                    "Edition",
                    "AudioMusicRate",
                    &[("rate", &format!("{:.3}", audio.music_rate()))],
                )
            }
            Err(error) => {
                self.status = i18n::tr_fmt("Edition", "Error", &[("message", &error.to_string())])
            }
        }
    }
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
        if index == 2 {
            return i18n::tr_fmt(
                "Edition",
                key,
                &[("rate", &format!("{:.4}", audio.music_rate()))],
            );
        }
        let seconds = if index == 0 {
            audio.fade_in()
        } else {
            audio.fade_out()
        };
        i18n::tr_fmt("Edition", key, &[("seconds", &format!("{seconds:.4}"))])
    }
    pub(super) fn audio_settings_key(&mut self, index: usize, code: KeyCode) {
        if code == KeyCode::Escape
            || (index == 5 && matches!(code, KeyCode::Enter | KeyCode::NumpadEnter))
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
                (index + if backward { 5 } else { 1 }) % 6,
            ));
            return;
        }
        if index < 3
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
                } else if index == 1 {
                    audio.fade_out()
                } else {
                    audio.music_rate()
                };
                let delta = if self.shift { 0.0001 } else { 0.001 };
                let value = match code {
                    KeyCode::Home => {
                        if index == 0 {
                            Audio::default().fade_in()
                        } else if index == 1 {
                            Audio::default().fade_out()
                        } else {
                            1.0
                        }
                    }
                    KeyCode::Digit0 | KeyCode::Numpad0 => {
                        if index == 2 {
                            1.0
                        } else {
                            0.0
                        }
                    }
                    KeyCode::ArrowLeft => (current - delta).max(0.0),
                    _ => current + delta,
                };
                let result = if index == 0 {
                    audio.set_fade_in(value)
                } else if index == 1 {
                    audio.set_fade_out(value)
                } else {
                    audio.set_music_rate(value)
                };
                if let Err(error) = result {
                    self.status =
                        i18n::tr_fmt("Edition", "Error", &[("message", &error.to_string())]);
                }
            }
        } else if matches!(code, KeyCode::Enter | KeyCode::NumpadEnter) {
            if index == 3 {
                self.audio_preferences_draft = Some(Audio::default());
            }
            if index != 4 {
                return;
            }
            let Some(_) = self.audio_preferences_path else {
                self.status = i18n::tr("Edition", "AudioSettingsUnavailable");
                return;
            };
            let Some(audio) = self.audio_preferences_draft else {
                return;
            };
            let result = self.commit_audio_preferences(audio);
            match result {
                Ok(_) => {
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
