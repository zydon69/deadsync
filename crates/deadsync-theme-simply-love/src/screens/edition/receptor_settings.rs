//! Keyboard draft for the source receptor effects. Save is one global undo action.
use super::*;
use deadsync_editor::receptor_preferences::Preferences;
const KEYS: [&str; 11] = [
    "ReceptorHideArrows",
    "ReceptorLightHolds",
    "ReceptorAutoRim",
    "ReceptorAutoGlow",
    "ReceptorAutoShrink",
    "ReceptorTapRim",
    "ReceptorTapShrink",
    "ReceptorPulse",
    "ScrollSettingsDefaults",
    "ScrollSettingsSave",
    "ScrollSettingsCancel",
];
fn flag(p: &mut Preferences, index: usize) -> Option<&mut bool> {
    match index {
        0 => Some(&mut p.hide_arrows),
        1 => Some(&mut p.light_holds),
        2 => Some(&mut p.animation.autoplay_rim),
        3 => Some(&mut p.animation.autoplay_glow),
        4 => Some(&mut p.animation.autoplay_shrink),
        5 => Some(&mut p.animation.tap_rim),
        6 => Some(&mut p.animation.tap_shrink),
        7 => Some(&mut p.animation.pulse),
        _ => None,
    }
}
impl State {
    pub(super) fn open_receptor_settings(&mut self) {
        self.minimap_release_input();
        self.stop_audio();
        self.receptor_views.release_input();
        self.receptor_draft = Some(self.receptor_preferences());
        self.panel = Some(Panel::ReceptorSettings(0));
        self.status = i18n::tr("Edition", "ReceptorSettingsHelp");
    }
    pub(super) fn sync_receptor_preferences_history(&mut self) {
        if let Some(update) = self.workspace.take_receptor_preferences_update() {
            self.configure_receptor_preferences(update.preferences);
            if let Some(file) = &update.file {
                self.audio_preferences_path = Some(file.path().to_owned());
            }
            self.audio_preferences_file = update.file;
        }
    }
    pub(super) fn receptor_settings_key(&mut self, index: usize, code: KeyCode) {
        let enter = matches!(code, KeyCode::Enter | KeyCode::NumpadEnter);
        if code == KeyCode::Escape || (index == 10 && enter) {
            self.receptor_draft = None;
            self.panel = None;
            self.status = i18n::tr("Edition", "ScrollSettingsDiscarded");
            return;
        }
        if matches!(code, KeyCode::ArrowUp | KeyCode::ArrowDown | KeyCode::Tab) {
            let back = code == KeyCode::ArrowUp || (code == KeyCode::Tab && self.shift);
            self.panel = Some(Panel::ReceptorSettings(
                (index + if back { 10 } else { 1 }) % 11,
            ));
            return;
        }
        if index < 8
            && (enter
                || matches!(
                    code,
                    KeyCode::ArrowLeft | KeyCode::ArrowRight | KeyCode::Home
                ))
        {
            if let Some(draft) = self.receptor_draft.as_mut() {
                let default = *flag(&mut Preferences::default(), index).unwrap();
                let value = flag(draft, index).unwrap();
                *value = if code == KeyCode::Home {
                    default
                } else {
                    !*value
                };
            }
            return;
        }
        if !enter {
            return;
        }
        if index == 8 {
            self.receptor_draft = Some(Preferences::default());
            return;
        }
        if index != 9 {
            return;
        }
        let Some(after) = self.receptor_draft else {
            return;
        };
        let before = self.receptor_preferences();
        let result = (|| -> Result<bool, deadsync_editor::Error> {
            let mut file = self.audio_preferences_file.clone();
            if file.is_none() {
                if let Some(path) = &self.audio_preferences_path {
                    file = Some(deadsync_editor::audio_preferences::File::open(path)?);
                }
            }
            let changed = self
                .workspace
                .set_receptor_preferences(before, after, file.as_ref())?;
            self.sync_receptor_preferences_history();
            Ok(changed)
        })();
        match result {
            Ok(_) => {
                self.receptor_draft = None;
                self.panel = None;
                self.status = i18n::tr("Edition", "ScrollSettingsSaved");
            }
            Err(error) => {
                self.status = i18n::tr_fmt("Edition", "Error", &[("message", &error.to_string())]);
            }
        }
    }
    pub(super) fn receptor_settings_actors(&self, index: usize, actors: &mut Vec<Actor>) {
        let mut settings = self.receptor_draft.unwrap_or(self.receptor_preferences());
        let x = screen_width() * 0.5;
        let top = screen_height() * 0.5 - 170.0;
        actors.push(
            act!(quad:xy(x,top+160.0):setsize(560.0,350.0):diffuse(0.04,0.06,0.1,0.98):z(8050)),
        );
        for (row, key) in KEYS.iter().enumerate() {
            let label = if row < 8 {
                let enabled = *flag(&mut settings, row).unwrap();
                i18n::tr_fmt(
                    "Edition",
                    key,
                    &[(
                        "value",
                        &i18n::tr(
                            "Edition",
                            if enabled { "WaveformYes" } else { "WaveformNo" },
                        ),
                    )],
                )
            } else {
                i18n::tr("Edition", key)
            };
            actors.push(act!(text:font("miso"):settext(label):xy(x,top+row as f32*30.0):zoom(0.65):maxwidth(520.0):diffuse(1.0,if row==index{0.8}else{1.0},if row==index{0.3}else{1.0},1.0):z(8051)));
        }
    }
}
