//! Compact keyboard inspector for every source DensityGraph setting.
use super::{Panel, State};
use crate::{act, i18n};
use deadlib_platform::input::KeyCode;
use deadlib_present::{
    actors::Actor,
    space::{screen_height, screen_width},
};
use deadsync_editor::density_preferences::{Accumulation, ColorMode, Position, Preferences};
#[derive(Clone, Copy)]
pub(super) struct Draft {
    pub preferences: Preferences,
    pub(super) field: usize,
}
const FIELDS: [&str; 20] = [
    "DensityVisible",
    "DensityStream",
    "DensityAccumulation",
    "DensityPosition",
    "DensityOffset",
    "DensityWidthOffset",
    "DensityHeight",
    "DensityColorMode",
    "DensityHighR",
    "DensityHighG",
    "DensityHighB",
    "DensityHighA",
    "DensityLowR",
    "DensityLowG",
    "DensityLowB",
    "DensityLowA",
    "DensityBackgroundR",
    "DensityBackgroundG",
    "DensityBackgroundB",
    "DensityBackgroundA",
];
impl State {
    pub(super) fn open_density_settings(&mut self) {
        self.minimap_release_input();
        self.receptor_views.release_input();
        self.density_draft = Some(Draft {
            preferences: self.density_preferences(),
            field: 0,
        });
        self.panel = Some(Panel::DensitySettings(0));
        self.status = i18n::tr("Edition", "DensitySettingsHelp");
    }
    pub(super) fn sync_density_preferences_history(&mut self) {
        if let Some(update) = self.workspace.take_density_preferences_update() {
            if let Some(file) = update.file {
                if file.path().as_os_str().is_empty() {
                    self.audio_preferences_path = None;
                    self.audio_preferences_file = None;
                    self.source_preferences_import.memory = Some(file);
                } else {
                    self.audio_preferences_path = Some(file.path().to_owned());
                    self.source_preferences_import.memory = None;
                    self.audio_preferences_file = Some(file);
                }
            }
            if self
                .configure_density_preferences(update.preferences)
                .is_err()
            {
                self.status = i18n::tr("Edition", "DensitySaveFailed");
            }
        }
    }
    pub(super) fn density_settings_key(&mut self, index: usize, code: KeyCode) {
        let enter = matches!(code, KeyCode::Enter | KeyCode::NumpadEnter);
        if code == KeyCode::Escape || (index == 4 && enter) {
            self.density_draft = None;
            self.panel = None;
            self.status = i18n::tr("Edition", "ScrollSettingsDiscarded");
            return;
        }
        if matches!(code, KeyCode::ArrowUp | KeyCode::ArrowDown | KeyCode::Tab) {
            let back = code == KeyCode::ArrowUp || (code == KeyCode::Tab && self.shift);
            self.panel = Some(Panel::DensitySettings(
                (index + if back { 4 } else { 1 }) % 5,
            ));
            return;
        }
        if matches!(
            code,
            KeyCode::ArrowLeft | KeyCode::ArrowRight | KeyCode::Home
        ) || enter && index < 2
        {
            let Some(d) = &mut self.density_draft else {
                return;
            };
            let forward = code != KeyCode::ArrowLeft;
            if index == 0 {
                d.field = if code == KeyCode::Home {
                    0
                } else {
                    (d.field + if forward { 1 } else { 19 }) % 20
                };
            } else if index == 1 {
                adjust(
                    &mut d.preferences,
                    d.field,
                    forward,
                    code == KeyCode::Home,
                    self.shift,
                );
            }
            return;
        }
        if !enter {
            return;
        }
        if index == 2 {
            if let Some(d) = &mut self.density_draft {
                d.preferences = Default::default();
            }
            return;
        }
        if index != 3 {
            return;
        }
        let Some(after) = self.density_draft.map(|d| d.preferences) else {
            return;
        };
        let before = self.density_preferences();
        let result = (|| -> Result<bool, deadsync_editor::Error> {
            let file = self.capture_application_preferences_file()?;
            let changed = self
                .workspace
                .set_density_preferences(before, after, Some(&file))?;
            self.sync_density_preferences_history();
            Ok(changed)
        })();
        match result {
            Ok(_) => {
                self.density_draft = None;
                self.panel = None;
                self.status = i18n::tr("Edition", "ScrollSettingsSaved");
            }
            Err(_) => self.status = i18n::tr("Edition", "DensitySaveFailed"),
        }
    }
    pub(super) fn density_settings_actors(&self, index: usize, actors: &mut Vec<Actor>) {
        let Some(d) = self.density_draft else {
            return;
        };
        let x = screen_width() * 0.5;
        let top = (screen_height() - 230.0) * 0.5;
        actors.push(
            act!(quad:xy(x,top+115.0):zoomto(560.0,230.0):diffuse(0.04,0.06,0.1,0.98):z(8050)),
        );
        let labels = [
            i18n::tr_fmt(
                "Edition",
                "DensityField",
                &[("value", &i18n::tr("Edition", FIELDS[d.field]))],
            ),
            i18n::tr_fmt(
                "Edition",
                "DensityValue",
                &[("value", &value(d.preferences, d.field))],
            ),
            i18n::tr("Edition", "ScrollSettingsDefaults"),
            i18n::tr("Edition", "ScrollSettingsSave"),
            i18n::tr("Edition", "ScrollSettingsCancel"),
        ];
        for (row, label) in labels.into_iter().enumerate() {
            let ui_row_start = actors.len();
            actors.push(act!(text:font("miso"):settext(label):xy(x,top+26.0+row as f32*32.0):zoom(0.63):maxwidth(530.0):diffuse(1.0,if row==index{0.8}else{1.0},if row==index{0.3}else{1.0},1.0):z(8051)));

            self.ui_row(actors, ui_row_start, row == index, 32.);
        }
    }
}
fn adjust(p: &mut Preferences, field: usize, forward: bool, home: bool, large: bool) {
    let d = Preferences::default();
    let delta = if forward { 1 } else { -1 };
    match field {
        0 => p.visible = if home { d.visible } else { !p.visible },
        1 => p.stream = if home { d.stream } else { !p.stream },
        2 => {
            p.accumulation = if home {
                d.accumulation
            } else if p.accumulation == Accumulation::Step {
                Accumulation::Row
            } else {
                Accumulation::Step
            }
        }
        3 => {
            p.position = if home {
                d.position
            } else {
                Position::ALL[(p.position.index() + if forward { 1 } else { 5 }) % 6]
            }
        }
        4 | 5 => {
            let i = p.position.index();
            let target = if field == 4 {
                &mut p.position_offsets
            } else {
                &mut p.width_offsets
            };
            let defaults = if field == 4 {
                d.position_offsets
            } else {
                d.width_offsets
            };
            target[i] = if home {
                defaults[i]
            } else {
                target[i]
                    .saturating_add(delta * if large { 10 } else { 1 })
                    .clamp(-1024, 1024)
            };
        }
        6 => {
            p.height = if home {
                d.height
            } else {
                p.height
                    .saturating_add(delta * if large { 10 } else { 1 })
                    .clamp(16, 1024)
            }
        }
        7 => {
            p.color_mode = if home {
                d.color_mode
            } else if p.color_mode == ColorMode::ColorByDensity {
                ColorMode::ColorByHeight
            } else {
                ColorMode::ColorByDensity
            }
        }
        8..=19 => {
            let i = (field - 8) % 4;
            let (target, default) = match (field - 8) / 4 {
                0 => (&mut p.high, d.high),
                1 => (&mut p.low, d.low),
                _ => (&mut p.background, d.background),
            };
            target[i] = if home {
                default[i]
            } else {
                (target[i] + delta as f32 * if large { 0.1 } else { 0.01 }).clamp(0.0, 1.0)
            };
        }
        _ => {}
    }
}
fn value(p: Preferences, field: usize) -> String {
    let tr = |key| i18n::tr("Edition", key).to_string();
    match field {
        0 => tr(if p.visible {
            "WaveformYes"
        } else {
            "WaveformNo"
        }),
        1 => tr(if p.stream {
            "WaveformYes"
        } else {
            "WaveformNo"
        }),
        2 => {
            if let Accumulation::Unknown(raw) = p.accumulation {
                i18n::tr("Edition", "AccumulationRawRows").replace("{value}", &raw.to_string())
            } else {
                tr(if p.accumulation == Accumulation::Step {
                    "DensityStep"
                } else {
                    "DensityRow"
                })
            }
        }
        3 => tr([
            "DensityRight",
            "DensityLeft",
            "DensityFocused",
            "DensityFocusedScaled",
            "DensityTop",
            "DensityBottom",
        ][p.position.index()]),
        4 => p.position_offsets[p.position.index()].to_string(),
        5 => p.width_offsets[p.position.index()].to_string(),
        6 => p.height.to_string(),
        7 => tr(if p.color_mode == ColorMode::ColorByDensity {
            "DensityByDensity"
        } else {
            "DensityByHeight"
        }),
        8..=19 => {
            let v = match (field - 8) / 4 {
                0 => p.high,
                1 => p.low,
                _ => p.background,
            };
            format!("{:.3}", v[(field - 8) % 4])
        }
        _ => String::new(),
    }
}

