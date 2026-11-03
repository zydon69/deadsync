//! Keyboard-accessible viewport settings, independent of song undo/history.
use super::*;
use deadsync_editor::{minimap_preferences::Preferences, minimap_viewport::Axis};
impl State {
    pub(super) fn open_minimap_settings(&mut self) {
        self.minimap_release_input();
        let pending = if matches!(self.panel, Some(Panel::MinimapOptions(_))) {
            self.pending_minimap_visual()
        } else {
            self.visual_minimap()
        };
        self.minimap.visual_draft = Some(pending);
        self.minimap.options_draft = None;
        self.minimap.draft = Some(pending.viewport);
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
            self.minimap.options_draft = None;
            self.minimap.visual_draft = None;
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
                            .clamp(
                                if axis == Axis::Time { 30.0 } else { 3072.0 },
                                if axis == Axis::Time { 300.0 } else { 28800.0 },
                            )
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
            self.minimap.visual_draft = Some(self.pending_minimap_visual().restored());
            self.minimap.draft = Some(Preferences::default());
            return;
        }
        if index != 5 {
            return;
        }
        let Some(settings) = self.minimap.draft else {
            return;
        };
        let before = self.visual_minimap();
        let mut after = self.minimap.visual_draft.unwrap_or(before);
        after.viewport = settings;
        let result = self.apply_visual_preferences(
            deadsync_editor::visual_preferences::Snapshot::MiniMap(before),
            deadsync_editor::visual_preferences::Snapshot::MiniMap(after),
        );
        match result {
            Ok(()) => {
                self.minimap.draft = None;
                self.minimap.options_draft = None;
                self.minimap.visual_draft = None;
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
            match settings.variable_spacing() {
                deadsync_editor::minimap_preferences::VariableSpacing::ConstantTime => {
                    i18n::tr("Edition", "MinimapAxisTime").to_string()
                }
                deadsync_editor::minimap_preferences::VariableSpacing::ConstantRow => {
                    i18n::tr("Edition", "MinimapAxisRow").to_string()
                }
                deadsync_editor::minimap_preferences::VariableSpacing::Variable => {
                    i18n::tr("Edition", "MinimapAxisSourceVariable").to_string()
                }
                deadsync_editor::minimap_preferences::VariableSpacing::Unknown(value) => {
                    i18n::tr_fmt(
                        "Edition",
                        "MinimapAxisSourceUnknown",
                        &[("value", &value.to_string())],
                    )
                    .to_string()
                }
            },
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
            let ui_row_start = actors.len();
            let label = if row < 4 {
                i18n::tr_fmt("Edition", key, &[("value", &values[row])])
            } else {
                i18n::tr("Edition", key)
            };
            actors.push(act!(text: font("miso"): settext(label): xy(x,y+row as f32*30.0): zoom(0.75): maxwidth(520.0): diffuse(1.0,if row==index {0.8}else{1.0},if row==index {0.3}else{1.0},1.0): z(8051)));

            self.ui_row(actors, ui_row_start, row == index, 30.);
        }
    }
}

#[cfg(test)]
mod source_viewport_ui_bounds_tests {
    use super::*;
    #[test]
    fn minimap_keyboard_ranges_match_source_sliders_without_restricting_loaded_values() {
        i18n::init_for_tests();
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("song.ssc");
        std::fs::write(
            &path,
            "#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;",
        )
        .unwrap();
        let mut state = State::open(&path, 0).unwrap();
        state.open_minimap_settings();
        for (index, axis, min, max) in [(1, Axis::Time, 30., 300.), (2, Axis::Row, 3072., 28800.)] {
            state
                .minimap
                .draft
                .as_mut()
                .unwrap()
                .set_visible_range(axis, 1.)
                .unwrap();
            state.minimap_settings_key(index, KeyCode::ArrowLeft);
            assert_eq!(state.minimap.draft.unwrap().visible_range(axis), min);
            state
                .minimap
                .draft
                .as_mut()
                .unwrap()
                .set_visible_range(axis, u32::MAX as f64)
                .unwrap();
            state.minimap_settings_key(index, KeyCode::ArrowRight);
            assert_eq!(state.minimap.draft.unwrap().visible_range(axis), max);
            state.minimap_settings_key(index, KeyCode::Home);
            assert_eq!(
                state.minimap.draft.unwrap().visible_range(axis),
                Preferences::default().visible_range(axis)
            );
        }
        let mut raw = Preferences::default();
        raw.set_visible_range(Axis::Time, 1.).unwrap();
        raw.set_visible_range(Axis::Row, u32::MAX as f64).unwrap();
        assert_eq!(raw.visible_range(Axis::Time), 1.);
    }
}
