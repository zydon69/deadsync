//! Shared Stream inspector. Saving produces one non-file preference action.
use super::*;
use deadsync_editor::{
    density_preferences::Accumulation,
    stream_preferences::{NoteType, Preferences},
};
const KEYS: [&str; 10] = [
    "StreamNoteType",
    "StreamAccumulation",
    "StreamShowBreaks",
    "StreamMinimum",
    "StreamCutoff",
    "StreamShortMark",
    "StreamLongMark",
    "ScrollSettingsDefaults",
    "ScrollSettingsSave",
    "ScrollSettingsCancel",
];
impl State {
    pub fn stream_preferences(&self) -> Preferences {
        self.stream_preferences
    }
    pub fn configure_stream_preferences(
        &mut self,
        p: Preferences,
    ) -> Result<(), deadsync_editor::Error> {
        p.validate()?;
        self.stream_preferences = p;
        self.source_density.stream = p.density_settings();
        self.refresh_source_density();
        Ok(())
    }
    pub(super) fn open_stream_settings(&mut self) {
        self.minimap_release_input();
        self.receptor_views.release_input();
        self.stream_draft = Some(self.stream_preferences);
        self.panel = Some(Panel::StreamSettings(0));
        self.label_preedit.clear();
        self.status = i18n::tr("Edition", "StreamHelp");
    }
    pub(super) fn stream_character_active(&self) -> bool {
        matches!(self.panel, Some(Panel::StreamSettings(5 | 6)))
    }
    pub(super) fn stream_text_input(&mut self, text: &str) -> bool {
        let Some(c) = text
            .chars()
            .next()
            .filter(|c| !c.is_control() && c.len_utf16() == 1)
        else {
            self.status = i18n::tr("Edition", "StreamCharacterInvalid");
            return false;
        };
        let Some(d) = &mut self.stream_draft else {
            return false;
        };
        match self.panel {
            Some(Panel::StreamSettings(5)) => d.short_break = c,
            Some(Panel::StreamSettings(6)) => d.long_break = c,
            _ => return false,
        };
        self.label_preedit.clear();
        true
    }
    pub(super) fn sync_stream_preferences_history(&mut self) {
        if let Some(update) = self.workspace.take_stream_preferences_update() {
            if let Some(file) = update.file {self.publish_application_preferences_owner(file);}
            if self
                .configure_stream_preferences(update.preferences)
                .is_err()
            {
                self.status = i18n::tr("Edition", "StreamSaveFailed");
            }
        }
    }
    pub(super) fn stream_settings_key(&mut self, index: usize, code: KeyCode) {
        let enter = matches!(code, KeyCode::Enter | KeyCode::NumpadEnter);
        if code == KeyCode::Escape || (index == 9 && enter) {
            self.stream_draft = None;
            self.panel = None;
            self.label_preedit.clear();
            self.status = i18n::tr("Edition", "ScrollSettingsDiscarded");
            return;
        }
        if matches!(code, KeyCode::ArrowUp | KeyCode::ArrowDown | KeyCode::Tab) {
            let back = code == KeyCode::ArrowUp || (code == KeyCode::Tab && self.shift);
            self.panel = Some(Panel::StreamSettings(
                (index + if back { 9 } else { 1 }) % 10,
            ));
            self.label_preedit.clear();
            return;
        }
        if index == 7 && enter {
            self.stream_draft = Some(Preferences::default());
            return;
        }
        if index == 8 && enter {
            let Some(after) = self.stream_draft else {
                return;
            };
            let result = (|| -> Result<bool, deadsync_editor::Error> {
                let file = self.capture_application_preferences_file()?;
                let changed = self.workspace.set_stream_preferences(
                    self.stream_preferences,
                    after,
                    Some(&file),
                )?;
                self.sync_stream_preferences_history();
                Ok(changed)
            })();
            match result {
                Ok(_) => {
                    self.stream_draft = None;
                    self.panel = None;
                    self.label_preedit.clear();
                    self.status = i18n::tr("Edition", "ScrollSettingsSaved");
                }
                Err(_) => self.status = i18n::tr("Edition", "StreamSaveFailed"),
            };
            return;
        }
        if index >= 7 {
            return;
        }
        let Some(d) = &mut self.stream_draft else {
            return;
        };
        let default = Preferences::default();
        let home = code == KeyCode::Home;
        let delta = if code == KeyCode::ArrowLeft { -1 } else { 1 };
        if index >= 5 {
            if home {
                if index == 5 {
                    d.short_break = default.short_break
                } else {
                    d.long_break = default.long_break
                }
            } else if matches!(code, KeyCode::Backspace | KeyCode::Delete) {
                if index == 5 {
                    d.short_break = ' '
                } else {
                    d.long_break = ' '
                }
            }
            return;
        }
        if !(home || enter || matches!(code, KeyCode::ArrowLeft | KeyCode::ArrowRight)) {
            return;
        }
        match index {
            0 => {
                d.note_type = if home {
                    default.note_type
                } else {
                    NoteType::ALL[(d.note_type.index() + if delta < 0 { 8 } else { 1 }) % 9]
                }
            }
            1 => {
                d.accumulation = if home {
                    default.accumulation
                } else if d.accumulation == Accumulation::Step {
                    Accumulation::Row
                } else {
                    Accumulation::Step
                }
            }
            2 => {
                d.show_break_lengths = if home {
                    default.show_break_lengths
                } else {
                    !d.show_break_lengths
                }
            }
            3 => {
                d.minimum_length = if home {
                    default.minimum_length
                } else {
                    d.minimum_length.saturating_add(delta).clamp(0, 8)
                }
            }
            4 => {
                d.short_break_cutoff = if home {
                    default.short_break_cutoff
                } else {
                    d.short_break_cutoff
                        .saturating_add(delta * if self.shift { 8 } else { 1 })
                        .clamp(0, 64)
                }
            }
            _ => {}
        }
    }
    pub(super) fn stream_settings_actors(&self, index: usize, actors: &mut Vec<Actor>) {
        let p = self.stream_draft.unwrap_or(self.stream_preferences);
        let x = screen_width() * 0.5;
        let top = (screen_height() * 0.5 - 145.0).max(12.0);
        actors.push(
            act!(quad:xy(x,top+132.0):zoomto(580.0,310.0):diffuse(0.04,0.06,0.1,0.98):z(8050)),
        );
        for (row, key) in KEYS.iter().enumerate() {
            let ui_row_start=actors.len();
            let value = match row {
                0 => format!("1/{}", p.note_type.count()),
                1 => if let Accumulation::Unknown(raw) = p.accumulation {
                    i18n::tr("Edition", "AccumulationRawRows").replace("{value}", &raw.to_string())
                } else { i18n::tr(
                    "Edition",
                    if p.accumulation == Accumulation::Step {
                        "DensityStep"
                    } else {
                        "DensityRow"
                    },
                )
                .to_string() },
                2 => i18n::tr(
                    "Edition",
                    if p.show_break_lengths {
                        "WaveformYes"
                    } else {
                        "WaveformNo"
                    },
                )
                .to_string(),
                3 => p.minimum_length.to_string(),
                4 => p.short_break_cutoff.to_string(),
                5 | 6 => {
                    let c = if row == 5 {
                        p.short_break
                    } else {
                        p.long_break
                    };
                    if c == ' ' {
                        i18n::tr("Edition", "StreamSpace").to_string()
                    } else {
                        if c.is_control() {
                            c.escape_default().to_string()
                        } else {
                            c.to_string()
                        }
                    }
                }
                _ => String::new(),
            };
            let label = if let Some(label)=self.preference_numeric_label(row){label}else if row < 7 {
                i18n::tr_fmt("Edition", key, &[("value", &value)])
            } else {
                i18n::tr("Edition", key)
            };
            actors.push(act!(text:font("miso"):settext(label):xy(x,top+row as f32*29.0):zoom(0.62):maxwidth(550.0):diffuse(1.0,if row==index{0.8}else{1.0},if row==index{0.3}else{1.0},1.0):z(8051)));

            self.ui_row(actors,ui_row_start,row==index,31.);
        }
    }
}