#[cfg(test)]
mod memory_history_tests {
    use super::*;
    use deadsync_editor::{audio_preferences::File, workspace::Command};
    #[test]
    fn density_restore_save_cancel_undo_redo_keeps_pathless_application_owner() {
        for language in ["en", "fr"] {
            i18n::init(deadsync_assets::language::load_for_tests(language));
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("song.ssc");
            std::fs::write(
                &path,
                "#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;",
            )
            .unwrap();
            let mut state = State::open(&path, 0).unwrap();
            let song = state.workspace.file().session.serialize().unwrap();
            let mut file = File::memory().unwrap();
            let mut before = file.density();
            before.height = 123;
            before.width_offsets[0] = 501;
            file.set_density(before).unwrap();
            state.configure_density_preferences(before).unwrap();
            state.source_preferences_import.memory = Some(file);
            state.open_density_settings();
            state.density_settings_key(2, KeyCode::Enter);
            state.density_settings_key(4, KeyCode::Enter);
            assert_eq!(state.density_preferences(), before);
            state.open_density_settings();
            state.density_settings_key(2, KeyCode::Enter);
            state.density_settings_key(3, KeyCode::Enter);
            assert_eq!(state.density_preferences(), Preferences::default());
            state.command(Command::Undo);
            assert_eq!(state.density_preferences(), before);
            state.command(Command::Redo);
            assert_eq!(state.density_preferences(), Preferences::default());
            assert!(state.audio_preferences_path.is_none());
            assert!(state.audio_preferences_file.is_none());
            assert_eq!(
                state
                    .source_preferences_import
                    .memory
                    .as_ref()
                    .unwrap()
                    .density(),
                Preferences::default()
            );
            assert!(!state.workspace.file().session.is_dirty());
            assert_eq!(state.workspace.file().session.serialize().unwrap(), song);
        }
    }
}

