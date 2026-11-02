use super::*;
use deadsync_editor::zoom_preferences::Preferences;
pub(super) struct Edit {
    before: Preferences,
    session: deadsync_editor::chart_order::SessionBinding,
}
impl State {
    pub fn zoom_preferences(&self) -> Preferences {
        self.zoom_preferences
    }
    pub(super) fn open_zoom_settings(&mut self) {
        self.zoom_preferences_edit = Some(Edit {
            before: self.zoom_preferences,
            session: deadsync_editor::chart_order::SessionBinding::capture(
                &self.workspace.file().session,
            ),
        });
        self.zoom_preferences_draft = Some(self.zoom_preferences);
        self.panel = Some(Panel::ZoomSettings(0));
        self.status = i18n::tr("Edition", "ZoomSettingsHelp");
    }
    pub(super) fn zoom_settings_label(&self, index: usize, key: &str) -> Arc<str> {
        let settings = self.zoom_preferences_draft.unwrap_or(self.zoom_preferences);
        let value = match index {
            0 => format!("{:.3}", settings.duration()),
            1 => format!("{:.2}", settings.multiplier()),
            2 => format!("{:.6}", settings.size_cap()),
            _ => i18n::tr(
                "Edition",
                if settings.limited() {
                    "WaveformYes"
                } else {
                    "WaveformNo"
                },
            )
            .to_string(),
        };
        i18n::tr_fmt("Edition", key, &[("value", &value)])
    }
    pub(super) fn zoom_settings_key(&mut self, index: usize, code: KeyCode) {
        let enter = matches!(code, KeyCode::Enter | KeyCode::NumpadEnter);
        if code == KeyCode::Escape || (index == 6 && enter) {
            self.zoom_preferences_draft = None;
            self.zoom_preferences_edit = None;
            self.panel = None;
            self.status = i18n::tr("Edition", "ZoomSettingsDiscarded");
            return;
        }
        if matches!(code, KeyCode::ArrowUp | KeyCode::ArrowDown | KeyCode::Tab) {
            let back = code == KeyCode::ArrowUp || (code == KeyCode::Tab && self.shift);
            self.panel = Some(Panel::ZoomSettings((index + if back { 6 } else { 1 }) % 7));
            return;
        }
        if index < 4
            && (matches!(
                code,
                KeyCode::ArrowLeft
                    | KeyCode::ArrowRight
                    | KeyCode::Home
                    | KeyCode::Digit0
                    | KeyCode::Numpad0
            ) || (index == 3 && enter))
        {
            let Some(settings) = self.zoom_preferences_draft.as_mut() else {
                return;
            };
            let defaults = Preferences::default();
            let home = code == KeyCode::Home;
            let zero = matches!(code, KeyCode::Digit0 | KeyCode::Numpad0);
            let sign = if code == KeyCode::ArrowLeft {
                -1.0
            } else {
                1.0
            };
            let step = if self.shift { 0.001 } else { 0.01 };
            let result = match index {
                0 => settings.set_duration(if home {
                    defaults.duration()
                } else if zero {
                    0.0
                } else {
                    (settings.duration() + sign * step).clamp(0.0, 1.0)
                }),
                1 => settings.set_multiplier(if home {
                    defaults.multiplier()
                } else if zero {
                    1.0
                } else {
                    (settings.multiplier() + sign * step * 10.0).clamp(1.0, 4.0)
                }),
                2 => settings.set_size_cap(if home {
                    defaults.size_cap()
                } else if zero {
                    deadsync_editor::zoom::MIN_ZOOM
                } else {
                    (settings.size_cap() + sign * step).clamp(deadsync_editor::zoom::MIN_ZOOM, 1.0)
                }),
                _ => {
                    settings.set_limited(if home {
                        defaults.limited()
                    } else if zero {
                        false
                    } else {
                        !settings.limited()
                    });
                    Ok(())
                }
            };
            if let Err(error) = result {
                self.status = i18n::tr_fmt(
                    "Edition",
                    "Error",
                    &[("message", &crate::i18n::editor_error(&error))],
                );
            }
            return;
        }
        if !enter {
            return;
        }
        if index == 4 {
            self.zoom_preferences_draft = Some(Preferences::default());
            return;
        }
        if index != 5 {
            return;
        }
        let Some(settings) = self.zoom_preferences_draft else {
            return;
        };
        let result = (|| -> Result<(), deadsync_editor::Error> {
            let edit = self
                .zoom_preferences_edit
                .as_ref()
                .ok_or(deadsync_editor::Error::ExternalChange)?;
            if !edit.session.belongs_to(&self.workspace.file().session) {
                return Err(deadsync_editor::Error::ExternalChange);
            }
            let before = edit.before;
            let current = self.capture_application_preferences_file()?;
            let merged = current.zoom().merge_changes(before, settings)?;
            let mut prepared = self.zoom.clone();
            merged.apply(&mut prepared)?;
            self.workspace
                .apply_zoom_preferences(before, settings, &current)?;
            self.sync_restore_scroll_history()?;
            Ok(())
        })();
        match result {
            Ok(()) => {
                self.zoom_preferences_draft = None;
                self.zoom_preferences_edit = None;
                self.panel = None;
                self.status = i18n::tr("Edition", "ZoomSettingsSaved");
            }
            Err(error) => {
                self.status = i18n::tr_fmt(
                    "Edition",
                    "Error",
                    &[("message", &crate::i18n::editor_error(&error))],
                )
            }
        }
    }
}
#[cfg(test)]
#[path = "zoom_settings/history_tests.rs"]
mod history_tests;
