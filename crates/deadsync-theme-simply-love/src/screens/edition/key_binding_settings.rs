//! Staged global key bindings. The panel keeps fixed navigation until Save.
use super::*;
use deadsync_editor::key_binding_preferences::inspector::{Capture, CapturePhase, Draft};
use std::sync::Arc;
pub(super) struct Settings {
    pub(super) draft: Draft,
    capture: Option<Capture>,
    conflicts: Vec<String>,
}
impl Settings {
    fn refresh_conflicts(&mut self) {
        let registry = deadsync_editor::key_commands::Registry::new(self.draft.staged().bindings());
        let chord = &self.draft.alternatives()[self.draft.alternative];
        self.conflicts = registry.conflicts(
            self.draft.action().id(),
            &chord
                .iter()
                .map(|key| deadsync_editor::key_commands::Key::named(key))
                .collect::<Vec<_>>(),
        );
    }
    pub(super) fn release_input(&mut self) {
        self.capture = None;
    }
}
const ROWS: [&str; 8] = [
    "KeyBindingAction",
    "KeyBindingAlternative",
    "KeyBindingCapture",
    "KeyBindingAdd",
    "KeyBindingDelete",
    "KeyBindingReset",
    "ScrollSettingsSave",
    "ScrollSettingsCancel",
];
impl State {
    pub(super) fn open_key_binding_settings(&mut self) {
        self.release_input();
        self.stop_audio();
        match Draft::capture(Arc::clone(&self.command_bindings.preferences)) {
            Ok(draft) => {
                self.key_binding_draft = Some(Settings {
                    draft,
                    capture: None,
                    conflicts: Vec::new(),
                });
                self.key_binding_draft.as_mut().unwrap().refresh_conflicts();
                self.panel = Some(Panel::KeyBindingSettings(0));
                self.status = i18n::tr("Edition", "KeyBindingHelp");
            }
            Err(error) => self.key_binding_error(error),
        }
    }
    fn key_binding_error(&mut self, error: deadsync_editor::Error) {
        self.status = i18n::tr_fmt("Edition", "Error", &[("message", &error.to_string())]);
    }
    pub(super) fn sync_key_binding_preferences_history(&mut self) {
        if let Some(update) = self.workspace.take_key_binding_preferences_update() {
            self.configure_key_binding_preferences(update.preferences);
            if let Some(file) = &update.file {
                self.audio_preferences_path = Some(file.path().to_owned());
            }
            self.audio_preferences_file = update.file;
        }
    }
    /// Full raw events reach this before modifier/modal early returns. Capture
    /// finishes when its chord is released; Enter/Escape then act in Review.
    pub(super) fn key_binding_capture_event(&mut self, event: &RawKeyboardEvent) -> bool {
        let Some(settings) = self.key_binding_draft.as_mut() else {
            return false;
        };
        let Some(capture) = settings.capture.as_mut() else {
            return false;
        };
        if capture.phase() == CapturePhase::Review {
            if event.pressed && !event.repeat {
                if event.code == KeyCode::Escape {
                    settings.capture = None;
                } else if matches!(event.code, KeyCode::Enter | KeyCode::NumpadEnter) {
                    let keys = capture.keys().to_vec();
                    match settings.draft.replace(keys) {
                        Ok(()) => {
                            settings.capture = None;
                            settings.refresh_conflicts();
                        }
                        Err(error) => self.key_binding_error(error),
                    }
                }
            }
            return true;
        }
        let keys = self
            .command_bindings
            .capture_snapshot(self.control_keys, self.shift_keys);
        capture.observe(&keys);
        true
    }
    pub(super) fn key_binding_settings_key(&mut self, index: usize, code: KeyCode) {
        let enter = matches!(code, KeyCode::Enter | KeyCode::NumpadEnter);
        if code == KeyCode::Escape || (index == 7 && enter) {
            self.key_binding_draft = None;
            self.panel = None;
            return;
        }
        if matches!(code, KeyCode::ArrowUp | KeyCode::ArrowDown | KeyCode::Tab) {
            let backward = code == KeyCode::ArrowUp || (code == KeyCode::Tab && self.shift);
            self.panel = Some(Panel::KeyBindingSettings(
                (index + if backward { 7 } else { 1 }) % 8,
            ));
            return;
        }
        let Some(settings) = self.key_binding_draft.as_mut() else {
            return;
        };
        if matches!(code, KeyCode::PageUp | KeyCode::PageDown)
            || (index == 0 && matches!(code, KeyCode::ArrowLeft | KeyCode::ArrowRight))
        {
            settings
                .draft
                .select_action(matches!(code, KeyCode::PageDown | KeyCode::ArrowRight));
            settings.refresh_conflicts();
            return;
        }
        if index == 1 && matches!(code, KeyCode::ArrowLeft | KeyCode::ArrowRight) {
            let count = settings.draft.alternatives().len();
            settings.draft.alternative = (settings.draft.alternative
                + if code == KeyCode::ArrowRight {
                    1
                } else {
                    count - 1
                })
                % count;
            settings.refresh_conflicts();
            return;
        }
        if !enter {
            return;
        }
        let result = match index {
            2 => {
                settings.capture = Some(Capture::new());
                return;
            }
            3 => settings.draft.add(),
            4 => settings.draft.delete(),
            5 => settings.draft.reset_action(),
            6 => {
                let before = settings.draft.baseline();
                let after = settings.draft.staged();
                let result = (|| -> Result<(), deadsync_editor::Error> {
                    let file = match self.audio_preferences_file.clone() {
                        Some(file) => Some(file),
                        None => self
                            .audio_preferences_path
                            .as_ref()
                            .map(|path| deadsync_editor::audio_preferences::File::open(path))
                            .transpose()?,
                    };
                    self.workspace
                        .set_key_binding_preferences(before, after, file.as_ref())?;
                    self.sync_key_binding_preferences_history();
                    Ok(())
                })();
                if result.is_ok() {
                    self.key_binding_draft = None;
                    self.panel = None;
                }
                result
            }
            _ => Ok(()),
        };
        if let Err(error) = result {
            self.key_binding_error(error);
        } else if let Some(settings) = self.key_binding_draft.as_mut() {
            settings.refresh_conflicts();
        }
    }
    pub(super) fn key_binding_settings_actors(&self, index: usize, actors: &mut Vec<Actor>) {
        let Some(settings) = self.key_binding_draft.as_ref() else {
            return;
        };
        let x = screen_width() * 0.5;
        let top = screen_height() * 0.5 - 155.;
        actors
            .push(act!(quad:xy(x,top+145.):setsize(620.,330.):diffuse(0.04,0.06,0.1,0.98):z(8050)));
        let alternatives = settings.draft.alternatives();
        let chord = &alternatives[settings.draft.alternative];
        for (row, key) in ROWS.iter().enumerate() {
            let label = match row {
                0 => i18n::tr_fmt(
                    "Edition",
                    key,
                    &[(
                        "action",
                        &i18n::tr(
                            "Edition",
                            &format!("KeyBindingAction{}", settings.draft.action().id()),
                        ),
                    )],
                ),
                1 => i18n::tr_fmt(
                    "Edition",
                    key,
                    &[
                        ("index", &(settings.draft.alternative + 1).to_string()),
                        ("total", &alternatives.len().to_string()),
                        (
                            "keys",
                            &if chord.is_empty() {
                                i18n::tr("Edition", "KeyBindingUnbound").to_string()
                            } else {
                                super::command_bindings::chord_label(chord)
                            },
                        ),
                    ],
                ),
                _ => i18n::tr("Edition", key),
            };
            actors.push(act!(text:font("miso"):settext(label):xy(x,top+row as f32*30.):zoom(0.62):maxwidth(580.):diffuse(1.,if row==index{0.8}else{1.},if row==index{0.3}else{1.},1.):z(8051)));
        }
        let conflicts = &settings.conflicts;
        if !conflicts.is_empty() {
            let warning = i18n::tr_fmt(
                "Edition",
                "KeyBindingConflicts",
                &[(
                    "commands",
                    &conflicts
                        .iter()
                        .map(|id| i18n::tr("Edition", &format!("KeyBindingAction{id}")))
                        .collect::<Vec<_>>()
                        .join(", "),
                )],
            );
            actors.push(act!(text:font("miso"):settext(warning):xy(x,top+240.):zoom(0.5):maxwidth(580.):diffuse(1.,0.7,0.2,1.):z(8052)));
        }
        if let Some(capture) = &settings.capture {
            let help = i18n::tr_fmt(
                "Edition",
                if capture.phase() == CapturePhase::Review {
                    "KeyBindingReview"
                } else {
                    "KeyBindingRecording"
                },
                &[(
                    "keys",
                    &super::command_bindings::chord_label(capture.keys()),
                )],
            );
            actors.push(act!(text:font("miso"):settext(help):xy(x,top+270.):zoom(0.55):maxwidth(580.):z(8052)));
        }
    }
}