#[cfg(test)]
mod raw_slider_tests {
    use super::*;
    #[test]
    fn raw_extremes_survive_until_explicit_slider_input_then_use_source_ui_bounds() {
        for raw in [i32::MIN, i32::MAX] {
            for forward in [false, true] {
                let mut p = Preferences::default();
                p.height = raw;
                p.position_offsets[0] = raw;
                p.width_offsets[0] = raw;
                p.position = Position::RightSideOfWindow;
                assert!(p.validate().is_ok());
                adjust(&mut p, 4, forward, false, true);
                assert!((-1024..=1024).contains(&p.position_offsets[0]));
                assert_eq!(p.width_offsets[0], raw);
                assert_eq!(p.height, raw);
                adjust(&mut p, 5, forward, false, true);
                assert!((-1024..=1024).contains(&p.width_offsets[0]));
                adjust(&mut p, 6, forward, false, true);
                assert!((16..=1024).contains(&p.height));
            }
        }
    }
}

#[cfg(test)]
mod unknown_accumulation_tests {
    use super::*;
    #[test]
    fn raw_ordinal_is_visible_and_explicit_cycle_and_restore_choose_known_values() {
        for language in ["en", "fr"] {
            i18n::init(deadsync_assets::language::load_for_tests(language));
            let mut p = Preferences::default();
            p.accumulation = Accumulation::Unknown(99);
            assert!(value(p, 2).contains("99"));
            adjust(&mut p, 2, true, false, false);
            assert_eq!(p.accumulation, Accumulation::Step);
        }
    }
}
