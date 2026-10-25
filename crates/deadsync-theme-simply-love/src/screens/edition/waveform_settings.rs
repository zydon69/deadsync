use super::*;
use deadsync_editor::{audio_preferences::File, waveform_preferences::Preferences};
impl State {
    pub(super) fn open_waveform_settings(&mut self) {
        self.stop_audio();
        self.waveform_preferences_draft = Some(self.waveform_preferences);
        self.panel = Some(Panel::WaveformSettings(0));
        self.status = i18n::tr("Edition", "WaveformSettingsHelp");
    }
    pub(super) fn waveform_settings_label(&self, index: usize, key: &str) -> Arc<str> {
        let settings = self
            .waveform_preferences_draft
            .unwrap_or(self.waveform_preferences);
        let value = match index {
            0 => i18n::tr(
                "Edition",
                if settings.show() {
                    "WaveformYes"
                } else {
                    "WaveformNo"
                },
            )
            .to_string(),
            1 => i18n::tr(
                "Edition",
                if settings.enabled() {
                    "WaveformYes"
                } else {
                    "WaveformNo"
                },
            )
            .to_string(),
            2 => format!("{:.1}", settings.channel_width() * 100.0),
            3 => format!("{:.2}", settings.dense_scale()),
            _ => i18n::tr(
                "Edition",
                &format!("WaveformScroll{}", settings.scroll_mode().name()),
            )
            .to_string(),
        };
        i18n::tr_fmt("Edition", key, &[("value", &value)])
    }
    pub(super) fn waveform_settings_key(&mut self, index: usize, code: KeyCode) {
        if code == KeyCode::F2 {
            self.panel = Some(Panel::WaveformColors(0));
            self.status = i18n::tr("Edition", "WaveformColorsHelp");
            return;
        }
        let enter = matches!(code, KeyCode::Enter | KeyCode::NumpadEnter);
        if code == KeyCode::Escape || (index == 7 && enter) {
            self.waveform_preferences_draft = None;
            if let Some(file) = self.audio_preferences_file.as_mut() {
                file.set_waveform(self.waveform_preferences);
            }
            self.panel = None;
            self.status = i18n::tr("Edition", "WaveformSettingsDiscarded");
            return;
        }
        if matches!(code, KeyCode::ArrowUp | KeyCode::ArrowDown | KeyCode::Tab) {
            let back = code == KeyCode::ArrowUp || (code == KeyCode::Tab && self.shift);
            self.panel = Some(Panel::WaveformSettings(
                (index + if back { 7 } else { 1 }) % 8,
            ));
            return;
        }
        if index < 5
            && (matches!(
                code,
                KeyCode::ArrowLeft
                    | KeyCode::ArrowRight
                    | KeyCode::Home
                    | KeyCode::Digit0
                    | KeyCode::Numpad0
            ) || ((index < 2 || index == 4) && enter))
        {
            let Some(settings) = self.waveform_preferences_draft.as_mut() else {
                return;
            };
            let defaults = Preferences::default();
            let zero = matches!(code, KeyCode::Digit0 | KeyCode::Numpad0);
            let home = code == KeyCode::Home;
            let result = match index {
                0 => {
                    settings.set_show(if home {
                        defaults.show()
                    } else if zero {
                        false
                    } else {
                        !settings.show()
                    });
                    Ok(())
                }
                1 => {
                    settings.set_enabled(if home {
                        defaults.enabled()
                    } else if zero {
                        false
                    } else {
                        !settings.enabled()
                    });
                    Ok(())
                }
                2 => {
                    let delta = if self.shift { 0.001 } else { 0.01 };
                    let value = if home {
                        defaults.channel_width()
                    } else if zero {
                        0.0
                    } else {
                        settings.channel_width()
                            + if code == KeyCode::ArrowLeft {
                                -delta
                            } else {
                                delta
                            }
                    };
                    settings.set_channel_width(value.clamp(0.0, 1.0))
                }
                3 => {
                    let delta = if self.shift { 0.01 } else { 0.1 };
                    let value = if home {
                        defaults.dense_scale()
                    } else if zero {
                        0.0
                    } else {
                        settings.dense_scale()
                            + if code == KeyCode::ArrowLeft {
                                -delta
                            } else {
                                delta
                            }
                    };
                    settings.set_dense_scale(value.clamp(0.0, 10.0))
                }
                _ => {
                    settings.set_scroll_mode(if home {
                        defaults.scroll_mode()
                    } else if zero {
                        deadsync_editor::waveform_spacing::ScrollMode::CurrentTempo
                    } else {
                        settings.scroll_mode().cycle(code == KeyCode::ArrowLeft)
                    });
                    Ok(())
                }
            };
            if let Err(error) = result {
                self.status = i18n::tr_fmt("Edition", "Error", &[("message", &error.to_string())]);
            }
            return;
        }
        if index == 5 && enter {
            self.waveform_preferences_draft = Some(Preferences::default());
        }
        if index != 6 || !enter {
            return;
        }
        let Some(path) = self.audio_preferences_path.clone() else {
            self.status = i18n::tr("Edition", "WaveformSettingsUnavailable");
            return;
        };
        let Some(settings) = self.waveform_preferences_draft else {
            return;
        };
        let result = (|| -> Result<(), deadsync_editor::Error> {
            if self.audio_preferences_file.is_none() {
                self.audio_preferences_file = Some(File::open(&path)?);
            }
            let file = self.audio_preferences_file.as_mut().unwrap();
            file.set_waveform(settings);
            file.save()?;
            Ok(())
        })();
        match result {
            Ok(()) => {
                self.waveform_preferences = settings;
                self.waveform_preferences_draft = None;
                self.panel = None;
                self.apply_waveform_preferences();
                self.status = i18n::tr("Edition", "WaveformSettingsSaved");
            }
            Err(error) => {
                self.status = i18n::tr_fmt("Edition", "Error", &[("message", &error.to_string())])
            }
        }
    }
}
