//! Keyboard-accessible viewport settings, independent of song undo/history.
use super::*;
use deadsync_editor::{minimap_preferences::Preferences, minimap_viewport::Axis};
impl State {
    pub(super) fn open_minimap_settings(&mut self) {
        self.minimap_release_input();
        self.stop_audio();
        self.minimap.draft = Some(self.minimap.preferences);
        self.panel = Some(Panel::MinimapSettings(0));
        self.status = i18n::tr("Edition", "MinimapSettingsHelp");
    }
    pub(super) fn minimap_settings_key(&mut self, index: usize, code: KeyCode) {
        let enter = matches!(code, KeyCode::Enter | KeyCode::NumpadEnter);
        if index == 7 && enter {
            self.open_minimap_options();
            return;
        }
        if code == KeyCode::Escape || (index == 6 && enter) {
            self.minimap.draft = None;
            if let Some(file) = self.audio_preferences_file.as_mut() {
                file.set_minimap(self.minimap.preferences);
            }
            self.panel = None;
            self.status = i18n::tr("Edition", "ScrollSettingsDiscarded");
            return;
        }
        if matches!(code, KeyCode::ArrowUp | KeyCode::ArrowDown | KeyCode::Tab) {
            let backwards = code == KeyCode::ArrowUp || (code == KeyCode::Tab && self.shift);
            self.panel = Some(Panel::MinimapSettings(
                (index + if backwards { 7 } else { 1 }) % 8,
            ));
            return;
        }
        if index < 4
            && (enter
                || matches!(
                    code,
                    KeyCode::ArrowLeft | KeyCode::ArrowRight | KeyCode::Home
                ))
        {
            if let Some(settings) = self.minimap.draft.as_mut() {
                match index {
                    0 => settings.set_variable_axis(
                        if code == KeyCode::Home || settings.variable_axis() == Axis::Row {
                            Axis::Time
                        } else {
                            Axis::Row
                        },
                    ),
                    1 | 2 => {
                        let axis = if index == 1 { Axis::Time } else { Axis::Row };
                        let step = if axis == Axis::Time { 10.0 } else { 192.0 };
                        let value = if code == KeyCode::Home {
                            Preferences::default().visible_range(axis)
                        } else {
                            (settings.visible_range(axis)
                                + if code == KeyCode::ArrowLeft {
                                    -step
                                } else {
                                    step
                                })
                            .clamp(1.0, u32::MAX as f64)
                        };
                        let _ = settings.set_visible_range(axis, value);
                    }
                    3 => settings.set_quantize(code != KeyCode::Home && !settings.quantize()),
                    _ => unreachable!(),
                }
            }
            return;
        }
        if !enter {
            return;
        }
        if index == 4 {
            self.minimap.draft = Some(Preferences::default());
            return;
        }
        if index != 5 {
            return;
        }
        let Some(settings) = self.minimap.draft else {
            return;
        };
        let result = (|| -> Result<(), deadsync_editor::Error> {
            let path = self.audio_preferences_path.as_ref().ok_or_else(|| {
                deadsync_editor::Error::InvalidEdit("Missing preferences path".into())
            })?;
            if self.audio_preferences_file.is_none() {
                self.audio_preferences_file =
                    Some(deadsync_editor::audio_preferences::File::open(path)?);
            }
            let file = self.audio_preferences_file.as_mut().unwrap();
            file.set_minimap(settings);
            file.save()?;
            Ok(())
        })();
        match result {
            Ok(()) => {
                self.configure_minimap_preferences(settings);
                self.panel = None;
                self.status = i18n::tr("Edition", "ScrollSettingsSaved");
            }
            Err(error) => {
                log::warn!("Editor minimap preferences: {error}");
                self.status = i18n::tr("Edition", "MinimapFailed");
            }
        }
    }
    pub(super) fn minimap_settings_actors(&self, index: usize, actors: &mut Vec<Actor>) {
        let settings = self.minimap.draft.unwrap_or(self.minimap.preferences);
        let values = [
            i18n::tr(
                "Edition",
                if settings.variable_axis() == Axis::Time {
                    "MinimapAxisTime"
                } else {
                    "MinimapAxisRow"
                },
            )
            .to_string(),
            settings.visible_range(Axis::Time).to_string(),
            settings.visible_range(Axis::Row).to_string(),
            i18n::tr(
                "Edition",
                if settings.quantize() {
                    "WaveformYes"
                } else {
                    "WaveformNo"
                },
            )
            .to_string(),
        ];
        let x = screen_width() * 0.5;
        let y = screen_height() * 0.5 - 100.0;
        actors.push(
            act!(quad: xy(x,y+100.0): setsize(560.0,290.0): diffuse(0.04,0.06,0.1,0.98): z(8050)),
        );
        for (row, key) in [
            "MinimapVariableAxis",
            "MinimapTimeRange",
            "MinimapRowRange",
            "MinimapQuantize",
            "ScrollSettingsDefaults",
            "ScrollSettingsSave",
            "ScrollSettingsCancel",
            "MinimapAdvanced",
        ]
        .into_iter()
        .enumerate()
        {
            let label = if row < 4 {
                i18n::tr_fmt("Edition", key, &[("value", &values[row])])
            } else {
                i18n::tr("Edition", key)
            };
            actors.push(act!(text: font("miso"): settext(label): xy(x,y+row as f32*30.0): zoom(0.75): maxwidth(520.0): diffuse(1.0,if row==index {0.8}else{1.0},if row==index {0.3}else{1.0},1.0): z(8051)));
        }
    }
}
